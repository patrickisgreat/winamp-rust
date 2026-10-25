//! Keyboard shortcuts, matching Winamp 2.x defaults where possible.

use egui::Key;

use crate::actions::Cmd;
use crate::app::App;
use crate::windows::Win;

pub const HELP: &[(&str, &str)] = &[
    ("Z / X / C / V / B", "Previous / Play / Pause / Stop / Next"),
    ("L", "Open file(s)"),
    ("Shift+L", "Open folder"),
    ("Ctrl+L", "Open location (URL)"),
    ("J", "Jump to file"),
    ("Ctrl+J", "Jump to time"),
    ("Left / Right", "Seek 5 seconds back / forward"),
    ("Up / Down", "Volume up / down"),
    ("R", "Toggle repeat"),
    ("S", "Toggle shuffle"),
    ("Alt+W", "Toggle main window"),
    ("Alt+E", "Toggle playlist editor"),
    ("Alt+G", "Toggle equalizer"),
    ("Alt+3", "File info"),
    ("Alt+S", "Skin browser"),
    ("Ctrl+P", "Preferences"),
    ("Ctrl+T", "Toggle time elapsed / remaining"),
    ("Ctrl+A", "Toggle always on top"),
    ("Ctrl+D", "Toggle double size"),
    ("Ctrl+W", "Toggle main windowshade"),
    ("Ctrl+Shift+W", "Toggle EQ windowshade"),
    ("Ctrl+Alt+W", "Toggle playlist windowshade"),
    ("Ctrl+V", "Cycle visualization mode"),
    ("Alt+F4 / Cmd+Q", "Exit"),
    ("Playlist: Enter", "Play selected"),
    ("Playlist: Delete", "Remove selected"),
    ("Playlist: Ctrl+A", "Select all"),
    ("Playlist: Alt+Up/Down", "Move selected up / down"),
];

pub fn handle(app: &mut App, ctx: &egui::Context, win: Win) {
    let mut cmds: Vec<Cmd> = Vec::new();
    let pl_focus = win == Win::Pl;
    ctx.input(|i| {
        let mods = i.modifiers;
        let alt = mods.alt;
        let ctrl = mods.ctrl || mods.mac_cmd || mods.command;
        let shift = mods.shift;
        let plain = !alt && !ctrl;
        for ev in &i.events {
            if let egui::Event::Key { key, pressed: true, modifiers, .. } = ev {
                let _ = modifiers;
                let k = *key;
                let cmd = match (k, alt, ctrl, shift) {
                    (Key::Z, false, false, _) => Some(Cmd::Prev),
                    (Key::X, false, false, _) => Some(Cmd::Play),
                    (Key::C, false, false, _) => Some(Cmd::Pause),
                    (Key::V, false, false, _) => Some(Cmd::Stop),
                    (Key::V, false, true, _) => Some(Cmd::CycleVisMode),
                    (Key::B, false, false, _) => Some(Cmd::Next),
                    (Key::L, false, false, false) => Some(Cmd::OpenFiles),
                    (Key::L, false, false, true) => Some(Cmd::OpenDirectory),
                    (Key::L, false, true, _) => Some(Cmd::OpenUrl),
                    (Key::J, false, false, _) => Some(Cmd::JumpToFile),
                    (Key::J, false, true, _) => Some(Cmd::JumpToTime),
                    (Key::R, false, false, _) => Some(Cmd::ToggleRepeat),
                    (Key::S, false, false, _) => Some(Cmd::ToggleShuffle),
                    (Key::S, true, false, _) => Some(Cmd::SkinBrowser),
                    (Key::W, true, false, _) => Some(Cmd::ToggleMain),
                    (Key::E, true, false, _) => Some(Cmd::TogglePlaylist),
                    (Key::G, true, false, _) => Some(Cmd::ToggleEq),
                    (Key::Num3, true, false, _) => Some(if pl_focus { Cmd::PlFileInfo } else { Cmd::FileInfo }),
                    (Key::P, false, true, _) => Some(Cmd::Preferences),
                    (Key::T, false, true, _) => Some(Cmd::ToggleTimeMode),
                    (Key::A, false, true, _) if !pl_focus => Some(Cmd::ToggleAlwaysOnTop),
                    (Key::A, false, true, _) => Some(Cmd::PlSelectAll),
                    (Key::D, false, true, _) => Some(Cmd::ToggleDoubleSize),
                    (Key::W, false, true, false) => Some(Cmd::ToggleMainShade),
                    (Key::W, false, true, true) => Some(Cmd::ToggleEqShade),
                    (Key::W, true, true, _) => Some(Cmd::TogglePlShade),
                    (Key::Q, false, true, _) => Some(Cmd::Exit),
                    (Key::F4, true, _, _) => Some(Cmd::Exit),
                    (Key::ArrowLeft, false, _, _) if plain => Some(Cmd::SeekRelative(-5.0)),
                    (Key::ArrowRight, false, _, _) if plain => Some(Cmd::SeekRelative(5.0)),
                    (Key::ArrowUp, false, false, _) if !pl_focus => Some(Cmd::VolumeDelta(0.02)),
                    (Key::ArrowDown, false, false, _) if !pl_focus => Some(Cmd::VolumeDelta(-0.02)),
                    (Key::ArrowUp, true, false, _) if pl_focus => Some(Cmd::PlMoveSelected(-1)),
                    (Key::ArrowDown, true, false, _) if pl_focus => Some(Cmd::PlMoveSelected(1)),
                    (Key::Enter, false, false, _) if pl_focus => Some(Cmd::PlPlaySelected),
                    (Key::Delete, false, false, _) | (Key::Backspace, false, false, _) if pl_focus => Some(Cmd::PlRemoveSelected),
                    (Key::Space, false, false, _) => Some(Cmd::Pause),
                    _ => None,
                };
                if let Some(c) = cmd {
                    cmds.push(c);
                }
            }
        }
    });
    // Playlist keyboard navigation (selection with arrows).
    if pl_focus {
        let (up, down, home, end, pgup, pgdn, shift) = ctx.input(|i| {
            let plain = !i.modifiers.alt && !i.modifiers.command;
            (
                plain && i.key_pressed(Key::ArrowUp),
                plain && i.key_pressed(Key::ArrowDown),
                i.key_pressed(Key::Home),
                i.key_pressed(Key::End),
                i.key_pressed(Key::PageUp),
                i.key_pressed(Key::PageDown),
                i.modifiers.shift,
            )
        });
        if up || down || home || end || pgup || pgdn {
            let n = app.playlist.len();
            if n > 0 {
                let rows = app.pl_visible_rows() as i64;
                let sel = app.playlist.selected_indices();
                let cur = sel.first().copied().or(app.playlist.current).unwrap_or(0) as i64;
                let last = *sel.last().unwrap_or(&(cur as usize)) as i64;
                let target = if up {
                    cur - 1
                } else if down {
                    last + 1
                } else if home {
                    0
                } else if end {
                    n as i64 - 1
                } else if pgup {
                    cur - rows
                } else {
                    last + rows
                }
                .clamp(0, n as i64 - 1) as usize;
                if shift {
                    let anchor = app.pl_anchor.unwrap_or(cur as usize);
                    let (a, b) = if anchor <= target { (anchor, target) } else { (target, anchor) };
                    for (k, e) in app.playlist.entries.iter_mut().enumerate() {
                        e.selected = k >= a && k <= b;
                    }
                } else {
                    app.playlist.select_none();
                    app.playlist.entries[target].selected = true;
                    app.pl_anchor = Some(target);
                }
                let rows = rows as usize;
                if target < app.pl_scroll {
                    app.pl_scroll = target;
                } else if target >= app.pl_scroll + rows {
                    app.pl_scroll = target + 1 - rows;
                }
            }
        }
    }
    for c in cmds {
        app.exec(c, ctx);
    }
}
