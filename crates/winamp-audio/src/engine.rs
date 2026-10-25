//! The playback engine: a decode thread feeding a lock-free ring buffer that a
//! cpal output callback drains. EQ, volume and balance are applied in the
//! callback so changes are heard immediately.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::decode::{OpenedTrack, Source, TrackInfo};
use crate::eq::{EqSettings, Equalizer};
use crate::resample::Resampler;
use crate::ring::{self, Consumer, Producer};
use crate::vis::VisRing;

/// Samples the ring buffer holds; about 0.68 s of stereo at 48 kHz.
const RING_SAMPLES: usize = 1 << 16;
/// Free space the decoder waits for before decoding another packet.
const CHUNK_SAMPLES: usize = 4096;
/// Size of the visualisation history ring.
const VIS_RING_SAMPLES: usize = 4096;
/// How long the decode thread waits for a command while idle.
const IDLE_POLL: Duration = Duration::from_millis(20);
/// Pause when the ring buffer is full.
const BACKPRESSURE_PAUSE: Duration = Duration::from_millis(5);
/// Pause after the track has been fully decoded, waiting for it to drain.
const DRAIN_PAUSE: Duration = Duration::from_millis(10);
/// How long to wait for the audio callback to acknowledge a flush.
const FLUSH_TIMEOUT: Duration = Duration::from_millis(500);
/// Seeks stop this far short of the end so the track still plays out.
const SEEK_END_MARGIN_SECS: f64 = 0.05;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum PlayState {
    Stopped = 0,
    Playing = 1,
    Paused = 2,
}

impl PlayState {
    fn from_u8(value: u8) -> Self {
        match value {
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
    /// Bumped on every EQ change so the callback knows to re-read `eq`.
    eq_version: AtomicU32,
    /// Set by the decoder to ask the callback to drop buffered audio and
    /// reset the position to `flush_to_us`; cleared by the callback.
    flush: AtomicBool,
    flush_to_us: AtomicU64,
    /// The decoder has pushed the last sample of the track.
    producer_eof: AtomicBool,
    /// The callback has played the last sample of the track.
    drained: AtomicBool,
    vis: Mutex<VisRing>,
    shutdown: AtomicBool,
}

impl Shared {
    fn state(&self) -> PlayState {
        PlayState::from_u8(self.state.load(Ordering::Acquire))
    }

    fn set_state(&self, state: PlayState) {
        self.state.store(state as u8, Ordering::Release);
    }

    fn volume(&self) -> f32 {
        f32::from_bits(self.volume_bits.load(Ordering::Relaxed))
    }

    fn balance(&self) -> f32 {
        f32::from_bits(self.balance_bits.load(Ordering::Relaxed))
    }

    fn eq_settings(&self) -> EqSettings {
        *lock_ignoring_poison(&self.eq)
    }

    fn set_eq_settings(&self, settings: EqSettings) {
        *lock_ignoring_poison(&self.eq) = settings;
        self.eq_version.fetch_add(1, Ordering::Release);
    }

    fn request_flush(&self, to_secs: f64) {
        self.flush_to_us
            .store((to_secs.max(0.0) * 1e6) as u64, Ordering::Release);
        self.flush.store(true, Ordering::Release);
    }

    fn wait_flushed(&self) {
        let start = Instant::now();
        while self.flush.load(Ordering::Acquire) && start.elapsed() < FLUSH_TIMEOUT {
            thread::sleep(Duration::from_millis(1));
        }
    }

    /// Reset the end-of-track bookkeeping before new audio starts flowing.
    fn clear_end_of_track(&self) {
        self.producer_eof.store(false, Ordering::Release);
        self.drained.store(false, Ordering::Release);
    }
}

/// The data behind these mutexes is plain `Copy` state, so a panic while it
/// was held cannot have left it inconsistent; just keep using it.
fn lock_ignoring_poison<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

pub struct Player {
    commands: Sender<Cmd>,
    events: Receiver<Event>,
    shared: Arc<Shared>,
    info: Option<TrackInfo>,
    current: Option<Source>,
    worker_threads: Vec<thread::JoinHandle<()>>,
}

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
            vis: Mutex::new(VisRing::new(VIS_RING_SAMPLES)),
            shutdown: AtomicBool::new(false),
        });
        let (producer, consumer) = ring::ring(RING_SAMPLES);

        // Output thread: owns the cpal stream for its whole lifetime.
        let (ready_tx, ready_rx) = mpsc::channel::<Result<u32>>();
        let output_thread = thread::Builder::new()
            .name("winamp-output".into())
            .spawn({
                let shared = Arc::clone(&shared);
                move || output_thread(shared, consumer, ready_tx)
            })
            .context("spawning output thread")?;
        let out_rate = ready_rx
            .recv()
            .map_err(|_| anyhow!("output thread died"))?
            .context("audio output")?;
        shared.out_rate.store(out_rate, Ordering::Release);

        let (commands, command_rx) = mpsc::channel();
        let (event_tx, events) = mpsc::channel();
        let worker = DecodeWorker {
            shared: Arc::clone(&shared),
            producer,
            events: event_tx,
            out_rate,
            active: None,
            source: None,
            decoded: Vec::with_capacity(CHUNK_SAMPLES * 4),
        };
        let decode_thread = thread::Builder::new()
            .name("winamp-decode".into())
            .spawn(move || worker.run(command_rx))
            .context("spawning decode thread")?;

        Ok(Self {
            commands,
            events,
            shared,
            info: None,
            current: None,
            worker_threads: vec![output_thread, decode_thread],
        })
    }

    fn send(&self, command: Cmd) {
        if self.commands.send(command).is_err() {
            log::error!("decode thread has stopped; command dropped");
        }
    }

    pub fn load(&mut self, source: Source, autoplay: bool) {
        self.current = Some(source.clone());
        self.info = None;
        self.send(Cmd::Load { source, autoplay });
    }

    pub fn play(&self) {
        self.send(Cmd::Play);
    }

    pub fn pause(&self) {
        self.send(Cmd::Pause);
    }

    pub fn stop(&self) {
        self.send(Cmd::Stop);
    }

    pub fn seek(&self, secs: f64) {
        self.send(Cmd::Seek(secs));
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
        self.info.as_ref().and_then(|info| info.duration)
    }

    pub fn output_sample_rate(&self) -> u32 {
        self.shared.out_rate.load(Ordering::Relaxed)
    }

    pub fn volume(&self) -> f32 {
        self.shared.volume()
    }

    pub fn set_volume(&self, volume: f32) {
        self.shared
            .volume_bits
            .store(volume.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }

    pub fn balance(&self) -> f32 {
        self.shared.balance()
    }

    pub fn set_balance(&self, balance: f32) {
        self.shared
            .balance_bits
            .store(balance.clamp(-1.0, 1.0).to_bits(), Ordering::Relaxed);
    }

    pub fn eq(&self) -> EqSettings {
        self.shared.eq_settings()
    }

    pub fn set_eq(&self, settings: EqSettings) {
        self.shared.set_eq_settings(settings.clamped());
    }

    /// Latest `count` mono samples that went to the output (for visualisation).
    pub fn vis_samples(&self, count: usize) -> Vec<f32> {
        match self.shared.vis.try_lock() {
            Ok(vis) => vis.latest(count),
            Err(_) => vec![0.0; count],
        }
    }

    /// Drain pending events. Updates cached track info as a side effect.
    pub fn poll_events(&mut self) -> Vec<Event> {
        let mut out = Vec::new();
        while let Ok(event) = self.events.try_recv() {
            match &event {
                Event::Opened(info) => self.info = Some((**info).clone()),
                Event::StreamTitle(title) => {
                    if let Some(info) = &mut self.info {
                        info.title = Some(title.clone());
                    }
                }
                Event::Finished | Event::Error(_) => {}
            }
            out.push(event);
        }
        out
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        // The decode thread may already be gone; the shutdown flag covers the output thread.
        let _ = self.commands.send(Cmd::Shutdown);
        self.shared.shutdown.store(true, Ordering::Release);
        for handle in self.worker_threads.drain(..) {
            if handle.join().is_err() {
                log::error!("audio worker thread panicked");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Output thread
// ---------------------------------------------------------------------------

/// Owns the cpal stream until shutdown; reports the device sample rate (or
/// the error that prevented opening the device) through `ready`.
fn output_thread(shared: Arc<Shared>, consumer: Consumer, ready: Sender<Result<u32>>) {
    let stream = match start_output_stream(&shared, consumer) {
        Ok((stream, sample_rate)) => {
            let _ = ready.send(Ok(sample_rate));
            stream
        }
        Err(e) => {
            let _ = ready.send(Err(e));
            return;
        }
    };
    while !shared.shutdown.load(Ordering::Acquire) {
        thread::sleep(Duration::from_millis(50));
    }
    drop(stream);
}

fn start_output_stream(shared: &Arc<Shared>, consumer: Consumer) -> Result<(cpal::Stream, u32)> {
    let device = cpal::default_host()
        .default_output_device()
        .ok_or_else(|| anyhow!("no output device"))?;
    let config = device
        .default_output_config()
        .context("querying the default output format")?;
    let sample_rate = config.sample_rate().0;
    let mixer = OutputMixer {
        shared: Arc::clone(shared),
        consumer,
        equalizer: Equalizer::new(sample_rate),
        eq_version_seen: 0,
        stereo: Vec::with_capacity(8192),
        device_channels: usize::from(config.channels()),
        sample_rate,
    };
    let stream_config: cpal::StreamConfig = config.clone().into();
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => build_stream::<f32>(&device, &stream_config, mixer),
        cpal::SampleFormat::I16 => build_stream::<i16>(&device, &stream_config, mixer),
        cpal::SampleFormat::U16 => build_stream::<u16>(&device, &stream_config, mixer),
        cpal::SampleFormat::I32 => build_stream::<i32>(&device, &stream_config, mixer),
        other => Err(anyhow!("unsupported sample format {other:?}")),
    }?;
    stream.play().context("starting the audio stream")?;
    Ok((stream, sample_rate))
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut mixer: OutputMixer,
) -> Result<cpal::Stream>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    let stream = device.build_output_stream(
        config,
        move |out: &mut [T], _| mixer.fill(out),
        |e| log::error!("audio stream error: {e}"),
        None,
    )?;
    Ok(stream)
}

/// Everything the audio callback needs: it pulls stereo samples from the ring,
/// applies EQ, volume and balance, and writes them in the device's layout.
struct OutputMixer {
    shared: Arc<Shared>,
    consumer: Consumer,
    equalizer: Equalizer,
    eq_version_seen: u32,
    /// Scratch buffer of interleaved stereo frames for one callback.
    stereo: Vec<f32>,
    device_channels: usize,
    sample_rate: u32,
}

impl OutputMixer {
    fn fill<T>(&mut self, out: &mut [T])
    where
        T: cpal::SizedSample + cpal::FromSample<f32>,
    {
        let frames = out.len() / self.device_channels.max(1);
        self.apply_pending_flush();
        self.apply_pending_eq_change();
        let silence = T::from_sample(0.0f32);
        if self.shared.state() != PlayState::Playing {
            out.fill(silence);
            return;
        }
        let got_frames = self.pull_stereo_frames(frames);
        self.equalizer.process(&mut self.stereo);
        self.apply_volume_and_balance();
        if let Ok(mut vis) = self.shared.vis.try_lock() {
            vis.push_stereo(&self.stereo[..got_frames * 2]);
        }
        self.write_device_frames(out, silence);
        let advance_us = (got_frames as u64 * 1_000_000) / u64::from(self.sample_rate);
        self.shared
            .position_us
            .fetch_add(advance_us, Ordering::Relaxed);
    }

    fn apply_pending_flush(&mut self) {
        if self.shared.flush.load(Ordering::Acquire) {
            self.consumer.clear();
            self.shared.position_us.store(
                self.shared.flush_to_us.load(Ordering::Acquire),
                Ordering::Release,
            );
            self.shared.drained.store(false, Ordering::Release);
            self.shared.flush.store(false, Ordering::Release);
        }
    }

    fn apply_pending_eq_change(&mut self) {
        let version = self.shared.eq_version.load(Ordering::Acquire);
        if version != self.eq_version_seen {
            // Never block the audio thread: if the UI holds the lock, try again next callback.
            if let Ok(settings) = self.shared.eq.try_lock() {
                self.equalizer.apply(*settings);
                self.eq_version_seen = version;
            }
        }
    }

    /// Fill `self.stereo` with `frames` frames, zero-padding when the ring
    /// runs dry. Returns how many frames actually came from the ring.
    fn pull_stereo_frames(&mut self, frames: usize) -> usize {
        self.stereo.clear();
        self.stereo.resize(frames * 2, 0.0);
        let got = self.consumer.pop(&mut self.stereo);
        let got_frames = got / 2;
        if got_frames < frames {
            self.stereo[got..].fill(0.0);
            if self.shared.producer_eof.load(Ordering::Acquire) && self.consumer.len() < 2 {
                self.shared.drained.store(true, Ordering::Release);
            }
        }
        got_frames
    }

    fn apply_volume_and_balance(&mut self) {
        let volume = self.shared.volume();
        let balance = self.shared.balance();
        let left_gain = volume * (1.0 - balance).min(1.0);
        let right_gain = volume * (1.0 + balance).min(1.0);
        for frame in self.stereo.chunks_exact_mut(2) {
            frame[0] *= left_gain;
            frame[1] *= right_gain;
        }
    }

    fn write_device_frames<T>(&self, out: &mut [T], silence: T)
    where
        T: cpal::SizedSample + cpal::FromSample<f32>,
    {
        for (frame, stereo) in out
            .chunks_mut(self.device_channels)
            .zip(self.stereo.chunks_exact(2))
        {
            let (left, right) = (stereo[0], stereo[1]);
            match frame {
                [mono] => *mono = T::from_sample(0.5 * (left + right)),
                [first, second, rest @ ..] => {
                    *first = T::from_sample(left);
                    *second = T::from_sample(right);
                    rest.fill(silence);
                }
                [] => {}
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Decode thread
// ---------------------------------------------------------------------------

/// The track currently being decoded plus its resampling state.
struct ActiveTrack {
    track: OpenedTrack,
    resampler: Resampler,
    /// Resampled samples the ring buffer did not have room for yet.
    pending: Vec<f32>,
    /// Last stream title forwarded, so repeats are not re-sent.
    last_title: Option<String>,
}

impl ActiveTrack {
    fn open(source: &Source, out_rate: u32) -> Result<Self> {
        let track = OpenedTrack::open(source)?;
        let resampler = Resampler::new(track.info.sample_rate, out_rate);
        Ok(Self {
            track,
            resampler,
            pending: Vec::new(),
            last_title: None,
        })
    }

    fn is_seekable(&self) -> bool {
        self.track.info.seekable
    }

    /// Forget any resampled-but-unplayed audio.
    fn discard_buffered(&mut self) {
        self.resampler.reset();
        self.pending.clear();
    }

    /// Seek, then discard buffered audio on success. Returns the position reached.
    fn seek_and_discard(&mut self, secs: f64) -> Result<f64> {
        let reached = self.track.seek(secs)?;
        self.discard_buffered();
        Ok(reached)
    }
}

/// Runs on the decode thread: executes commands and keeps the ring buffer fed.
struct DecodeWorker {
    shared: Arc<Shared>,
    producer: Producer,
    events: Sender<Event>,
    out_rate: u32,
    active: Option<ActiveTrack>,
    /// The most recently loaded source, kept so Stop then Play can reopen it.
    source: Option<Source>,
    /// Scratch buffer for one decoded packet.
    decoded: Vec<f32>,
}

fn emit(events: &Sender<Event>, event: Event) {
    if events.send(event).is_err() {
        log::debug!("player dropped; event discarded");
    }
}

impl DecodeWorker {
    fn run(mut self, commands: Receiver<Cmd>) {
        loop {
            let command = if self.is_decoding() {
                match commands.try_recv() {
                    Ok(command) => Some(command),
                    Err(TryRecvError::Empty) => None,
                    Err(TryRecvError::Disconnected) => return,
                }
            } else {
                match commands.recv_timeout(IDLE_POLL) {
                    Ok(command) => Some(command),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            };
            if let Some(command) = command {
                if matches!(command, Cmd::Shutdown) {
                    return;
                }
                self.handle(command);
            }
            if self.finish_if_drained() || self.shared.state() != PlayState::Playing {
                continue;
            }
            self.forward_stream_title();
            self.pump_audio();
        }
    }

    /// True while there is audio left to decode and we are playing, in which
    /// case commands are only polled between packets.
    fn is_decoding(&self) -> bool {
        self.active
            .as_ref()
            .is_some_and(|active| !active.track.is_finished())
            && self.shared.state() == PlayState::Playing
    }

    fn handle(&mut self, command: Cmd) {
        match command {
            Cmd::Load { source, autoplay } => {
                self.source = Some(source);
                if self.reopen_source() && autoplay {
                    self.shared.set_state(PlayState::Playing);
                }
            }
            Cmd::Play => self.play(),
            Cmd::Pause => self.toggle_pause(),
            Cmd::Stop => self.stop(),
            Cmd::Seek(secs) => self.seek(secs),
            Cmd::Shutdown => {}
        }
    }

    /// (Re)open the current source from the beginning and report the outcome
    /// as an event. Returns true when a track is ready.
    fn reopen_source(&mut self) -> bool {
        self.active = None;
        let Some(source) = &self.source else {
            return false;
        };
        self.shared.set_state(PlayState::Stopped);
        self.shared.clear_end_of_track();
        self.shared.request_flush(0.0);
        self.shared.wait_flushed();
        match ActiveTrack::open(source, self.out_rate) {
            Ok(active) => {
                emit(
                    &self.events,
                    Event::Opened(Box::new(active.track.info.clone())),
                );
                self.active = Some(active);
                true
            }
            Err(e) => {
                emit(&self.events, Event::Error(e.to_string()));
                false
            }
        }
    }

    fn play(&mut self) {
        match self.shared.state() {
            PlayState::Paused => self.shared.set_state(PlayState::Playing),
            PlayState::Playing => self.restart_current_track(),
            PlayState::Stopped => {
                if self.source.is_none() {
                    return;
                }
                // Finished tracks and network streams cannot simply continue.
                let needs_reopen = self.active.as_ref().is_none_or(|active| {
                    active.track.is_finished() || active.track.info.stream.is_some()
                });
                if needs_reopen && !self.reopen_source() {
                    return;
                }
                if self.active.is_some() {
                    self.shared.set_state(PlayState::Playing);
                }
            }
        }
    }

    /// Winamp restarts the track when play is pressed while already playing.
    fn restart_current_track(&mut self) {
        let Some(active) = &mut self.active else {
            return;
        };
        if active.is_seekable() && active.seek_and_discard(0.0).is_ok() {
            self.shared.producer_eof.store(false, Ordering::Release);
            self.shared.request_flush(0.0);
            self.shared.wait_flushed();
        }
    }

    fn toggle_pause(&mut self) {
        match self.shared.state() {
            PlayState::Playing => self.shared.set_state(PlayState::Paused),
            PlayState::Paused => self.shared.set_state(PlayState::Playing),
            PlayState::Stopped => {}
        }
    }

    fn stop(&mut self) {
        self.shared.set_state(PlayState::Stopped);
        self.shared.producer_eof.store(false, Ordering::Release);
        self.shared.request_flush(0.0);
        self.shared.wait_flushed();
        if !self.rewind_or_drop_active() {
            self.active = None;
        }
    }

    /// Rewind a seekable track to the start. Returns false for network
    /// streams, which cannot be rewound and must be reopened later.
    fn rewind_or_drop_active(&mut self) -> bool {
        let Some(active) = &mut self.active else {
            return true;
        };
        active.discard_buffered();
        if !active.is_seekable() {
            return false;
        }
        if let Err(e) = active.track.seek(0.0) {
            log::debug!("rewinding after stop: {e}");
        }
        true
    }

    fn seek(&mut self, secs: f64) {
        let Some(active) = &mut self.active else {
            return;
        };
        if !active.is_seekable() {
            return;
        }
        let target = match active.track.info.duration {
            Some(duration) => secs.clamp(
                0.0,
                (duration.as_secs_f64() - SEEK_END_MARGIN_SECS).max(0.0),
            ),
            None => secs.max(0.0),
        };
        match active.seek_and_discard(target) {
            Ok(reached) => {
                self.shared.producer_eof.store(false, Ordering::Release);
                self.shared.request_flush(reached);
                self.shared.wait_flushed();
            }
            Err(e) => log::warn!("seek: {e}"),
        }
    }

    /// Once the output has played the last sample, rewind and report
    /// `Finished`. Returns true when that happened this iteration.
    fn finish_if_drained(&mut self) -> bool {
        if !(self.shared.drained.load(Ordering::Acquire)
            && self.shared.state() == PlayState::Playing)
        {
            return false;
        }
        self.shared.clear_end_of_track();
        self.shared.set_state(PlayState::Stopped);
        if !self.rewind_or_drop_active() {
            self.active = None;
        }
        self.shared.request_flush(0.0);
        emit(&self.events, Event::Finished);
        true
    }

    /// Pass a new ICY stream title on to the UI, ignoring repeats.
    fn forward_stream_title(&mut self) {
        let Some(active) = &mut self.active else {
            return;
        };
        let Some(slot) = &active.track.title_slot else {
            return;
        };
        let Ok(mut slot) = slot.try_lock() else {
            return;
        };
        let Some(title) = slot.take() else { return };
        if active.last_title.as_ref() != Some(&title) {
            active.last_title = Some(title.clone());
            emit(&self.events, Event::StreamTitle(title));
        }
    }

    /// Move audio towards the output: first any leftovers, then a fresh packet.
    fn pump_audio(&mut self) {
        let Some(active) = &mut self.active else {
            return;
        };
        if !active.pending.is_empty() {
            let pushed = self.producer.push(&active.pending);
            active.pending.drain(..pushed);
            if !active.pending.is_empty() {
                thread::sleep(BACKPRESSURE_PAUSE);
                return;
            }
        }
        if active.track.is_finished() {
            thread::sleep(DRAIN_PAUSE);
            return;
        }
        if self.producer.free() < CHUNK_SAMPLES {
            thread::sleep(BACKPRESSURE_PAUSE);
            return;
        }
        self.decoded.clear();
        match active.track.decode_next(&mut self.decoded) {
            Ok(true) => {
                active.pending.clear();
                active.resampler.process(&self.decoded, &mut active.pending);
                let pushed = self.producer.push(&active.pending);
                active.pending.drain(..pushed);
            }
            Ok(false) => self.shared.producer_eof.store(true, Ordering::Release),
            Err(e) => {
                emit(&self.events, Event::Error(e.to_string()));
                self.shared.producer_eof.store(true, Ordering::Release);
            }
        }
    }
}
