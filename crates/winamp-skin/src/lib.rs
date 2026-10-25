//! Winamp 2.x classic skin loading (.wsz / .zip / directory) plus a built-in
//! procedurally generated default skin.

pub mod default;
pub mod font;
pub mod sprites;
pub mod themes;

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use image::buffer::ConvertBuffer;
use image::{RgbImage, RgbaImage};

pub use sprites::Sprite;

/// Every bitmap a classic skin can provide.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SkinImage {
    Main,
    Titlebar,
    CButtons,
    ShufRep,
    PosBar,
    Volume,
    Balance,
    MonoSter,
    PlayPaus,
    Numbers,
    NumsEx,
    Text,
    EqMain,
    EqEx,
    PlEdit,
}

impl SkinImage {
    pub const ALL: [SkinImage; 15] = [
        SkinImage::Main,
        SkinImage::Titlebar,
        SkinImage::CButtons,
        SkinImage::ShufRep,
        SkinImage::PosBar,
        SkinImage::Volume,
        SkinImage::Balance,
        SkinImage::MonoSter,
        SkinImage::PlayPaus,
        SkinImage::Numbers,
        SkinImage::NumsEx,
        SkinImage::Text,
        SkinImage::EqMain,
        SkinImage::EqEx,
        SkinImage::PlEdit,
    ];

    /// The file name (without extension) this bitmap has inside a skin.
    pub fn file_stem(self) -> &'static str {
        match self {
            SkinImage::Main => "main",
            SkinImage::Titlebar => "titlebar",
            SkinImage::CButtons => "cbuttons",
            SkinImage::ShufRep => "shufrep",
            SkinImage::PosBar => "posbar",
            SkinImage::Volume => "volume",
            SkinImage::Balance => "balance",
            SkinImage::MonoSter => "monoster",
            SkinImage::PlayPaus => "playpaus",
            SkinImage::Numbers => "numbers",
            SkinImage::NumsEx => "nums_ex",
            SkinImage::Text => "text",
            SkinImage::EqMain => "eqmain",
            SkinImage::EqEx => "eq_ex",
            SkinImage::PlEdit => "pledit",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    /// Parse `#RRGGBB` (the `#` is optional; trailing text is ignored).
    pub fn parse_hex(text: &str) -> Option<Rgb> {
        let hex = text.trim().trim_start_matches('#');
        let channel = |range: std::ops::Range<usize>| {
            hex.get(range).and_then(|s| u8::from_str_radix(s, 16).ok())
        };
        Some(Rgb(channel(0..2)?, channel(2..4)?, channel(4..6)?))
    }
}

/// Colors from pledit.txt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PleditColors {
    pub normal: Rgb,
    pub current: Rgb,
    pub normal_bg: Rgb,
    pub selected_bg: Rgb,
    pub mb_fg: Rgb,
    pub mb_bg: Rgb,
    pub font: String,
}

impl Default for PleditColors {
    fn default() -> Self {
        Self {
            normal: Rgb(0x00, 0xFF, 0x00),
            current: Rgb(0xFF, 0xFF, 0xFF),
            normal_bg: Rgb(0x00, 0x00, 0x00),
            selected_bg: Rgb(0x00, 0x00, 0xC6),
            mb_fg: Rgb(0x00, 0xFF, 0x00),
            mb_bg: Rgb(0x00, 0x00, 0x00),
            font: "Arial".to_string(),
        }
    }
}

impl PleditColors {
    /// Parse the `Key=Value` lines of pledit.txt; unknown or malformed
    /// entries keep their defaults.
    pub fn parse(text: &str) -> Self {
        let mut colors = Self::default();
        for line in text.lines() {
            let Some((key, value)) = line.trim().split_once('=') else {
                continue;
            };
            let value = value.trim();
            let slot = match key.trim().to_ascii_lowercase().as_str() {
                "normal" => &mut colors.normal,
                "current" => &mut colors.current,
                "normalbg" => &mut colors.normal_bg,
                "selectedbg" => &mut colors.selected_bg,
                "mbfg" => &mut colors.mb_fg,
                "mbbg" => &mut colors.mb_bg,
                "font" => {
                    colors.font = value.to_string();
                    continue;
                }
                _ => continue,
            };
            if let Some(color) = Rgb::parse_hex(value) {
                *slot = color;
            }
        }
        colors
    }
}

/// The 24 colors of viscolor.txt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VisColors(pub [Rgb; 24]);

impl Default for VisColors {
    fn default() -> Self {
        // Approximation of Winamp's base skin viscolor.txt.
        let colors = [
            (0, 0, 0),
            (24, 33, 41),
            (239, 49, 16),
            (206, 41, 16),
            (214, 90, 0),
            (214, 102, 0),
            (214, 115, 0),
            (198, 123, 8),
            (222, 165, 24),
            (214, 181, 33),
            (189, 222, 41),
            (148, 222, 33),
            (41, 206, 16),
            (50, 190, 16),
            (57, 181, 16),
            (49, 156, 8),
            (41, 148, 0),
            (24, 132, 8),
            (255, 255, 255),
            (214, 214, 222),
            (181, 189, 189),
            (160, 170, 175),
            (148, 156, 165),
            (150, 150, 150),
        ];
        VisColors(colors.map(|(r, g, b)| Rgb(r, g, b)))
    }
}

impl VisColors {
    /// Parse `r,g,b` lines (comments after `//` are ignored). Missing lines
    /// keep the default colors.
    pub fn parse(text: &str) -> Self {
        let mut colors = Self::default();
        let parsed = text
            .lines()
            .map(|line| line.split("//").next().unwrap_or("").trim())
            .filter(|line| !line.is_empty())
            .filter_map(parse_rgb_line);
        for (slot, color) in colors.0.iter_mut().zip(parsed) {
            *slot = color;
        }
        colors
    }
}

/// Parse "r,g,b" (or whitespace separated), clamping each channel to 0..=255.
fn parse_rgb_line(line: &str) -> Option<Rgb> {
    let mut channels = line
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<i64>().ok())
        .map(|n| n.clamp(0, 255) as u8);
    Some(Rgb(channels.next()?, channels.next()?, channels.next()?))
}

#[derive(Clone, Debug)]
pub struct Skin {
    pub name: String,
    images: HashMap<SkinImage, RgbaImage>,
    pub pledit: PleditColors,
    pub viscolors: VisColors,
    /// Which images came from the loaded skin rather than the fallback.
    pub provided: Vec<SkinImage>,
}

/// Winamp's `balance.bmp` is 47 pixels wide; a missing one is cut from `volume.bmp`.
const BALANCE_WIDTH: u32 = 47;
const SLIDER_BITMAP_HEIGHT: u32 = 433;

impl Skin {
    /// The built-in default skin.
    pub fn builtin() -> Skin {
        default::generate()
    }

    pub fn image(&self, which: SkinImage) -> &RgbaImage {
        &self.images[&which]
    }

    /// True when the loaded skin supplied this bitmap itself.
    pub fn has(&self, which: SkinImage) -> bool {
        self.provided.contains(&which)
    }

    /// Copy of a sprite as a standalone image.
    pub fn sprite_image(&self, sprite: Sprite) -> RgbaImage {
        crop(
            self.image(sprite.img),
            sprite.x,
            sprite.y,
            sprite.w,
            sprite.h,
        )
    }

    /// Color of a single pixel (used e.g. for the EQ graph palette).
    pub fn pixel(&self, which: SkinImage, x: u32, y: u32) -> Rgb {
        let image = self.image(which);
        if x < image.width() && y < image.height() {
            let p = image.get_pixel(x, y);
            Rgb(p[0], p[1], p[2])
        } else {
            Rgb(0, 0, 0)
        }
    }

    /// Load a skin from a `.wsz`/`.zip` archive or an unpacked directory.
    /// Missing bitmaps fall back to the built-in skin, as Winamp does.
    pub fn load(path: impl AsRef<Path>) -> Result<Skin> {
        let path = path.as_ref();
        let files = if path.is_dir() {
            read_dir_files(path)?
        } else {
            read_zip_files(path)?
        };
        let name = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_else(|| "skin".to_string());
        Self::from_files(name, files)
    }

    /// Build a skin from a map of lowercase file name (no directory) to bytes.
    pub fn from_files(name: String, files: HashMap<String, Vec<u8>>) -> Result<Skin> {
        let mut images = Skin::builtin().images;
        let mut provided = Vec::new();
        for which in SkinImage::ALL {
            if let Some(image) = find_image(&files, which.file_stem()) {
                images.insert(which, image);
                provided.push(which);
            }
        }
        if provided.is_empty() {
            return Err(anyhow!("no skin bitmaps found in {name}"));
        }
        let has = |which: SkinImage| provided.contains(&which);
        // Winamp substitutes volume.bmp for a missing balance.bmp.
        if !has(SkinImage::Balance) && has(SkinImage::Volume) {
            let volume = &images[&SkinImage::Volume];
            let height = volume.height().max(SLIDER_BITMAP_HEIGHT);
            let balance = crop(volume, 0, 0, BALANCE_WIDTH, height);
            images.insert(SkinImage::Balance, balance);
        }
        if !has(SkinImage::NumsEx) && has(SkinImage::Numbers) {
            let nums_ex =
                default::nums_ex_from_numbers(&default::BASE_THEME, &images[&SkinImage::Numbers]);
            images.insert(SkinImage::NumsEx, nums_ex);
        }
        if !has(SkinImage::EqEx) && has(SkinImage::EqMain) {
            let eq_ex =
                default::eq_ex_from_eqmain(&default::BASE_THEME, &images[&SkinImage::EqMain]);
            images.insert(SkinImage::EqEx, eq_ex);
        }
        let text_file = |name: &str| files.get(name).map(|bytes| String::from_utf8_lossy(bytes));
        let pledit = text_file("pledit.txt")
            .map(|text| PleditColors::parse(&text))
            .unwrap_or_default();
        let viscolors = text_file("viscolor.txt")
            .map(|text| VisColors::parse(&text))
            .unwrap_or_default();
        Ok(Skin {
            name,
            images,
            pledit,
            viscolors,
            provided,
        })
    }
}

/// Copy a `w` x `h` region starting at (`x`, `y`); pixels outside `source`
/// come out transparent black.
fn crop(source: &RgbaImage, x: u32, y: u32, w: u32, h: u32) -> RgbaImage {
    let mut out = RgbaImage::new(w, h);
    for (dx, dy, pixel) in out.enumerate_pixels_mut() {
        let (sx, sy) = (x + dx, y + dy);
        if sx < source.width() && sy < source.height() {
            *pixel = *source.get_pixel(sx, sy);
        }
    }
    out
}

fn find_image(files: &HashMap<String, Vec<u8>>, stem: &str) -> Option<RgbaImage> {
    ["bmp", "png", "jpg", "gif"].iter().find_map(|ext| {
        let key = format!("{stem}.{ext}");
        let bytes = files.get(&key)?;
        match decode_image(bytes) {
            Ok(image) => Some(image),
            Err(e) => {
                log::warn!("skin: failed to decode {key}: {e}");
                None
            }
        }
    })
}

fn decode_image(bytes: &[u8]) -> Result<RgbaImage> {
    let image = image::load_from_memory(bytes)
        .or_else(|_| image::load_from_memory_with_format(bytes, image::ImageFormat::Bmp))?;
    Ok(image.to_rgba8())
}

/// Read every file in `dir` (recursing into nested folders, where top-level
/// files win) keyed by lowercase file name.
fn read_dir_files(dir: &Path) -> Result<HashMap<String, Vec<u8>>> {
    let mut out = HashMap::new();
    for entry in std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            match std::fs::read(&path) {
                Ok(bytes) => {
                    out.insert(name, bytes);
                }
                Err(e) => log::warn!("skin: skipping {}: {e}", path.display()),
            }
        } else if path.is_dir() {
            // Some skins unpack into a nested folder.
            for (name, bytes) in read_dir_files(&path)? {
                out.entry(name).or_insert(bytes);
            }
        }
    }
    Ok(out)
}

/// Read every file in a zip archive keyed by lowercase file name. When the
/// same name appears at several depths the shallowest copy wins.
fn read_zip_files(path: &Path) -> Result<HashMap<String, Vec<u8>>> {
    let file = std::fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut archive = zip::ZipArchive::new(file)
        .with_context(|| format!("{} is not a zip archive", path.display()))?;
    let mut shallowest: HashMap<String, (usize, Vec<u8>)> = HashMap::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        if entry.is_dir() {
            continue;
        }
        let name = entry
            .name()
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        if name.is_empty() {
            continue;
        }
        let depth = entry.name().matches(['/', '\\']).count();
        let existing_depth = shallowest
            .get(&name)
            .map_or(usize::MAX, |(depth, _)| *depth);
        if depth < existing_depth {
            let mut bytes = Vec::with_capacity(entry.size() as usize);
            entry.read_to_end(&mut bytes)?;
            shallowest.insert(name, (depth, bytes));
        }
    }
    Ok(shallowest
        .into_iter()
        .map(|(name, (_, bytes))| (name, bytes))
        .collect())
}

/// Export the built-in skin as a directory of BMP files (handy for skinners).
pub fn export_builtin(dir: impl AsRef<Path>) -> Result<()> {
    let dir = dir.as_ref();
    std::fs::create_dir_all(dir)?;
    let skin = Skin::builtin();
    for which in SkinImage::ALL {
        let path = dir.join(format!("{}.bmp", which.file_stem()));
        let rgb: RgbImage = skin.image(which).convert();
        rgb.save(&path)
            .with_context(|| format!("writing {}", path.display()))?;
    }
    std::fs::write(dir.join("pledit.txt"), default::PLEDIT_TXT)?;
    std::fs::write(dir.join("viscolor.txt"), default::VISCOLOR_TXT)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bmp_bytes(image: &RgbaImage) -> Vec<u8> {
        let mut bytes = std::io::Cursor::new(Vec::new());
        let rgb: RgbImage = image.convert();
        rgb.write_to(&mut bytes, image::ImageFormat::Bmp).unwrap();
        bytes.into_inner()
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("winamp-skin-tests-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn builtin_has_expected_sizes() {
        let skin = Skin::builtin();
        assert_eq!(skin.image(SkinImage::Main).dimensions(), (275, 116));
        assert_eq!(skin.image(SkinImage::EqMain).dimensions(), (275, 315));
        assert_eq!(skin.image(SkinImage::Text).dimensions(), (155, 18));
        assert_eq!(skin.image(SkinImage::Volume).dimensions(), (68, 433));
        assert_eq!(skin.image(SkinImage::Balance).dimensions(), (47, 433));
        assert!(skin.image(SkinImage::Titlebar).width() >= 344);
        assert!(SkinImage::ALL.iter().all(|&which| skin.has(which)));
    }

    #[test]
    fn parses_hex_colors() {
        assert_eq!(Rgb::parse_hex("#00FF00"), Some(Rgb(0, 255, 0)));
        assert_eq!(Rgb::parse_hex("  0000c6 "), Some(Rgb(0, 0, 0xC6)));
        assert_eq!(
            Rgb::parse_hex("#123456 trailing"),
            Some(Rgb(0x12, 0x34, 0x56))
        );
        assert_eq!(Rgb::parse_hex("#12345"), None);
        assert_eq!(Rgb::parse_hex("#GGGGGG"), None);
        assert_eq!(Rgb::parse_hex(""), None);
    }

    #[test]
    fn parse_pledit() {
        let colors = PleditColors::parse("[Text]\nNormal=#00FF00\nCurrent=#FFFFFF\nNormalBG=#000000\nSelectedBG=#0000C6\nFont=Arial\n");
        assert_eq!(colors.normal, Rgb(0, 255, 0));
        assert_eq!(colors.selected_bg, Rgb(0, 0, 0xC6));
    }

    #[test]
    fn pledit_is_case_insensitive_and_keeps_defaults_for_bad_values() {
        let colors = PleditColors::parse("NORMAL=#FF0000\ncurrent=garbage\nmbFG=#010203\nfont = Tahoma\nUnknown=1\nno equals sign");
        assert_eq!(colors.normal, Rgb(255, 0, 0));
        assert_eq!(colors.current, PleditColors::default().current);
        assert_eq!(colors.mb_fg, Rgb(1, 2, 3));
        assert_eq!(colors.font, "Tahoma");
    }

    #[test]
    fn parse_viscolor() {
        let text = "0,0,0 // color 0\n24,33,41\n239,49,16\n";
        let colors = VisColors::parse(text);
        assert_eq!(colors.0[1], Rgb(24, 33, 41));
        assert_eq!(
            colors.0[3],
            VisColors::default().0[3],
            "missing lines keep defaults"
        );
    }

    #[test]
    fn viscolor_accepts_spaces_and_clamps_out_of_range() {
        let colors = VisColors::parse("300, -5 ,  7\n\n// comment only\n1 2 3\nnot a color\n");
        assert_eq!(colors.0[0], Rgb(255, 0, 7));
        assert_eq!(colors.0[1], Rgb(1, 2, 3));
        let many = (0..30)
            .map(|i| format!("{i},{i},{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(
            VisColors::parse(&many).0[23],
            Rgb(23, 23, 23),
            "extra lines are ignored"
        );
    }

    #[test]
    fn from_files_requires_at_least_one_bitmap() {
        let err = Skin::from_files("empty".into(), HashMap::new())
            .err()
            .map(|e| e.to_string());
        assert_eq!(err.as_deref(), Some("no skin bitmaps found in empty"));
    }

    #[test]
    fn from_files_falls_back_and_synthesises_missing_bitmaps() {
        let builtin = Skin::builtin();
        let mut volume = builtin.image(SkinImage::Volume).clone();
        volume.put_pixel(3, 3, image::Rgba([1, 2, 3, 255]));
        let files = HashMap::from([
            ("volume.bmp".to_string(), bmp_bytes(&volume)),
            (
                "numbers.bmp".to_string(),
                bmp_bytes(builtin.image(SkinImage::Numbers)),
            ),
            ("pledit.txt".to_string(), b"Normal=#010203".to_vec()),
        ]);
        let skin = Skin::from_files("partial".into(), files).unwrap();
        assert_eq!(skin.provided, vec![SkinImage::Volume, SkinImage::Numbers]);
        assert_eq!(
            skin.pixel(SkinImage::Balance, 3, 3),
            Rgb(1, 2, 3),
            "balance is cut from volume"
        );
        assert_eq!(skin.image(SkinImage::Balance).dimensions(), (47, 433));
        assert_eq!(
            skin.image(SkinImage::NumsEx).dimensions(),
            (108, 13),
            "nums_ex is built from numbers"
        );
        assert_eq!(
            skin.image(SkinImage::Main).dimensions(),
            (275, 116),
            "main falls back to the built-in"
        );
        assert!(!skin.has(SkinImage::Main));
        assert_eq!(skin.pledit.normal, Rgb(1, 2, 3));
        assert_eq!(skin.viscolors, VisColors::default());
    }

    #[test]
    fn pixel_and_sprite_image_are_bounds_safe() {
        let skin = Skin::builtin();
        assert_eq!(skin.pixel(SkinImage::Main, 10_000, 10_000), Rgb(0, 0, 0));
        let sprite = Sprite {
            img: SkinImage::Numbers,
            x: 95,
            y: 10,
            w: 9,
            h: 13,
        };
        let image = skin.sprite_image(sprite);
        assert_eq!(image.dimensions(), (9, 13));
        assert_eq!(
            image.get_pixel(8, 12).0[3],
            0,
            "outside the bitmap is transparent"
        );
    }

    #[test]
    fn loads_a_skin_from_a_directory_including_nested_folders() {
        let dir = temp_dir("dir-skin");
        export_builtin(&dir).unwrap();
        std::fs::remove_file(dir.join("eq_ex.bmp")).unwrap();
        std::fs::create_dir_all(dir.join("nested")).unwrap();
        std::fs::rename(dir.join("text.bmp"), dir.join("nested/TEXT.BMP")).unwrap();
        let skin = Skin::load(&dir).unwrap();
        assert!(
            skin.name.ends_with("dir-skin"),
            "name is the folder stem: {}",
            skin.name
        );
        assert!(
            skin.has(SkinImage::Text),
            "nested, differently cased files are found"
        );
        assert!(!skin.has(SkinImage::EqEx));
        assert_eq!(
            skin.image(SkinImage::EqEx).dimensions(),
            (275, 57),
            "eq_ex is synthesised from eqmain"
        );
        assert!(Skin::load(dir.join("does-not-exist")).is_err());
    }

    #[test]
    fn zip_prefers_top_level_files_over_nested_duplicates() {
        use std::io::Write;
        let dir = temp_dir("zip-skin");
        let path = dir.join("test.wsz");
        let builtin = Skin::builtin();
        let mut nested_main = builtin.image(SkinImage::Main).clone();
        nested_main.put_pixel(0, 0, image::Rgba([9, 9, 9, 255]));
        {
            let file = std::fs::File::create(&path).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            let opts = zip::write::SimpleFileOptions::default();
            zip.start_file("Deep/Folder/main.bmp", opts).unwrap();
            zip.write_all(&bmp_bytes(&nested_main)).unwrap();
            zip.start_file("MAIN.BMP", opts).unwrap();
            zip.write_all(&bmp_bytes(builtin.image(SkinImage::Main)))
                .unwrap();
            zip.add_directory("empty/", opts).unwrap();
            zip.finish().unwrap();
        }
        let skin = Skin::load(&path).unwrap();
        assert_eq!(skin.name, "test");
        assert_eq!(skin.provided, vec![SkinImage::Main]);
        assert_ne!(
            skin.pixel(SkinImage::Main, 0, 0),
            Rgb(9, 9, 9),
            "top-level main.bmp wins"
        );
        std::fs::write(dir.join("garbage.wsz"), b"not a zip").unwrap();
        assert!(Skin::load(dir.join("garbage.wsz")).is_err());
    }
}
