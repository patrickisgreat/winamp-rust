//! Sprite coordinates for Winamp 2.x classic skins.
//!
//! Coordinates follow the layout documented by the Winamp skinning community
//! (and used by Webamp). Positions marked `POS_*` are where a sprite is drawn
//! inside its window.

use crate::SkinImage;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Sprite {
    pub img: SkinImage,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

const fn s(img: SkinImage, x: u32, y: u32, w: u32, h: u32) -> Sprite {
    Sprite { img, x, y, w, h }
}

pub const MAIN_W: u32 = 275;
pub const MAIN_H: u32 = 116;
pub const SHADE_H: u32 = 14;
pub const EQ_W: u32 = 275;
pub const EQ_H: u32 = 116;
pub const PL_MIN_W: u32 = 275;
pub const PL_MIN_H: u32 = 116;

use SkinImage::*;

// --- main window ----------------------------------------------------------
pub const MAIN_BG: Sprite = s(Main, 0, 0, 275, 116);

pub const TITLE_ACTIVE: Sprite = s(Titlebar, 27, 0, 275, 14);
pub const TITLE_INACTIVE: Sprite = s(Titlebar, 27, 15, 275, 14);
pub const TITLE_SHADE_ACTIVE: Sprite = s(Titlebar, 27, 29, 275, 14);
pub const TITLE_SHADE_INACTIVE: Sprite = s(Titlebar, 27, 42, 275, 14);

pub const BTN_OPTIONS: Sprite = s(Titlebar, 0, 0, 9, 9);
pub const BTN_OPTIONS_DOWN: Sprite = s(Titlebar, 0, 9, 9, 9);
pub const BTN_MINIMIZE: Sprite = s(Titlebar, 9, 0, 9, 9);
pub const BTN_MINIMIZE_DOWN: Sprite = s(Titlebar, 9, 9, 9, 9);
pub const BTN_SHADE: Sprite = s(Titlebar, 0, 18, 9, 9);
pub const BTN_SHADE_DOWN: Sprite = s(Titlebar, 9, 18, 9, 9);
pub const BTN_CLOSE: Sprite = s(Titlebar, 18, 0, 9, 9);
pub const BTN_CLOSE_DOWN: Sprite = s(Titlebar, 18, 9, 9, 9);
pub const BTN_UNSHADE: Sprite = s(Titlebar, 0, 27, 9, 9);
pub const BTN_UNSHADE_DOWN: Sprite = s(Titlebar, 9, 27, 9, 9);
pub const POS_BTN_OPTIONS: (u32, u32) = (6, 3);
pub const POS_BTN_MINIMIZE: (u32, u32) = (244, 3);
pub const POS_BTN_SHADE: (u32, u32) = (254, 3);
pub const POS_BTN_CLOSE: (u32, u32) = (264, 3);

pub const SHADE_POS_BG: Sprite = s(Titlebar, 0, 36, 17, 7);
pub const SHADE_POS_THUMB_LEFT: Sprite = s(Titlebar, 17, 36, 3, 7);
pub const SHADE_POS_THUMB: Sprite = s(Titlebar, 20, 36, 3, 7);
pub const SHADE_POS_THUMB_RIGHT: Sprite = s(Titlebar, 23, 36, 3, 7);
pub const POS_SHADE_POS: (u32, u32) = (226, 4);

pub const CLUTTER_BAR: Sprite = s(Titlebar, 304, 0, 8, 43);
pub const CLUTTER_BAR_DISABLED: Sprite = s(Titlebar, 312, 0, 8, 43);
pub const CLUTTER_O_DOWN: Sprite = s(Titlebar, 304, 47, 8, 8);
pub const CLUTTER_A_DOWN: Sprite = s(Titlebar, 312, 55, 8, 7);
pub const CLUTTER_I_DOWN: Sprite = s(Titlebar, 320, 62, 8, 7);
pub const CLUTTER_D_DOWN: Sprite = s(Titlebar, 328, 69, 8, 8);
pub const CLUTTER_V_DOWN: Sprite = s(Titlebar, 336, 77, 8, 7);
pub const POS_CLUTTER: (u32, u32) = (10, 22);
/// Y offsets (within the clutter bar) of the O, A, I, D, V buttons.
pub const CLUTTER_ROWS: [(u32, u32); 5] = [(0, 8), (8, 7), (15, 7), (22, 8), (30, 7)];

pub const BTN_PREV: Sprite = s(CButtons, 0, 0, 23, 18);
pub const BTN_PREV_DOWN: Sprite = s(CButtons, 0, 18, 23, 18);
pub const BTN_PLAY: Sprite = s(CButtons, 23, 0, 23, 18);
pub const BTN_PLAY_DOWN: Sprite = s(CButtons, 23, 18, 23, 18);
pub const BTN_PAUSE: Sprite = s(CButtons, 46, 0, 23, 18);
pub const BTN_PAUSE_DOWN: Sprite = s(CButtons, 46, 18, 23, 18);
pub const BTN_STOP: Sprite = s(CButtons, 69, 0, 23, 18);
pub const BTN_STOP_DOWN: Sprite = s(CButtons, 69, 18, 23, 18);
pub const BTN_NEXT: Sprite = s(CButtons, 92, 0, 22, 18);
pub const BTN_NEXT_DOWN: Sprite = s(CButtons, 92, 18, 22, 18);
pub const BTN_EJECT: Sprite = s(CButtons, 114, 0, 22, 16);
pub const BTN_EJECT_DOWN: Sprite = s(CButtons, 114, 16, 22, 16);
pub const POS_BTN_PREV: (u32, u32) = (16, 88);
pub const POS_BTN_PLAY: (u32, u32) = (39, 88);
pub const POS_BTN_PAUSE: (u32, u32) = (62, 88);
pub const POS_BTN_STOP: (u32, u32) = (85, 88);
pub const POS_BTN_NEXT: (u32, u32) = (108, 88);
pub const POS_BTN_EJECT: (u32, u32) = (136, 89);

pub const REPEAT_OFF: Sprite = s(ShufRep, 0, 0, 28, 15);
pub const REPEAT_OFF_DOWN: Sprite = s(ShufRep, 0, 15, 28, 15);
pub const REPEAT_ON: Sprite = s(ShufRep, 0, 30, 28, 15);
pub const REPEAT_ON_DOWN: Sprite = s(ShufRep, 0, 45, 28, 15);
pub const SHUFFLE_OFF: Sprite = s(ShufRep, 28, 0, 47, 15);
pub const SHUFFLE_OFF_DOWN: Sprite = s(ShufRep, 28, 15, 47, 15);
pub const SHUFFLE_ON: Sprite = s(ShufRep, 28, 30, 47, 15);
pub const SHUFFLE_ON_DOWN: Sprite = s(ShufRep, 28, 45, 47, 15);
pub const EQ_OFF: Sprite = s(ShufRep, 0, 61, 23, 12);
pub const EQ_OFF_DOWN: Sprite = s(ShufRep, 46, 61, 23, 12);
pub const EQ_ON: Sprite = s(ShufRep, 0, 73, 23, 12);
pub const EQ_ON_DOWN: Sprite = s(ShufRep, 46, 73, 23, 12);
pub const PL_OFF: Sprite = s(ShufRep, 23, 61, 23, 12);
pub const PL_OFF_DOWN: Sprite = s(ShufRep, 69, 61, 23, 12);
pub const PL_ON: Sprite = s(ShufRep, 23, 73, 23, 12);
pub const PL_ON_DOWN: Sprite = s(ShufRep, 69, 73, 23, 12);
pub const POS_REPEAT: (u32, u32) = (210, 89);
pub const POS_SHUFFLE: (u32, u32) = (164, 89);
pub const POS_EQ_BTN: (u32, u32) = (219, 58);
pub const POS_PL_BTN: (u32, u32) = (242, 58);

pub const POSBAR_BG: Sprite = s(PosBar, 0, 0, 248, 10);
pub const POSBAR_THUMB: Sprite = s(PosBar, 248, 0, 29, 10);
pub const POSBAR_THUMB_DOWN: Sprite = s(PosBar, 278, 0, 29, 10);
pub const POS_POSBAR: (u32, u32) = (16, 72);
pub const POSBAR_TRAVEL: u32 = 219; // 248 - 29

pub const VOLUME_FRAMES: u32 = 28;
pub const fn volume_bg(frame: u32) -> Sprite {
    s(Volume, 0, frame * 15, 68, 13)
}
pub const VOLUME_THUMB: Sprite = s(Volume, 15, 422, 14, 11);
pub const VOLUME_THUMB_DOWN: Sprite = s(Volume, 0, 422, 14, 11);
pub const POS_VOLUME: (u32, u32) = (107, 57);
pub const VOLUME_TRAVEL: u32 = 51;

pub const fn balance_bg(frame: u32) -> Sprite {
    s(Balance, 9, frame * 15, 38, 13)
}
pub const BALANCE_THUMB: Sprite = s(Balance, 15, 422, 14, 11);
pub const BALANCE_THUMB_DOWN: Sprite = s(Balance, 0, 422, 14, 11);
pub const POS_BALANCE: (u32, u32) = (177, 57);
pub const BALANCE_TRAVEL: u32 = 24;

pub const STEREO_ON: Sprite = s(MonoSter, 0, 0, 29, 12);
pub const STEREO_OFF: Sprite = s(MonoSter, 0, 12, 29, 12);
pub const MONO_ON: Sprite = s(MonoSter, 29, 0, 27, 12);
pub const MONO_OFF: Sprite = s(MonoSter, 29, 12, 27, 12);
pub const POS_STEREO: (u32, u32) = (239, 41);
pub const POS_MONO: (u32, u32) = (212, 41);

pub const IND_PLAYING: Sprite = s(PlayPaus, 0, 0, 9, 9);
pub const IND_PAUSED: Sprite = s(PlayPaus, 9, 0, 9, 9);
pub const IND_STOPPED: Sprite = s(PlayPaus, 18, 0, 9, 9);
pub const IND_WORK_OFF: Sprite = s(PlayPaus, 36, 0, 3, 9);
pub const IND_WORK_ON: Sprite = s(PlayPaus, 39, 0, 3, 9);
pub const POS_INDICATOR: (u32, u32) = (26, 28);
pub const POS_WORK_INDICATOR: (u32, u32) = (24, 28);

/// Digit `d` (0..=9) from numbers.bmp / nums_ex.bmp.
pub const fn digit(img: SkinImage, d: u32) -> Sprite {
    s(img, d * 9, 0, 9, 13)
}
pub const DIGIT_BLANK_EX: Sprite = s(NumsEx, 90, 0, 9, 13);
pub const DIGIT_MINUS_EX: Sprite = s(NumsEx, 99, 0, 9, 13);
pub const DIGIT_BLANK: Sprite = s(Numbers, 90, 0, 9, 13);
/// X positions of the four time digits and y.
pub const POS_TIME_DIGITS: [(u32, u32); 4] = [(48, 26), (60, 26), (78, 26), (90, 26)];
pub const POS_TIME_MINUS: (u32, u32) = (36, 26);
pub const POS_TIME_MINUS_EX: (u32, u32) = (36, 26);

pub const POS_MARQUEE: (u32, u32) = (112, 27);
pub const MARQUEE_W: u32 = 154;
pub const POS_BITRATE: (u32, u32) = (111, 43);
pub const POS_SAMPLERATE: (u32, u32) = (156, 43);
pub const POS_VIS: (u32, u32) = (24, 43);
pub const VIS_W: u32 = 76;
pub const VIS_H: u32 = 16;

/// Bitmap font: 5x6 cells, 31 per row, 3 rows.
pub const FONT_CELL_W: u32 = 5;
pub const FONT_CELL_H: u32 = 6;
pub const fn font_cell(col: u32, row: u32) -> Sprite {
    s(Text, col * 5, row * 6, 5, 6)
}

/// Look up the (row, col) of a character in text.bmp.
pub fn font_lookup(c: char) -> Option<(u32, u32)> {
    let c = c.to_ascii_lowercase();
    let col = match c {
        'a'..='z' => return Some((0, c as u32 - 'a' as u32)),
        '"' => return Some((0, 26)),
        '@' => return Some((0, 27)),
        ' ' => return Some((0, 30)),
        '0'..='9' => c as u32 - '0' as u32,
        '…' => 10,
        '.' => 11,
        ':' => 12,
        '(' => 13,
        ')' => 14,
        '-' => 15,
        '\'' => 16,
        '!' => 17,
        '_' => 18,
        '+' => 19,
        '\\' => 20,
        '/' => 21,
        '[' => 22,
        ']' => 23,
        '^' => 24,
        '&' => 25,
        '%' => 26,
        ',' => 27,
        '=' => 28,
        '$' => 29,
        '#' => 30,
        'å' => return Some((2, 0)),
        'ö' => return Some((2, 1)),
        'ä' => return Some((2, 2)),
        '?' => return Some((2, 3)),
        '*' => return Some((2, 4)),
        _ => return None,
    };
    Some((1, col))
}

// --- equalizer ------------------------------------------------------------
pub const EQ_BG: Sprite = s(EqMain, 0, 0, 275, 116);
pub const EQ_TITLE_ACTIVE: Sprite = s(EqMain, 0, 134, 275, 14);
pub const EQ_TITLE_INACTIVE: Sprite = s(EqMain, 0, 149, 275, 14);
pub const EQ_ON_OFF: Sprite = s(EqMain, 10, 119, 26, 12);
pub const EQ_ON_OFF_DOWN: Sprite = s(EqMain, 128, 119, 26, 12);
pub const EQ_ON_ON: Sprite = s(EqMain, 69, 119, 26, 12);
pub const EQ_ON_ON_DOWN: Sprite = s(EqMain, 187, 119, 26, 12);
pub const EQ_AUTO_OFF: Sprite = s(EqMain, 35, 119, 32, 12);
pub const EQ_AUTO_OFF_DOWN: Sprite = s(EqMain, 153, 119, 32, 12);
pub const EQ_AUTO_ON: Sprite = s(EqMain, 94, 119, 32, 12);
pub const EQ_AUTO_ON_DOWN: Sprite = s(EqMain, 212, 119, 32, 12);
pub const EQ_PRESETS: Sprite = s(EqMain, 224, 164, 44, 12);
pub const EQ_PRESETS_DOWN: Sprite = s(EqMain, 224, 176, 44, 12);
pub const EQ_CLOSE: Sprite = s(EqMain, 0, 116, 9, 9);
pub const EQ_CLOSE_DOWN: Sprite = s(EqMain, 0, 125, 9, 9);
pub const POS_EQ_ON: (u32, u32) = (14, 18);
pub const POS_EQ_AUTO: (u32, u32) = (39, 18);
pub const POS_EQ_PRESETS: (u32, u32) = (217, 18);
pub const POS_EQ_CLOSE: (u32, u32) = (264, 3);
pub const POS_EQ_SHADE: (u32, u32) = (254, 3);
pub const fn eq_slider_bg(frame: u32) -> Sprite {
    s(EqMain, 13 + (frame % 14) * 15, 164 + (frame / 14) * 65, 14, 63)
}
pub const EQ_SLIDER_FRAMES: u32 = 28;
pub const EQ_THUMB: Sprite = s(EqMain, 0, 164, 11, 11);
pub const EQ_THUMB_DOWN: Sprite = s(EqMain, 0, 176, 11, 11);
pub const EQ_SLIDER_TRAVEL: u32 = 51;
pub const POS_EQ_PREAMP: (u32, u32) = (21, 38);
pub const fn pos_eq_band(i: u32) -> (u32, u32) {
    (78 + i * 18, 38)
}
pub const EQ_GRAPH_BG: Sprite = s(EqMain, 0, 294, 113, 19);
pub const EQ_GRAPH_COLORS: Sprite = s(EqMain, 115, 294, 1, 19);
pub const POS_EQ_GRAPH: (u32, u32) = (86, 17);

pub const EQ_SHADE_TITLE_ACTIVE: Sprite = s(EqEx, 0, 0, 275, 14);
pub const EQ_SHADE_TITLE_INACTIVE: Sprite = s(EqEx, 0, 15, 275, 14);
pub const EQ_SHADE_CLOSE: Sprite = s(EqEx, 11, 38, 9, 9);
pub const EQ_SHADE_CLOSE_DOWN: Sprite = s(EqEx, 11, 47, 9, 9);
pub const EQ_SHADE_UNSHADE: Sprite = s(EqEx, 1, 38, 9, 9);
pub const EQ_SHADE_UNSHADE_DOWN: Sprite = s(EqEx, 1, 47, 9, 9);

// --- playlist editor ------------------------------------------------------
pub const PL_TL: Sprite = s(PlEdit, 0, 0, 25, 20);
pub const PL_TITLE: Sprite = s(PlEdit, 26, 0, 100, 20);
pub const PL_TOP_TILE: Sprite = s(PlEdit, 127, 0, 25, 20);
pub const PL_TR: Sprite = s(PlEdit, 153, 0, 25, 20);
pub const PL_TL_INACTIVE: Sprite = s(PlEdit, 0, 21, 25, 20);
pub const PL_TITLE_INACTIVE: Sprite = s(PlEdit, 26, 21, 100, 20);
pub const PL_TOP_TILE_INACTIVE: Sprite = s(PlEdit, 127, 21, 25, 20);
pub const PL_TR_INACTIVE: Sprite = s(PlEdit, 153, 21, 25, 20);
pub const PL_LEFT_TILE: Sprite = s(PlEdit, 0, 42, 12, 29);
pub const PL_RIGHT_TILE: Sprite = s(PlEdit, 31, 42, 20, 29);
pub const PL_BOTTOM_TILE: Sprite = s(PlEdit, 179, 0, 25, 38);
pub const PL_BL: Sprite = s(PlEdit, 0, 72, 125, 38);
pub const PL_BR: Sprite = s(PlEdit, 126, 72, 150, 38);
pub const PL_SCROLL_THUMB: Sprite = s(PlEdit, 52, 53, 8, 18);
pub const PL_SCROLL_THUMB_DOWN: Sprite = s(PlEdit, 61, 53, 8, 18);
pub const PL_CLOSE_DOWN: Sprite = s(PlEdit, 52, 42, 9, 9);
pub const PL_SHADE_DOWN: Sprite = s(PlEdit, 62, 42, 9, 9);
pub const PL_SHADE_BG_LEFT: Sprite = s(PlEdit, 72, 42, 25, 14);
pub const PL_SHADE_BG: Sprite = s(PlEdit, 72, 57, 25, 14);
pub const PL_SHADE_BG_RIGHT_ACTIVE: Sprite = s(PlEdit, 99, 42, 50, 14);
pub const PL_SHADE_BG_RIGHT: Sprite = s(PlEdit, 99, 57, 50, 14);
pub const PL_SHADE_UNSHADE_DOWN: Sprite = s(PlEdit, 150, 42, 9, 9);
pub const PL_SHADE_CLOSE_DOWN: Sprite = s(PlEdit, 150, 42, 9, 9);
/// Bottom-bar button hit boxes (relative to the bottom-left corner origin).
pub const PL_BTN_ADD: (u32, u32, u32, u32) = (11, 20, 25, 18);
pub const PL_BTN_REM: (u32, u32, u32, u32) = (40, 20, 25, 18);
pub const PL_BTN_SEL: (u32, u32, u32, u32) = (70, 20, 25, 18);
pub const PL_BTN_MISC: (u32, u32, u32, u32) = (99, 20, 25, 18);
/// Relative to the right edge of the window.
pub const PL_BTN_LIST_FROM_RIGHT: (u32, u32, u32, u32) = (46, 20, 25, 18);
pub const PL_MINI_BUTTONS_FROM_RIGHT: u32 = 150; // x of PL_BR sprite from right edge
