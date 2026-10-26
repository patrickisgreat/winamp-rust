# Developer documentation

Start here if you want to change the player rather than just use it.

| Document | What it covers |
|----------|----------------|
| [ARCHITECTURE.md](ARCHITECTURE.md) | The big picture: crates, data flow, threads, how a click becomes sound |
| [AUDIO.md](AUDIO.md) | The playback engine: decode thread, ring buffer, output callback, seeking, streaming |
| [UI.md](UI.md) | The skinned windows: drawing model, input model, menus, docking, dialogs, Milkdrop |
| [SKINS.md](SKINS.md) | Classic skin support, the sprite catalogue, and the procedural theme generator |
| [DEVELOPING.md](DEVELOPING.md) | Building, testing (unit, scripted UI, screenshots), debugging, adding features, packaging |

Conventions used throughout:

* "Skin pixels" are the coordinates of Winamp's original bitmaps (the main
  window is 275 x 116). Everything in the UI is positioned in skin pixels and
  scaled at draw time (1x, or 2x in double-size mode).
* "Points" are egui's logical units, which on a Retina display are half a
  physical pixel. Window positions and pointer positions arrive in points.
* A `Cmd` is a user-level command (play, open URL, toggle shuffle, ...).
  Buttons, menus and hotkeys all produce `Cmd`s; `App::exec` runs them.
