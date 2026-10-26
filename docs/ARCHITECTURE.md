# Architecture

winamp-rust is a reimplementation of the classic Winamp 2.x player. It is a
Cargo workspace of four crates, arranged so that everything below the UI is a
plain library with no knowledge of windows, egui or skins.

```
                 +----------------------------------------------------+
                 |  winamp-app (binary "winamp")                      |
                 |  eframe/egui windows, dialogs, menus, hotkeys,     |
                 |  config, Milkdrop, radio list, headless test tools |
                 +------+-------------------+-------------------+-----+
                        |                   |                   |
             +----------v-----+   +---------v--------+   +------v--------------+
             | winamp-playlist|   |  winamp-skin     |   |  winamp-audio       |
             | entries, M3U/  |   |  .wsz loading,   |   |  decode, stream,    |
             | PLS, shuffle,  |   |  sprite table,   |   |  resample, EQ,      |
             | sort, reorder  |   |  theme generator |   |  output, FFT        |
             +----------+-----+   +------------------+   +------+--------------+
                        |                                       |
                        +---------------------------------------+
                          (playlist depends on audio for Source
                           and tag reading only)
```

| Crate | Depends on | Role |
|-------|------------|------|
| `winamp-audio` | symphonia, cpal, lofty, ureq | Turns a file or URL into samples on the speakers. Owns all threads. |
| `winamp-playlist` | winamp-audio | The list of things to play and how to move through it. Pure data; no I/O beyond reading playlist files and tags. |
| `winamp-skin` | image, zip | Loads classic skins and generates the built-in ones. Knows the pixel layout of every sprite. No egui. |
| `winamp-app` | all of the above, eframe/egui, rfd | Everything the user sees. |

The dependency direction is strict: lower crates never reference higher ones,
and `winamp-skin` and `winamp-audio` don't know about each other.

## One frame of the application

The app is an `eframe::App`. Every frame (`Runner::update` in `main.rs`) does
the following, in order:

1. **Observe the main window.** `Windows::observe` records the OS-reported
   rect and focus state of the root viewport.
2. **Poll.** `App::poll` drains events from the audio engine (track opened,
   finished, error, stream title), applies tag-reader results to playlist
   entries, handles files dropped onto windows, and autosaves every minute.
3. **Analyse audio.** `App::update_vis` pulls the most recent samples from the
   engine and updates the spectrum bars and peak-hold values.
4. **Draw the main window** (`main_window::show`), then run hotkeys for it.
5. **Draw the child windows.** For each visible one of EQ and playlist, call
   `ctx.show_viewport_immediate` with a builder from `App::child_builder` and
   draw inside it. These are real OS windows; egui diffs the builder against
   last frame and only sends commands for fields that changed.
6. **Draw Milkdrop** if open (also an immediate viewport).
7. **Show the active popup menu** (`main_window::show_menu`), which is made of
   one frameless viewport per open level.
8. **Show dialogs** (`dialogs::show_all`): decorated viewports with ordinary
   egui widgets.
9. **Request the next repaint** (33 ms, or 16 ms while Milkdrop is open).

Everything mutable lives in one `App` struct (`app.rs`). Window modules take
`&mut App` and a `&egui::Context`; there is no shared-state plumbing beyond
that.

## Commands

User intent is expressed as `actions::Cmd`, a plain enum. Three producers:

* skinned buttons and sliders in the window modules,
* popup menus (`menu::Item` carries a `Cmd`),
* keyboard shortcuts (`hotkeys::command_for_key` is a pure mapping from key +
  modifiers to `Cmd`, so it is unit-tested without a window).

One consumer: `App::exec(cmd, ctx)`. Window modules collect commands into a
`Vec<Cmd>` while drawing and execute them after the borrow of the painter
ends. This keeps the borrow checker happy and gives every behaviour a single
place to live. If you want to know what "Next" does, read `Cmd::Next` in
`App::exec`; it is the same whether it came from a button, a menu or the B
key.

## Data flow for playback

```
 user clicks Play on track 3
   -> Cmd::PlayIndex(3)
   -> App::play_index: playlist.set_current(3); player.load(source, autoplay)
   -> engine decode thread opens the source (symphonia), emits Event::Opened
   -> decode thread fills the ring buffer; cpal callback drains it, applying
      EQ, volume and balance, and copies samples into the visualiser ring
   -> App::poll sees Event::Opened: fills in duration/title, sets window title
   -> at end of track the callback flags "drained"; decode thread emits
      Event::Finished; App::poll calls next_track(auto = true)
```

The engine never touches the playlist. The app decides what plays next, using
`Playlist::next_index(shuffle, repeat)`.

## Threads

| Thread | Owner | Purpose |
|--------|-------|---------|
| main | eframe | UI, command dispatch, config, menus |
| `winamp-output` | `winamp-audio::engine` | Owns the cpal stream for its whole life; the audio callback runs on cpal's realtime thread |
| `winamp-decode` | `winamp-audio::engine` | Opens sources, decodes packets, resamples, pushes into the ring buffer, handles seek/stop |
| `winamp-tags` | `winamp-app::app` | Reads metadata for playlist entries off the UI thread |

Communication is through `std::sync::mpsc` channels (commands in, events out),
atomics in a shared struct for hot values (position, volume, state), and a
lock-free SPSC ring buffer for samples. See [AUDIO.md](AUDIO.md).

## Persistence

Settings are a single TOML file (`config::Config`, serde), stored in the OS
config directory (`~/Library/Application Support/org.winamp-rust.Winamp` on
macOS). The playlist is saved beside it as `winamp.m3u8`, skins go into a
`Skins/` subfolder. `Config` has `#[serde(default)]`, so adding a field never
breaks old files; give it a sensible default and it just appears.

## Testing strategy

There are three layers (details in [DEVELOPING.md](DEVELOPING.md)):

* **Unit tests** in every crate for pure logic: EQ maths, EQF files, ring
  buffer, resampler, M3U/PLS parsing, shuffle order, skin text files, sprite
  table, hotkey mapping, snapping.
* **A scripted UI self-test** (`winamp --selftest files...`) that drives the
  real window code with synthetic pointer and key events through a headless
  egui context and asserts on application state. It catches the class of bug
  that only appears when three windows share state.
* **Headless screenshots** (`winamp --screenshot dir files...`) that render
  the windows through a small software rasteriser so layout changes can be
  eyeballed without a display.

## What is deliberately not here

* No Win32 plugin ABI. Winamp's `in_*.dll` / `vis_*.dll` plugins are replaced
  by built-in decoders (symphonia) and a built-in Milkdrop-style visualiser.
* No modern (Winamp 3/5) skins or media library.
* No GPU rendering of the visualiser; it is CPU-rendered at 320x240 and
  scaled, which keeps it testable headlessly and fast enough in release builds.
