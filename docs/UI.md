# The user interface (`winamp-app`)

Winamp's windows are bitmaps with hot spots. The UI layer therefore does not
use egui widgets for the skinned windows at all; it blits sprites and does its
own hit testing. egui provides windows (viewports), input, textures and a
painter, and is used as a normal widget toolkit only for dialogs and menus.

Modules:

| Module | Role |
|--------|------|
| `main.rs` | eframe setup, the per-frame `Runner::update`, CLI dispatch |
| `cli.rs` | Argument parsing into `CliCommand` |
| `app.rs` | `App`: all state, `exec` (command dispatch), `poll`, playback helpers, viewport builders |
| `actions.rs` | The `Cmd` enum |
| `gfx.rs` | `Textures` (skin bitmaps as egui textures), `Gfx` (sprite blitting in skin pixels), `PixelSurface`, `Mouse` |
| `widgets.rs` | Button / slider interaction helpers and the shared window-drag logic |
| `main_window.rs`, `eq_window.rs`, `pl_window.rs` | One module per skinned window |
| `windows.rs` | Geometry: rects, focus, dragging, snapping, docking groups, restacking |
| `menu.rs` | Popup menus rendered as frameless viewports |
| `dialogs.rs` | Preferences, jump-to-file, jump-to-time, file info, URL, EQ presets, about |
| `hotkeys.rs` | Key → `Cmd` mapping and playlist keyboard navigation |
| `config.rs` | Persistent settings |
| `milkdrop.rs`, `vis_window.rs` | The Milkdrop-style visualiser and its window |
| `radio.rs` | Built-in internet radio station list |
| `headless.rs` | Software rasteriser for screenshots, and the scripted self-test |

## Drawing model

`Gfx` wraps an egui `Painter` with an origin and a scale. All coordinates are
skin pixels; `Gfx::rect` converts to points. `Gfx::sprite(sp, x, y)` draws a
`Sprite` (an image id plus a rectangle in that image, see
[SKINS.md](SKINS.md)) by issuing `painter.image` with the matching UV
rectangle. Textures are uploaded with nearest-neighbour filtering so 2x
(double size) stays crisp.

Text in the skinned windows uses the skin's bitmap font (`Gfx::text`, 5x6
cells from `text.bmp`), and the LCD digits use `numbers.bmp` /
`nums_ex.bmp`. Playlist rows are the one place egui's vector font is used,
sized from the config and coloured from `pledit.txt`.

Dynamic pixel content (the 76x16 analyser, the 113x19 EQ graph, the Milkdrop
frame) is drawn into a `PixelSurface`, a small `ColorImage` uploaded as a
texture each frame.

## Input model

Each window reads a `Mouse` snapshot at the start of its frame: pointer
position converted to skin pixels, pressed/down/released flags, modifiers,
scroll. Interaction state lives in `App::hot: Option<Hot>`, the one control
currently being pressed (a button, a slider, the marquee, a playlist row drag,
the scrollbar, the resize corner, or a window drag). `Hot::owner()` names the
window that owns it.

Rules that keep three windows from fighting over one `hot`:

* A control may only claim `hot` when it is `None` and the press happened
  over it.
* A window may only clear `hot` when it owns it and its own mouse button is
  up. (The original bug: every window cleared `hot` whenever *its* pointer was
  up, which was always true for the two windows you were not clicking in.)
* Commands are collected into a `Vec<Cmd>` during drawing and executed after
  the painter borrow ends.

`widgets::Widgets` provides `button` (press-and-release-inside semantics with
a pressed sprite while held), `hit_button` (invisible hit area for buttons
baked into the title bar), `hslider` and `vslider` (value 0..=1 from the
pointer, jump-on-click like Winamp).

## Windows, dragging, docking

The three skinned windows are separate OS windows without decorations:
the root viewport (main) plus two immediate child viewports (EQ, playlist).
`windows::Windows` tracks each one's OS-reported rect and focus.

Dragging works from any non-control pixel. `update_drag` computes the target
position from the OS-reported rect plus the pointer delta since the grab (so
it is self-correcting if the OS lags a frame), snaps to screen edges and to
the other windows within 10 px (hold Shift to disable), and moves companions
to fixed offsets captured at drag start.

Which windows move together: with "Lock windows together" (default on) all
visible windows; otherwise the windows that are touching the main window
transitively, Winamp-style. Size changes (double size, windowshade) call
`restack` so the stack stays attached. When the app regains focus the
companions are raised with a focus cascade, because macOS only raises the
window that was clicked.

Window positions are only applied through egui viewport commands; child
viewports get their initial position from the builder once and are then
moved by `OuterPosition` commands.

## Menus

Popup menus must extend beyond the 275x116 window, so each open level is its
own frameless, always-on-top viewport (`menu::Menu::show`). A `Menu` is a
tree of `Item::Action { cmd, checked, enabled }`, `Item::Sub`, `Item::Sep`.
Behaviour worth knowing:

* Submenus open on hover with a 120 ms delay and close with a 350 ms delay,
  so moving diagonally into a submenu across a neighbouring item does not
  close it.
* Dismissal: Escape, choosing an item, clicking on any Winamp window (the
  windows do this), or 1.5 s with no Winamp window focused and the pointer not
  over any menu level. Focus alone is not trusted because frameless popups do
  not reliably report it.
* Menu positions are screen points: window-relative positions are converted
  with the owning window's rect (`main_window::open_menu_at`).

## Dialogs

`dialogs::dialog` opens a decorated immediate viewport with a title and size
and runs a closure of ordinary egui widgets inside a `CentralPanel`. Returning
`false` from the closure, pressing Escape or the OS close button ends it. The
dialog state (text fields, selections) lives in `Dialogs` on `App`. Commands
raised by dialogs are executed after all dialogs are drawn.

File pickers are `rfd`'s blocking native dialogs, called from the UI thread
(the event loop pauses while they are open, exactly as Winamp did).

## Milkdrop

`milkdrop::Milkdrop` is a CPU feedback renderer. Each `step`:

1. **Analyse**: bass/mid/treble from the spectrum with running averages
   (`*_att` values are "how far above average"), RMS volume, beat detection.
2. **Warp pass**: for every destination pixel, compute a source coordinate
   (zoom, rotate, translate, per-preset warp field, optional kaleidoscope
   fold), bilinearly sample the previous frame, multiply by decay, optionally
   hue-rotate. Optional echo blends a rotated half-scale copy back in.
3. **Draw pass**: waveform or spectrum shapes in the preset's mode (circle,
   spiral, horizontal, dual lines, lissajous, spectrum ring, star) with
   cycling hues, plus a beat ring.
4. Swap buffers; `rgba()` tone-maps to bytes for the texture.

Presets are plain `Preset` structs with per-frame modulation coefficients
(`bass_zoom`, `treb_rot`, `mid_warp`). Adding a preset is adding an entry to
`presets()`. `vis_window.rs` owns the window, keyboard shortcuts, the preset
menu and the texture upload; it reads 1024 samples from the player each
frame.

## Hotkeys

`hotkeys::command_for_key(key, modifiers, playlist_focused)` is the complete
table and is unit tested. `handle` applies it to the events of whichever
window's viewport is being drawn, which is how shortcuts work from any
window. On macOS, Command is accepted wherever Ctrl is listed. The playlist
window additionally handles arrow/Home/End/Page navigation with Shift
extension directly.
