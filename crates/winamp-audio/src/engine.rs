//! The playback engine: a decode thread feeding a lock-free ring buffer that a
//! cpal output callback drains. EQ, volume and balance are applied in the
//! callback so changes are heard immediately.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::decode::{OpenedTrack, Source, TrackInfo};
use crate::eq::{EqSettings, Equalizer};
use crate::resample::Resampler;
use crate::ring::{self, Consumer, Producer};
use crate::vis::VisRing;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum PlayState {
    Stopped = 0,
    Playing = 1,
    Paused = 2,
}

impl PlayState {
    fn from_u8(v: u8) -> Self {
        match v {
            1 => PlayState::Playing,
            2 => PlayState::Paused,
            _ => PlayState::Stopped,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Event {
    /// A source finished opening successfully.
    Opened(Box<TrackInfo>),
    /// Playback reached the end of the current track.
    Finished,
    /// Opening or decoding failed; playback has stopped.
    Error(String),
    /// A streaming server sent a new "now playing" title.
    StreamTitle(String),
}

enum Cmd {
    Load { source: Source, autoplay: bool },
    Play,
    Pause,
    Stop,
    Seek(f64),
    Shutdown,
}

/// State shared between the UI thread, decode thread and audio callback.
struct Shared {
    state: AtomicU8,
    position_us: AtomicU64,
    out_rate: AtomicU32,
    volume_bits: AtomicU32,
    balance_bits: AtomicU32,
    eq: Mutex<EqSettings>,
    eq_version: AtomicU32,
    flush: AtomicBool,
    flush_to_us: AtomicU64,
    producer_eof: AtomicBool,
    drained: AtomicBool,
    vis: Mutex<VisRing>,
    shutdown: AtomicBool,
}

impl Shared {
    fn state(&self) -> PlayState {
        PlayState::from_u8(self.state.load(Ordering::Acquire))
    }
    fn set_state(&self, s: PlayState) {
        self.state.store(s as u8, Ordering::Release);
    }
    fn volume(&self) -> f32 {
        f32::from_bits(self.volume_bits.load(Ordering::Relaxed))
    }
    fn balance(&self) -> f32 {
        f32::from_bits(self.balance_bits.load(Ordering::Relaxed))
    }
    fn request_flush(&self, to_secs: f64) {
        self.flush_to_us.store((to_secs.max(0.0) * 1e6) as u64, Ordering::Release);
        self.flush.store(true, Ordering::Release);
    }
    fn wait_flushed(&self) {
        let start = Instant::now();
        while self.flush.load(Ordering::Acquire) && start.elapsed() < Duration::from_millis(500) {
            thread::sleep(Duration::from_millis(1));
        }
    }
}

pub struct Player {
    cmd_tx: Sender<Cmd>,
    event_rx: Receiver<Event>,
    shared: Arc<Shared>,
    info: Option<TrackInfo>,
    current: Option<Source>,
    output_threads: Vec<thread::JoinHandle<()>>,
}

const RING_SAMPLES: usize = 1 << 16; // ~0.68 s of stereo at 48 kHz
const CHUNK_SAMPLES: usize = 4096;

impl Player {
    pub fn new() -> Result<Self> {
        let shared = Arc::new(Shared {
            state: AtomicU8::new(PlayState::Stopped as u8),
            position_us: AtomicU64::new(0),
            out_rate: AtomicU32::new(44100),
            volume_bits: AtomicU32::new(1.0f32.to_bits()),
            balance_bits: AtomicU32::new(0.0f32.to_bits()),
            eq: Mutex::new(EqSettings::default()),
            eq_version: AtomicU32::new(1),
            flush: AtomicBool::new(false),
            flush_to_us: AtomicU64::new(0),
            producer_eof: AtomicBool::new(false),
            drained: AtomicBool::new(false),
            vis: Mutex::new(VisRing::new(4096)),
            shutdown: AtomicBool::new(false),
        });
        let (producer, consumer) = ring::ring(RING_SAMPLES);

        // Output thread: owns the cpal stream for its whole lifetime.
        let (rate_tx, rate_rx) = mpsc::channel::<Result<u32, String>>();
        let out_shared = shared.clone();
        let output_thread = thread::Builder::new()
            .name("winamp-output".into())
            .spawn(move || output_thread(out_shared, consumer, rate_tx))?;
        let out_rate = rate_rx
            .recv()
            .map_err(|_| anyhow!("output thread died"))?
            .map_err(|e| anyhow!("audio output: {e}"))?;
        shared.out_rate.store(out_rate, Ordering::Release);

        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let dec_shared = shared.clone();
        let decode_thread = thread::Builder::new()
            .name("winamp-decode".into())
            .spawn(move || decode_thread(dec_shared, producer, cmd_rx, event_tx, out_rate))?;

        Ok(Self {
            cmd_tx,
            event_rx,
            shared,
            info: None,
            current: None,
            output_threads: vec![output_thread, decode_thread],
        })
    }

    pub fn load(&mut self, source: Source, autoplay: bool) {
        self.current = Some(source.clone());
        self.info = None;
        let _ = self.cmd_tx.send(Cmd::Load { source, autoplay });
    }

    pub fn play(&self) {
        let _ = self.cmd_tx.send(Cmd::Play);
    }
    pub fn pause(&self) {
        let _ = self.cmd_tx.send(Cmd::Pause);
    }
    pub fn stop(&self) {
        let _ = self.cmd_tx.send(Cmd::Stop);
    }
    pub fn seek(&self, secs: f64) {
        let _ = self.cmd_tx.send(Cmd::Seek(secs));
    }

    pub fn state(&self) -> PlayState {
        self.shared.state()
    }

    pub fn current_source(&self) -> Option<&Source> {
        self.current.as_ref()
    }

    pub fn info(&self) -> Option<&TrackInfo> {
        self.info.as_ref()
    }

    pub fn position(&self) -> Duration {
        Duration::from_micros(self.shared.position_us.load(Ordering::Relaxed))
    }

    pub fn duration(&self) -> Option<Duration> {
        self.info.as_ref().and_then(|i| i.duration)
    }

    pub fn output_sample_rate(&self) -> u32 {
        self.shared.out_rate.load(Ordering::Relaxed)
    }

    pub fn volume(&self) -> f32 {
        self.shared.volume()
    }
    pub fn set_volume(&self, v: f32) {
        self.shared.volume_bits.store(v.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }
    pub fn balance(&self) -> f32 {
        self.shared.balance()
    }
    pub fn set_balance(&self, b: f32) {
        self.shared.balance_bits.store(b.clamp(-1.0, 1.0).to_bits(), Ordering::Relaxed);
    }

    pub fn eq(&self) -> EqSettings {
        *self.shared.eq.lock().unwrap()
    }
    pub fn set_eq(&self, mut s: EqSettings) {
        s.clamp();
        *self.shared.eq.lock().unwrap() = s;
        self.shared.eq_version.fetch_add(1, Ordering::Release);
    }

    /// Latest `n` mono samples that went to the output (for visualisation).
    pub fn vis_samples(&self, n: usize) -> Vec<f32> {
        match self.shared.vis.try_lock() {
            Ok(v) => v.latest(n),
            Err(_) => vec![0.0; n],
        }
    }

    /// Drain pending events. Updates cached track info as a side effect.
    pub fn poll_events(&mut self) -> Vec<Event> {
        let mut out = Vec::new();
        while let Ok(ev) = self.event_rx.try_recv() {
            match &ev {
                Event::Opened(info) => self.info = Some((**info).clone()),
                Event::StreamTitle(t) => {
                    if let Some(i) = &mut self.info {
                        i.title = Some(t.clone());
                    }
                }
                _ => {}
            }
            out.push(ev);
        }
        out
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        let _ = self.cmd_tx.send(Cmd::Shutdown);
        self.shared.shutdown.store(true, Ordering::Release);
        for h in self.output_threads.drain(..) {
            let _ = h.join();
        }
    }
}

// ---------------------------------------------------------------------------
// Output thread
// ---------------------------------------------------------------------------

fn output_thread(shared: Arc<Shared>, consumer: Consumer, rate_tx: Sender<Result<u32, String>>) {
    let host = cpal::default_host();
    let device = match host.default_output_device() {
        Some(d) => d,
        None => {
            let _ = rate_tx.send(Err("no output device".into()));
            return;
        }
    };
    let config = match device.default_output_config() {
        Ok(c) => c,
        Err(e) => {
            let _ = rate_tx.send(Err(e.to_string()));
            return;
        }
    };
    let rate = config.sample_rate().0;
    let channels = config.channels() as usize;
    let stream_cfg: cpal::StreamConfig = config.clone().into();
    let res = match config.sample_format() {
        cpal::SampleFormat::F32 => build_stream::<f32>(&device, &stream_cfg, shared.clone(), consumer, channels, rate),
        cpal::SampleFormat::I16 => build_stream::<i16>(&device, &stream_cfg, shared.clone(), consumer, channels, rate),
        cpal::SampleFormat::U16 => build_stream::<u16>(&device, &stream_cfg, shared.clone(), consumer, channels, rate),
        cpal::SampleFormat::I32 => build_stream::<i32>(&device, &stream_cfg, shared.clone(), consumer, channels, rate),
        other => Err(anyhow!("unsupported sample format {other:?}")),
    };
    let stream = match res {
        Ok(s) => s,
        Err(e) => {
            let _ = rate_tx.send(Err(e.to_string()));
            return;
        }
    };
    if let Err(e) = stream.play() {
        let _ = rate_tx.send(Err(e.to_string()));
        return;
    }
    let _ = rate_tx.send(Ok(rate));
    while !shared.shutdown.load(Ordering::Acquire) {
        thread::sleep(Duration::from_millis(50));
    }
    drop(stream);
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    shared: Arc<Shared>,
    mut consumer: Consumer,
    channels: usize,
    rate: u32,
) -> Result<cpal::Stream>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    let mut eq = Equalizer::new(rate);
    let mut eq_seen = 0u32;
    let mut stereo: Vec<f32> = Vec::with_capacity(8192);
    let err_fn = |e| log::error!("audio stream error: {e}");
    let stream = device.build_output_stream(
        config,
        move |out: &mut [T], _| {
            let frames = out.len() / channels.max(1);
            if shared.flush.load(Ordering::Acquire) {
                consumer.clear();
                shared.position_us.store(shared.flush_to_us.load(Ordering::Acquire), Ordering::Release);
                shared.drained.store(false, Ordering::Release);
                shared.flush.store(false, Ordering::Release);
            }
            let ver = shared.eq_version.load(Ordering::Acquire);
            if ver != eq_seen {
                if let Ok(s) = shared.eq.try_lock() {
                    eq.apply(*s);
                    eq_seen = ver;
                }
            }
            let silence = T::from_sample(0.0f32);
            if shared.state() != PlayState::Playing {
                out.fill(silence);
                return;
            }
            stereo.clear();
            stereo.resize(frames * 2, 0.0);
            let got = consumer.pop(&mut stereo);
            let got_frames = got / 2;
            if got_frames < frames {
                for s in &mut stereo[got..] {
                    *s = 0.0;
                }
                if shared.producer_eof.load(Ordering::Acquire) && consumer.len() < 2 {
                    shared.drained.store(true, Ordering::Release);
                }
            }
            eq.process(&mut stereo);
            let vol = shared.volume();
            let bal = shared.balance();
            let lg = vol * (1.0 - bal).min(1.0);
            let rg = vol * (1.0 + bal).min(1.0);
            for f in stereo.chunks_exact_mut(2) {
                f[0] *= lg;
                f[1] *= rg;
            }
            if let Ok(mut v) = shared.vis.try_lock() {
                v.push_stereo(&stereo[..got_frames * 2]);
            }
            for (i, frame) in out.chunks_mut(channels).enumerate() {
                let l = stereo[i * 2];
                let r = stereo[i * 2 + 1];
                match channels {
                    1 => frame[0] = T::from_sample(0.5 * (l + r)),
                    _ => {
                        frame[0] = T::from_sample(l);
                        frame[1] = T::from_sample(r);
                        for s in &mut frame[2..] {
                            *s = silence;
                        }
                    }
                }
            }
            let add_us = (got_frames as u64 * 1_000_000) / rate as u64;
            shared.position_us.fetch_add(add_us, Ordering::Relaxed);
        },
        err_fn,
        None,
    )?;
    Ok(stream)
}

// ---------------------------------------------------------------------------
// Decode thread
// ---------------------------------------------------------------------------

struct Active {
    track: OpenedTrack,
    resampler: Resampler,
    pending: Vec<f32>,
    last_title: Option<String>,
}

fn decode_thread(
    shared: Arc<Shared>,
    mut producer: Producer,
    cmd_rx: Receiver<Cmd>,
    event_tx: Sender<Event>,
    out_rate: u32,
) {
    let mut active: Option<Active> = None;
    let mut source: Option<Source> = None;
    let mut decoded: Vec<f32> = Vec::with_capacity(CHUNK_SAMPLES * 4);

    let open = |src: &Source, shared: &Shared, producer: &mut Producer| -> Result<Active> {
        shared.set_state(PlayState::Stopped);
        shared.producer_eof.store(false, Ordering::Release);
        shared.drained.store(false, Ordering::Release);
        shared.request_flush(0.0);
        shared.wait_flushed();
        let _ = producer; // producer is reset by the consumer-side flush
        let track = OpenedTrack::open(src)?;
        let resampler = Resampler::new(track.info.sample_rate, out_rate);
        Ok(Active { track, resampler, pending: Vec::new(), last_title: None })
    };

    loop {
        let busy = active.as_ref().map(|a| !a.track.eof).unwrap_or(false) && shared.state() == PlayState::Playing;
        let cmd = if busy {
            match cmd_rx.try_recv() {
                Ok(c) => Some(c),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => return,
            }
        } else {
            match cmd_rx.recv_timeout(Duration::from_millis(20)) {
                Ok(c) => Some(c),
                Err(mpsc::RecvTimeoutError::Timeout) => None,
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            }
        };

        if let Some(cmd) = cmd {
            match cmd {
                Cmd::Shutdown => return,
                Cmd::Load { source: src, autoplay } => {
                    active = None;
                    source = Some(src.clone());
                    match open(&src, &shared, &mut producer) {
                        Ok(a) => {
                            let _ = event_tx.send(Event::Opened(Box::new(a.track.info.clone())));
                            active = Some(a);
                            if autoplay {
                                shared.set_state(PlayState::Playing);
                            }
                        }
                        Err(e) => {
                            let _ = event_tx.send(Event::Error(e.to_string()));
                        }
                    }
                }
                Cmd::Play => match shared.state() {
                    PlayState::Paused => shared.set_state(PlayState::Playing),
                    PlayState::Playing => {
                        // Winamp restarts the track when play is pressed while playing.
                        if let Some(a) = &mut active {
                            if a.track.info.seekable {
                                if a.track.seek(0.0).is_ok() {
                                    a.resampler.reset();
                                    a.pending.clear();
                                    shared.producer_eof.store(false, Ordering::Release);
                                    shared.request_flush(0.0);
                                    shared.wait_flushed();
                                }
                            }
                        }
                    }
                    PlayState::Stopped => {
                        if let Some(src) = source.clone() {
                            let needs_open = match &active {
                                Some(a) => a.track.eof || a.track.info.stream.is_some(),
                                None => true,
                            };
                            if needs_open {
                                active = None;
                                match open(&src, &shared, &mut producer) {
                                    Ok(a) => {
                                        let _ = event_tx.send(Event::Opened(Box::new(a.track.info.clone())));
                                        active = Some(a);
                                    }
                                    Err(e) => {
                                        let _ = event_tx.send(Event::Error(e.to_string()));
                                    }
                                }
                            }
                            if active.is_some() {
                                shared.set_state(PlayState::Playing);
                            }
                        }
                    }
                },
                Cmd::Pause => match shared.state() {
                    PlayState::Playing => shared.set_state(PlayState::Paused),
                    PlayState::Paused => shared.set_state(PlayState::Playing),
                    PlayState::Stopped => {}
                },
                Cmd::Stop => {
                    shared.set_state(PlayState::Stopped);
                    shared.producer_eof.store(false, Ordering::Release);
                    shared.request_flush(0.0);
                    shared.wait_flushed();
                    if let Some(a) = &mut active {
                        a.pending.clear();
                        a.resampler.reset();
                        if a.track.info.seekable {
                            let _ = a.track.seek(0.0);
                        } else {
                            // Network streams must be reopened.
                            active = None;
                        }
                    }
                }
                Cmd::Seek(secs) => {
                    if let Some(a) = &mut active {
                        if a.track.info.seekable {
                            let target = match a.track.info.duration {
                                Some(d) => secs.clamp(0.0, (d.as_secs_f64() - 0.05).max(0.0)),
                                None => secs.max(0.0),
                            };
                            match a.track.seek(target) {
                                Ok(actual) => {
                                    a.resampler.reset();
                                    a.pending.clear();
                                    shared.producer_eof.store(false, Ordering::Release);
                                    shared.request_flush(actual);
                                    shared.wait_flushed();
                                }
                                Err(e) => log::warn!("seek: {e}"),
                            }
                        }
                    }
                }
            }
        }

        // Track-finished detection.
        if shared.drained.load(Ordering::Acquire) && shared.state() == PlayState::Playing {
            shared.drained.store(false, Ordering::Release);
            shared.producer_eof.store(false, Ordering::Release);
            shared.set_state(PlayState::Stopped);
            if let Some(a) = &mut active {
                if a.track.info.seekable {
                    let _ = a.track.seek(0.0);
                    a.resampler.reset();
                    a.pending.clear();
                } else {
                    active = None;
                }
            }
            shared.request_flush(0.0);
            let _ = event_tx.send(Event::Finished);
            continue;
        }

        if shared.state() != PlayState::Playing {
            continue;
        }
        let Some(a) = &mut active else { continue };

        // Stream title updates.
        if let Some(slot) = &a.track.title_slot {
            if let Ok(s) = slot.try_lock() {
                if s.is_some() && *s != a.last_title {
                    a.last_title = s.clone();
                    if let Some(t) = &*s {
                        let _ = event_tx.send(Event::StreamTitle(t.clone()));
                    }
                }
            }
        }

        // Push any samples left over from last iteration.
        if !a.pending.is_empty() {
            let n = producer.push(&a.pending);
            a.pending.drain(..n);
            if !a.pending.is_empty() {
                thread::sleep(Duration::from_millis(5));
                continue;
            }
        }
        if a.track.eof {
            thread::sleep(Duration::from_millis(10));
            continue;
        }
        if producer.free() < CHUNK_SAMPLES {
            thread::sleep(Duration::from_millis(5));
            continue;
        }
        decoded.clear();
        match a.track.decode_next(&mut decoded) {
            Ok(true) => {
                a.pending.clear();
                a.resampler.process(&decoded, &mut a.pending);
                let n = producer.push(&a.pending);
                a.pending.drain(..n);
            }
            Ok(false) => {
                shared.producer_eof.store(true, Ordering::Release);
            }
            Err(e) => {
                let _ = event_tx.send(Event::Error(e.to_string()));
                shared.producer_eof.store(true, Ordering::Release);
            }
        }
    }
}
