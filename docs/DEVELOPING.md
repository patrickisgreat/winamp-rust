# Developing

## Build and run

```
cargo run --release                      # the player
cargo run --release -- ~/Music           # add a folder and start playing
cargo run --release -- https://ice1.somafm.com/groovesalad-128-mp3
scripts/make-app.sh                      # macOS: build Winamp.app and install to /Applications
```

The dev profile compiles dependencies at `opt-level = 3` and our crates at
`opt-level = 1`, which keeps debug builds usable for audio and the CPU
visualiser. Milkdrop is noticeably smoother in release.

Toolchain: stable Rust (edition 2021). No system libraries beyond what cpal
needs (CoreAudio on macOS, ALSA on Linux, WASAPI on Windows).

## Command line

| Flag | Effect |
|------|--------|
| `FILE DIR URL ...` | Add to the playlist and play the first new entry |
| `--install-theme NAME` | Generate a built-in theme as `.wsz`, install into the skins folder, select it |
| `--export-default-skin DIR` | Write the base skin as BMPs plus `pledit.txt` / `viscolor.txt` |
| `--screenshot DIR [files]` | Headless render of main, EQ and playlist windows plus Milkdrop presets to PNGs |
| `--selftest [files]` | Scripted UI self-test; exits non-zero on failure |
| `--help` | Usage |

Parsing is in `cli.rs` (`CliCommand`), with tests.

## Tests

```
cargo test --workspace                   # unit tests (all crates)
cargo clippy --workspace --all-targets   # must be warning-free
cargo fmt --all --check
cargo run -- --selftest a.mp3 b.flac c.m4a d.aiff   # needs 4 playable files
```

Generate test media with ffmpeg if you have none:

```
ffmpeg -f lavfi -i "sine=frequency=440:duration=12" -ac 2 -metadata title=Tone -metadata artist=Test a.mp3
```

### The scripted self-test

`headless::selftest` builds the real `App` against a headless `egui::Context`,
then drives it with synthetic `egui::Event`s (pointer moves, presses with
modifiers, key presses) at skin-pixel coordinates and asserts on `App` state.
It covers transport buttons, toggles, sliders, the seek bar, the clutter bar,
double size, menus opening, keyboard shortcuts, EQ sliders and buttons,
playlist selection (click, shift, ctrl), double-click to play, drag reorder,
keyboard navigation, delete, bottom buttons, resize and shade.

Every scripted frame also runs the other visible windows, exactly like the
real app does, because the nastiest bugs so far were cross-window state
interactions. When you fix a UI bug, add a step here; a step is usually three
lines (`d.click(...)`, `d.check(...)`).

Timing matters for a few checks: engine commands are asynchronous, so the
test calls `settle()` before asserting on `PlayState`, and it advances the
synthetic clock by a second before a double-click so an earlier click is not
counted as the first half of a triple-click.

### Screenshots

`headless::run` renders through a small software rasteriser (`rasterize`)
that tessellates egui's shapes and samples textures with nearest filtering.
It exists because the project was built on a machine where screen capture
was unavailable; it is also handy in CI. Output lands in the given directory
as `main.png`, `eq.png`, `pl.png`, `milkdrop-N.png`.

## Debugging

* `RUST_LOG=debug cargo run` enables `log` output (symphonia is chatty at
  `info`; `warn` is the default).
* `WINAMP_DEBUG=1` prints every executed `Cmd` and every engine `Event` to
  stderr, which is the quickest way to see why a track advanced or stopped.
* `cargo run -p winamp-audio --example play -- FILE_OR_URL SECONDS` exercises
  decoding, seeking and streaming without a window.
* Panics in the GUI go to stderr; when launched from Finder they are lost.
  Run `/Applications/Winamp.app/Contents/MacOS/winamp` from a terminal with
  `RUST_BACKTRACE=1` to see them.

## Adding things

**A command.** Add a variant to `actions::Cmd`, handle it in `App::exec`.
Then expose it: a `button`/`hit_button` in a window module, an `Item` in a
menu, and/or a row in `hotkeys::command_for_key` (and its `HELP` table that
Preferences shows).

**A dialog.** Add state to `dialogs::Dialogs`, a block in
`dialogs::show_all` using `dialog(ctx, id, title, size, |ctx, ui| ...)`,
and a `Cmd` that sets the state.

**A setting.** Add a field with a default to `config::Config` (it is
`#[serde(default)]`), read it where needed, add a control in the Preferences
tab in `dialogs.rs`.

**A skin theme.** See [SKINS.md](SKINS.md).

**A Milkdrop preset.** Add a `Preset` to `milkdrop::presets()`; render it
with `--screenshot` to check it before launching the app.

**A radio station.** Add it to `radio::GROUPS`.

**A decoder.** symphonia is enabled with `features = ["all"]`. Formats it
lacks (Opus, WMA, AC3) need an external decoder wrapped in a symphonia
`Decoder` impl and registered in `decode::OpenedTrack::from_mss`.

## Code conventions

* Lower crates never depend on higher ones; `winamp-skin` and
  `winamp-audio` do not know about each other.
* No egui types below `winamp-app`.
* Window modules collect `Cmd`s while drawing and execute them afterwards;
  do not call `App::exec` while a `Gfx` borrow is alive.
* Nothing may block in the audio callback (`try_lock` only, no allocation in
  the steady state).
* All positions in UI code are skin pixels unless the variable is explicitly
  in points (window rects, menu positions).
* Keep `cargo clippy --workspace --all-targets` clean and `cargo fmt` applied.

## Packaging

`scripts/make-app.sh` builds release, renders the icon
(`crates/winamp-skin/examples/icon.rs`), assembles `Winamp.app` with an
`Info.plist` declaring audio, playlist and `.wsz` document types, ad-hoc
signs it and copies it to `/Applications` (or `~/Applications` if that is not
writable). Re-run it after any change you want in the installed app.

User data lives in the OS config directory:
`~/Library/Application Support/org.winamp-rust.Winamp/` on macOS
(`winamp.toml`, `winamp.m3u8`, `Skins/`).
