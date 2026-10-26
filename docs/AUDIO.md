# The audio engine (`winamp-audio`)

The engine's job is narrow: given a `Source` (a local file or an HTTP URL),
produce audio on the default output device and report what happened. It
knows nothing about playlists, windows or skins.

```
  UI thread                decode thread                 cpal callback thread
  ---------                -------------                 --------------------
  Player::load ──Cmd──▶  open source (symphonia)
  Player::play           decode packet
  Player::seek           ├─ mono/multichannel → stereo
  Player::stop           ├─ resample to device rate
                         └─ push f32 samples ──▶ [ring buffer] ──▶ pop frames
                                                                   apply EQ
                                                                   apply volume/balance
                                                                   write to device
                                                                   copy into vis ring
  Player::poll_events ◀──Event── (Opened / Finished / Error / StreamTitle)
```

Files:

| File | Contents |
|------|----------|
| `engine.rs` | `Player` (the public handle), the two threads, the `Shared` state, the flush protocol |
| `decode.rs` | `Source`, `TrackInfo`, `OpenedTrack` (symphonia wrapper: open, decode, seek) |
| `stream.rs` | HTTP/HTTPS streaming with ICY metadata (`IcyReader`), MIME → extension hints, remote playlist fetch |
| `resample.rs` | Stateful Catmull-Rom resampler for interleaved stereo |
| `eq.rs` | 10-band peaking-biquad equalizer, EQF preset file format, stock presets |
| `ring.rs` | Lock-free single-producer/single-consumer `f32` ring buffer |
| `vis.rs` | Radix-2 FFT, log-spaced spectrum bars, oscilloscope downsampling, `VisRing` |
| `tags.rs` | Metadata via lofty, "Artist - Title" display logic |

## The `Player` handle

`Player::new()` creates the output stream (and therefore learns the device
sample rate) and spawns the decode thread. From then on the UI calls cheap
methods:

* `load(source, autoplay)`, `play()`, `pause()`, `stop()`, `seek(secs)` send
  a `Cmd` over an mpsc channel to the decode thread.
* `set_volume`, `set_balance`, `set_eq` write atomics / a mutex read by the
  audio callback; they take effect on the next callback (a few milliseconds).
* `state()`, `position()`, `volume()` read atomics.
* `poll_events()` drains the event channel. It also caches the latest
  `TrackInfo` so `info()` and `duration()` are available afterwards.
* `vis_samples(n)` returns the last `n` mono samples that went to the device.

`Player` is `!Sync` by design; keep it on the UI thread.

## Shared state

`Shared` (private to `engine.rs`) is an `Arc` holding:

| Field | Written by | Read by |
|-------|------------|---------|
| `state` (Stopped/Playing/Paused) | decode thread | everyone |
| `position_us` | callback (advances), callback on flush (resets) | UI |
| `volume_bits`, `balance_bits` | UI | callback |
| `eq` + `eq_version` | UI | callback (re-applies when the version changes; uses `try_lock` so it never blocks) |
| `flush`, `flush_to_us` | decode thread | callback |
| `producer_eof`, `drained` | decode thread / callback | callback / decode thread |
| `vis` (`VisRing`) | callback (`try_lock`) | UI (`try_lock`) |

Nothing in the callback blocks: the only locks are `try_lock`s that skip a
frame if contended.

## The flush protocol (seek, stop, track change)

Only the consumer (callback) may discard ring-buffer contents, so the decode
thread cannot simply clear it. Instead:

1. Decode thread sets `flush_to_us` to the new position and `flush = true`,
   then waits (polling, max 500 ms) until `flush` is false again.
2. The next callback sees `flush`, clears the ring, resets `position_us`,
   clears `drained` and sets `flush = false`.
3. Decode thread resumes pushing from the new position.

This gives sample-accurate position display after a seek and guarantees no
stale audio from before the seek is heard.

## End of track

When the decoder hits EOF it sets `producer_eof`. The callback keeps draining;
when it cannot fill a buffer and the ring is empty, it sets `drained`. The
decode thread notices `drained`, switches to Stopped, rewinds seekable sources
(so Play restarts from the beginning without reopening) and emits
`Event::Finished`. Network streams are dropped instead and reopened on Play.

## Decoding

`OpenedTrack::open` builds a symphonia `MediaSourceStream` from a `File` or
from a `ReadOnlySource` over the HTTP body, probes the container with an
extension hint (from the file name, or from the `Content-Type` header for
streams), picks the first audio track and creates a decoder.

`decode_next` returns interleaved **stereo** `f32`: mono is duplicated,
multichannel is downmixed (extra channels averaged into both sides).
`DecodeError`s (corrupt frames) are skipped; `ResetRequired` resets the
decoder; `UnexpectedEof` ends the track.

Duration comes from the container (`n_frames`), falling back to lofty's
estimate for files without one (VBR MP3 without a Xing header). Bitrate is
the codec's, else lofty's, else file size / duration.

## Resampling

The device may run at 48 kHz while files are 44.1 kHz. `Resampler` keeps a
small history buffer and does 4-point Catmull-Rom interpolation per channel.
It is reset on seek so no pre-seek samples bleed through. If the rates match
it is a no-op copy.

## Equalizer

`Equalizer` holds 10 peaking biquads per channel at Winamp's centre
frequencies (60 Hz ... 16 kHz), plus a preamp. Bands at 0 dB are bypassed.
Coefficients are rebuilt only when settings change; filter state carries
across rebuilds so slider drags don't click. Gains are ±12 dB like Winamp.

`parse_eqf` / `write_eqf` implement the `Winamp EQ library file v1.1` format
(257-byte name, 10 band bytes, 1 preamp byte; 0 = +12 dB, 63 = -12 dB), so
presets exchange with real Winamp installs. `builtin_presets` are the 18
presets Winamp shipped.

## Streaming

`stream::open` sends `Icy-MetaData: 1`. If the server answers with
`icy-metaint`, the body is wrapped in `IcyReader`, which strips the metadata
blocks every `metaint` bytes and parses `StreamTitle='...'` into a shared
`TitleSlot`. The decode thread polls that slot and emits
`Event::StreamTitle`. `icy-name`, `icy-genre`, `icy-br` populate
`StreamInfo`. Streams are not seekable; `OpenedTrack::seek` refuses.

## Visualisation support

The callback copies every output frame (post-EQ, post-volume, mixed to mono)
into a 4096-sample `VisRing`. `vis::spectrum` windows the last 512 samples,
runs the FFT and bins magnitudes into log-spaced bars with a dB-ish curve
tuned to look like Winamp's analyser. `vis::oscilloscope` picks `width`
evenly spaced samples from the last 576.

## Testing the engine without the UI

```
cargo run -p winamp-audio --example play -- song.mp3 8
cargo run -p winamp-audio --example play -- https://ice1.somafm.com/groovesalad-128-mp3 30
```

prints open info, position/RMS twice a second, seeks at t = 2 s and reports
events. Unit tests cover the ring buffer, resampler output count, EQ identity
at flat settings, EQF round-trip and ICY stripping.
