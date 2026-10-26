# winamp-rust

A rewrite of the classic Winamp 2.x player in Rust: the familiar three
skinned windows (main, equalizer, playlist editor), classic `.wsz` skin
support, a 10 band EQ, Shoutcast/Icecast streaming and the original keyboard
shortcuts. Audio decoding uses `symphonia`, output uses `cpal`, the UI is
`egui`/`eframe`.

```
cargo run --release                       # start the player
cargo run --release -- ~/Music            # add a folder and play
cargo run --release -- --install-theme DMT      # psychedelic geometry skin (also: Liminal)
cargo run --release -- --export-default-skin base-skin/
cargo run --release -- --screenshot shots/ song.mp3   # headless render of all windows
cargo run --release -- --selftest song.mp3 other.flac # scripted UI self-test
```

## Install

Prebuilt binaries are not published yet; build from source with a Rust
toolchain (1.88 or newer) from <https://rustup.rs>. All platforms:

```
git clone https://github.com/patrickisgreat/winamp-rust.git
cd winamp-rust
cargo install --path crates/winamp-app     # puts `winamp` in ~/.cargo/bin
winamp ~/Music                             # or just `winamp`
```

`cargo run --release` works from the checkout without installing.

### macOS (12 Monterey or newer, Intel and Apple Silicon)

No extra libraries are needed: audio goes through CoreAudio and the file
dialogs are native. To get a double-clickable `Winamp.app` with an icon and
file associations for audio files, playlists and `.wsz` skins:

```
scripts/make-app.sh                 # installs into /Applications
scripts/make-app.sh ~/Applications  # or a folder of your choice
```

If Xcode's command line tools are missing, run `xcode-select --install`
first (the script uses `sips` and `iconutil`).

### Linux

Install the development packages for ALSA (audio output), GTK 3 (file
dialogs) and the windowing libraries, then build as above.

Debian / Ubuntu:

```
sudo apt install build-essential pkg-config libasound2-dev libgtk-3-dev \
  libxkbcommon-dev libwayland-dev libxcb-render0-dev libxcb-shape0-dev \
  libxcb-xfixes0-dev
```

Fedora:

```
sudo dnf install gcc pkgconf-pkg-config alsa-lib-devel gtk3-devel \
  libxkbcommon-devel wayland-devel libxcb-devel
```

Arch:

```
sudo pacman -S --needed base-devel alsa-lib gtk3 libxkbcommon wayland libxcb
```

Both Wayland and X11 are supported. PipeWire and PulseAudio systems work
through their ALSA compatibility layer. Settings end up in
`~/.config/winamp-rust/Winamp/` (or `$XDG_CONFIG_HOME`).

### Windows (10 or newer)

Install Rust with the default `x86_64-pc-windows-msvc` toolchain, which also
prompts for the Visual Studio C++ build tools. Then, in a developer or regular
PowerShell:

```
cargo install --path crates\winamp-app
winamp
```

Audio uses WASAPI and no additional libraries are required. Release builds
start without a console window. Settings live in
`%APPDATA%\winamp-rust\Winamp\config\`.

### Where files live

| Platform | Settings and playlist | Skins folder |
|----------|----------------------|--------------|
| macOS | `~/Library/Application Support/org.winamp-rust.Winamp/` | `.../Skins/` |
| Linux | `~/.config/winamp-rust/Winamp/` | `.../Skins/` |
| Windows | `%APPDATA%\winamp-rust\Winamp\config\` | `...\Skins\` |

Drop classic `.wsz` / `.zip` skins (or unpacked skin folders) into the skins
folder; Preferences > Skins has an "Open skins folder" button.

### Developing

```
cargo test --workspace                       # unit tests for every crate
cargo clippy --workspace --all-targets       # lint (the tree is kept warning-free)
cargo fmt --all                              # format
cargo run -- --selftest a.wav b.wav c.wav    # scripted UI check, needs 3+ audio files
```

## Features

* Main window: transport buttons, LCD time (elapsed/remaining, blinks when
  paused), scrolling title marquee (drag to scroll), bitrate / sample rate /
  mono-stereo indicators, spectrum analyzer (bars or lines, with peaks) and
  oscilloscope, volume and balance sliders, seek bar, shuffle/repeat, EQ/PL
  toggles, clutter bar (O A I D V), windowshade mode, double size, always on
  top, right-click menu.
* Equalizer: on/auto, preamp, 10 bands at Winamp's frequencies, spline graph,
  presets dialog with the 18 stock presets, user presets, `.eqf`/`.q1` import
  and export, per-file auto presets, windowshade mode.
* Playlist editor: resizable in Winamp's tile steps, click / shift / ctrl
  selection, drag reorder, keyboard navigation, scrollbar, ADD/REM/SEL/MISC/LIST
  menus (files, folders, URLs, dead-file and duplicate removal, crop, sorting,
  randomize), M3U / M3U8 / PLS load and save, running-time display,
  windowshade mode, file info.
* Skins: classic `.wsz`/`.zip` or unpacked folders, with `pledit.txt` and
  `viscolor.txt`. Missing bitmaps fall back to the built-in skin. A procedural
  base skin plus the DMT, Liminal and No Man's Sky themes are generated at runtime; `--install-theme`
  writes a real `.wsz` into the skins folder.
* Windows snap to each other and to the screen edges; EQ and playlist docked
  to the main window move with it, and "Lock windows together" (default on)
  moves all windows as one. Shift while dragging disables snapping.
* Formats: MP3, MP2, AAC/M4A, FLAC, Ogg Vorbis, WAV, AIFF, ALAC, audio in
  MKV/WebM. HTTP/HTTPS streams with ICY "now playing" titles; remote `.pls`
  and `.m3u` are expanded.
* Settings, window positions and the playlist persist in the OS config
  directory (`winamp.toml`, `winamp.m3u8`). Skins live in `Skins/` next to them.
* Drag and drop files or folders onto any window.

## Keyboard

Z/X/C/V/B transport, L open files, Shift+L open folder, Ctrl+L open URL,
J jump to file, Ctrl+J jump to time, arrows seek/volume, R repeat, S shuffle,
Alt+W/E/G toggle main/playlist/EQ, Alt+3 file info, Alt+S skins, Ctrl+P
preferences, Ctrl+T time mode, Ctrl+A always on top, Ctrl+D double size,
Ctrl+W windowshade, Ctrl+V visualization mode. The full list is in
Preferences > Hotkeys. On macOS the Command key works wherever Ctrl is listed.

## Not (yet) implemented

* Win32 plugin binaries (`in_*.dll`, `out_*.dll`, `vis_*.dll`, `dsp_*.dll`,
  `gen_*.dll`), including AVS and MilkDrop. Decoders are built in instead.
* Modern (Winamp 3/5) skins, the Winamp 5 media library, CD playback and
  ripping, the mini-browser, `region.txt` non-rectangular windows, skin
  cursors, tag editing in the file info dialog.

## Developer docs

Architecture, the audio engine, the UI model, skins and the development
workflow are documented in [`docs/`](docs/README.md).

## Layout

* `crates/winamp-audio`: decoding, streaming, resampling, EQ, output, FFT.
* `crates/winamp-playlist`: playlist model and M3U/PLS parsing.
* `crates/winamp-skin`: skin loading, sprite catalogue, procedural themes.
* `crates/winamp-app`: the eframe application, windows, dialogs, hotkeys,
  headless renderer and self-test.

## macOS app bundle

`scripts/make-app.sh` builds the release binary, renders an icon, wraps
everything in `Winamp.app` (with file associations for audio, playlists and
`.wsz` skins) and installs it into `/Applications` (or `~/Applications`).
