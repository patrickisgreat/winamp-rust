# Skins (`winamp-skin`)

Winamp 2.x "classic" skins are a zip archive (`.wsz`) of Windows bitmaps with
fixed names and fixed sprite layouts, plus two text files. This crate loads
them, fills gaps the way Winamp did, and can also generate complete skins
procedurally from a `Theme`.

## Files in a classic skin

| File | Size | Contents |
|------|------|----------|
| `main.bmp` | 275x116 | Main window background |
| `titlebar.bmp` | 344x87 | Title bars (active/inactive, normal/shade), title buttons, clutter bar, shade-mode position bar |
| `cbuttons.bmp` | 136x36 | Prev/play/pause/stop/next/eject, normal and pressed |
| `shufrep.bmp` | 92x85 | Shuffle/repeat (4 states each), EQ/PL toggles |
| `posbar.bmp` | 307x10 | Seek bar background and thumb |
| `volume.bmp` | 68x433 | 28 volume backgrounds (one per level) and the thumb |
| `balance.bmp` | 47x433 | 28 balance backgrounds (by deviation from centre) and thumb; Winamp substitutes `volume.bmp` if missing |
| `monoster.bmp` | 56x24 | STEREO / MONO indicators, lit and unlit |
| `playpaus.bmp` | 42x9 | Play/pause/stop indicator and the "working" dash |
| `numbers.bmp` | 99x13 | LCD digits 0-9 and a blank |
| `nums_ex.bmp` | 108x13 | Same plus a minus sign; synthesised from `numbers.bmp` if missing |
| `text.bmp` | 155x18 | 5x6 bitmap font, 3 rows of 31 cells |
| `eqmain.bmp` | 275x315 | EQ window, buttons, 28 slider backgrounds, thumb, graph background and the graph colour column |
| `eq_ex.bmp` | 275x57 | EQ shade mode; synthesised from `eqmain.bmp` if missing |
| `pledit.bmp` | 280x186 | Playlist frame tiles, corners, scrollbar, shade mode |
| `pledit.txt` | | `Normal`, `Current`, `NormalBG`, `SelectedBG`, `Font` |
| `viscolor.txt` | | 24 RGB lines: background, grid, 16 analyser rows (top to bottom), 5 oscilloscope colours, peak colour |

Skin archives are matched by lowercase base name, so nested folders and
mixed case (very common) are fine. PNG inside a skin is accepted too.

`Skin::load` returns a `Skin` whose `images` map always contains every
`SkinImage`: anything missing is taken from the built-in skin, so the UI never
has to handle absence. `Skin::provided` lists what really came from the file.

## The sprite catalogue (`sprites.rs`)

A `Sprite` is `{ img: SkinImage, x, y, w, h }`. `sprites.rs` is the single
source of truth for where everything is, both inside the bitmaps
(`BTN_PLAY = cbuttons.bmp @ 23,0 23x18`) and inside the windows
(`POS_BTN_PLAY = (39, 88)`). Travel distances for sliders (`VOLUME_TRAVEL =
51`, `EQ_SLIDER_TRAVEL = 51`, `POSBAR_TRAVEL = 219`) and frame-indexed
sprites (`volume_bg(frame)`, `eq_slider_bg(frame)`) live here too, as does
`font_lookup(char) -> (row, col)` for `text.bmp`.

If a sprite looks wrong in a third-party skin, this file is where to compare
against the community layout documentation.

## Procedural themes (`default.rs`, `themes.rs`)

The crate ships no Nullsoft bitmaps. Instead `default::generate_theme(&Theme)`
draws every bitmap from scratch with a small pixel toolkit (`Canvas`: fills,
bevels, panels, a 3x5 mini font, the 5x6 font, seven-segment digits,
pattern fills). The base skin is `BASE_THEME`; `themes.rs` adds DMT, Liminal
and No Man's Sky.

A `Theme` is a plain `const` struct:

| Field group | Purpose |
|-------------|---------|
| `bg`, `bg2`, `light`, `lighter`, `dark`, `face`, `face_down` | Chrome colours and bevels |
| `green`, `dim_green`, `lcd` | LCD foreground (lit / unlit) and background |
| `label`, `label_dim`, `title_color`, `accent` | Text and highlight colours |
| `grad_lo`, `grad_mid`, `grad_hi` | Gradients for volume/balance/EQ slider fills |
| `pattern: Option<fn(x, y, dim) -> [u8; 3]>` | Background artwork as a function of window coordinates |
| `title_main`, `title_eq`, `title_pl`, `emblem` | Title bar text and an optional 9x9 emblem |
| `flat` | HUD style: translucent chamfered panels instead of bevels |
| `decorate: Option<fn(&mut Canvas, SkinImage)>` | Final overlay pass per bitmap (logos, wordmarks) |
| `pledit_txt`, `viscolor_txt` | The two text files, verbatim |

Pattern functions receive window coordinates, not bitmap coordinates:
`Canvas::origin` is set while drawing sprites that live elsewhere in their
sheet (title bars are at x = 27 in `titlebar.bmp` but x = 0 in the window) so
artwork lines up across sprites. On patterned themes small labels get a
backing rectangle in `bg` for legibility.

### Adding a theme

1. Write a `pub const MY_THEME: Theme = Theme { .. }` in `themes.rs`
   (copy an existing one). Optionally write a pattern function and a
   `decorate` function.
2. Add it to `themes::all()`.
3. `cargo run -- --install-theme "My Theme"` writes `MyTheme.wsz` into the
   skins folder and selects it. `cargo run -- --screenshot dir song.mp3`
   renders all three windows so you can check it without launching the app.

The `.wsz` writer (`themes::write_wsz`) produces real Winamp-compatible
archives (24-bit BMPs), so generated themes also load in other classic-skin
players.

## Runtime use in the app

`gfx::Textures::from_skin` uploads each `SkinImage` as one egui texture. The
UI only ever draws sub-rectangles of those, so loading a different skin is a
matter of replacing `App::skin` and `App::textures`; no layout code changes.
`Skin::pixel` is used for the EQ graph colour column, and `Skin::viscolors`
and `Skin::pledit` drive the analyser and playlist colours.
