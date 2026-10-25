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
use image::RgbaImage;

pub use sprites::Sprite;

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
    pub fn parse_hex(s: &str) -> Option<Rgb> {
        let s = s.trim().trim_start_matches('#');
        if s.len() < 6 {
            return None;
        }
        let r = u8::from_str_radix(&s[0..2], 16).ok()?;
        let g = u8::from_str_radix(&s[2..4], 16).ok()?;
        let b = u8::from_str_radix(&s[4..6], 16).ok()?;
        Some(Rgb(r, g, b))
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
    pub fn parse(text: &str) -> Self {
        let mut c = Self::default();
        for line in text.lines() {
            let line = line.trim();
            let Some((k, v)) = line.split_once('=') else { continue };
            let k = k.trim().to_ascii_lowercase();
            let v = v.trim();
            match k.as_str() {
                "normal" => c.normal = Rgb::parse_hex(v).unwrap_or(c.normal),
                "current" => c.current = Rgb::parse_hex(v).unwrap_or(c.current),
                "normalbg" => c.normal_bg = Rgb::parse_hex(v).unwrap_or(c.normal_bg),
                "selectedbg" => c.selected_bg = Rgb::parse_hex(v).unwrap_or(c.selected_bg),
                "mbfg" => c.mb_fg = Rgb::parse_hex(v).unwrap_or(c.mb_fg),
                "mbbg" => c.mb_bg = Rgb::parse_hex(v).unwrap_or(c.mb_bg),
                "font" => c.font = v.to_string(),
                _ => {}
            }
        }
        c
    }
}

/// The 24 colors of viscolor.txt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VisColors(pub [Rgb; 24]);

impl Default for VisColors {
    fn default() -> Self {
        // Approximation of Winamp's base skin viscolor.txt.
        let c = [
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
        VisColors(c.map(|(r, g, b)| Rgb(r, g, b)))
    }
}

impl VisColors {
    pub fn parse(text: &str) -> Self {
        let mut v = Self::default();
        let mut i = 0;
        for line in text.lines() {
            if i >= 24 {
                break;
            }
            let line = line.split("//").next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let nums: Vec<u8> = line
                .split(|c: char| c == ',' || c.is_whitespace())
                .filter(|s| !s.is_empty())
                .filter_map(|s| s.trim().parse::<i64>().ok())
                .map(|n| n.clamp(0, 255) as u8)
                .collect();
            if nums.len() >= 3 {
                v.0[i] = Rgb(nums[0], nums[1], nums[2]);
                i += 1;
            }
        }
        v
    }
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

impl Skin {
    /// The built-in default skin.
    pub fn builtin() -> Skin {
        default::generate()
    }

    pub fn image(&self, which: SkinImage) -> &RgbaImage {
        &self.images[&which]
    }

    pub fn has(&self, which: SkinImage) -> bool {
        self.provided.contains(&which)
    }

    /// Copy of a sprite as a standalone image.
    pub fn sprite_image(&self, sp: Sprite) -> RgbaImage {
        let src = self.image(sp.img);
        let mut out = RgbaImage::new(sp.w, sp.h);
        for y in 0..sp.h {
            for x in 0..sp.w {
                let (sx, sy) = (sp.x + x, sp.y + y);
                if sx < src.width() && sy < src.height() {
                    out.put_pixel(x, y, *src.get_pixel(sx, sy));
                }
            }
        }
        out
    }

    /// Color of a single pixel (used e.g. for the EQ graph palette).
    pub fn pixel(&self, img: SkinImage, x: u32, y: u32) -> Rgb {
        let im = self.image(img);
        if x < im.width() && y < im.height() {
            let p = im.get_pixel(x, y);
            Rgb(p[0], p[1], p[2])
        } else {
            Rgb(0, 0, 0)
        }
    }

    /// Load a skin from a `.wsz`/`.zip` archive or an unpacked directory.
    /// Missing bitmaps fall back to the built-in skin, as Winamp does.
    pub fn load(path: &Path) -> Result<Skin> {
        let files = if path.is_dir() { read_dir_files(path)? } else { read_zip_files(path)? };
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "skin".to_string());
        Self::from_files(name, files)
    }

    /// Build a skin from a map of lowercase file name (no directory) to bytes.
    pub fn from_files(name: String, files: HashMap<String, Vec<u8>>) -> Result<Skin> {
        let base = Skin::builtin();
        let mut images = base.images.clone();
        let mut provided = Vec::new();
        let mut any = false;
        for which in SkinImage::ALL {
            if let Some(img) = find_image(&files, which.file_stem()) {
                images.insert(which, img);
                provided.push(which);
                any = true;
            }
        }
        if !any {
            return Err(anyhow!("no skin bitmaps found in {name}"));
        }
        // Winamp substitutes volume.bmp for a missing balance.bmp.
        if !provided.contains(&SkinImage::Balance) && provided.contains(&SkinImage::Volume) {
            let vol = images[&SkinImage::Volume].clone();
            let mut bal = RgbaImage::new(47, vol.height().max(433));
            for y in 0..bal.height() {
                for x in 0..47 {
                    if x < vol.width() && y < vol.height() {
                        bal.put_pixel(x, y, *vol.get_pixel(x, y));
                    }
                }
            }
            images.insert(SkinImage::Balance, bal);
        }
        if !provided.contains(&SkinImage::NumsEx) && provided.contains(&SkinImage::Numbers) {
            images.insert(SkinImage::NumsEx, default::nums_ex_from_numbers(&default::BASE_THEME, &images[&SkinImage::Numbers]));
        }
        if !provided.contains(&SkinImage::EqEx) && provided.contains(&SkinImage::EqMain) {
            images.insert(SkinImage::EqEx, default::eq_ex_from_eqmain(&default::BASE_THEME, &images[&SkinImage::EqMain]));
        }
        let pledit = files
            .get("pledit.txt")
            .map(|b| PleditColors::parse(&String::from_utf8_lossy(b)))
            .unwrap_or_default();
        let viscolors = files
            .get("viscolor.txt")
            .map(|b| VisColors::parse(&String::from_utf8_lossy(b)))
            .unwrap_or_default();
        Ok(Skin { name, images, pledit, viscolors, provided })
    }
}

fn find_image(files: &HashMap<String, Vec<u8>>, stem: &str) -> Option<RgbaImage> {
    for ext in ["bmp", "png", "jpg", "gif"] {
        let key = format!("{stem}.{ext}");
        if let Some(bytes) = files.get(&key) {
            match decode_image(bytes) {
                Ok(img) => return Some(img),
                Err(e) => log::warn!("skin: failed to decode {key}: {e}"),
            }
        }
    }
    None
}

fn decode_image(bytes: &[u8]) -> Result<RgbaImage> {
    let img = image::load_from_memory(bytes)
        .or_else(|_| image::load_from_memory_with_format(bytes, image::ImageFormat::Bmp))?;
    Ok(img.to_rgba8())
}

fn read_dir_files(dir: &Path) -> Result<HashMap<String, Vec<u8>>> {
    let mut out = HashMap::new();
    for entry in std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let entry = entry?;
        let p = entry.path();
        if p.is_file() {
            let name = p.file_name().unwrap().to_string_lossy().to_ascii_lowercase();
            if let Ok(bytes) = std::fs::read(&p) {
                out.insert(name, bytes);
            }
        } else if p.is_dir() {
            // Some skins unpack into a nested folder.
            for (k, v) in read_dir_files(&p)? {
                out.entry(k).or_insert(v);
            }
        }
    }
    Ok(out)
}

fn read_zip_files(path: &Path) -> Result<HashMap<String, Vec<u8>>> {
    let file = std::fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut zip = zip::ZipArchive::new(file).with_context(|| "not a zip archive")?;
    let mut out = HashMap::new();
    for i in 0..zip.len() {
        let mut f = zip.by_index(i)?;
        if f.is_dir() {
            continue;
        }
        let name = f.name().rsplit(['/', '\\']).next().unwrap_or("").to_ascii_lowercase();
        if name.is_empty() {
            continue;
        }
        let mut bytes = Vec::with_capacity(f.size() as usize);
        f.read_to_end(&mut bytes)?;
        // Prefer top-level files over nested duplicates.
        let depth = f.name().matches(['/', '\\']).count();
        let existing_depth = out.get(&name).map(|(d, _)| *d).unwrap_or(usize::MAX);
        if depth < existing_depth {
            out.insert(name, (depth, bytes));
        }
    }
    Ok(out.into_iter().map(|(k, (_, v))| (k, v)).collect())
}

/// Export the built-in skin as a directory of BMP files (handy for skinners).
pub fn export_builtin(dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    let skin = Skin::builtin();
    for which in SkinImage::ALL {
        let p = dir.join(format!("{}.bmp", which.file_stem()));
        let img = skin.image(which);
        image::DynamicImage::ImageRgba8(img.clone()).to_rgb8().save(&p)?;
    }
    std::fs::write(dir.join("pledit.txt"), default::PLEDIT_TXT)?;
    std::fs::write(dir.join("viscolor.txt"), default::VISCOLOR_TXT)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_has_expected_sizes() {
        let s = Skin::builtin();
        assert_eq!(s.image(SkinImage::Main).dimensions(), (275, 116));
        assert_eq!(s.image(SkinImage::EqMain).dimensions(), (275, 315));
        assert_eq!(s.image(SkinImage::Text).dimensions(), (155, 18));
        assert_eq!(s.image(SkinImage::Volume).dimensions(), (68, 433));
        assert!(s.image(SkinImage::Titlebar).width() >= 344);
    }

    #[test]
    fn parse_pledit() {
        let c = PleditColors::parse("[Text]\nNormal=#00FF00\nCurrent=#FFFFFF\nNormalBG=#000000\nSelectedBG=#0000C6\nFont=Arial\n");
        assert_eq!(c.normal, Rgb(0, 255, 0));
        assert_eq!(c.selected_bg, Rgb(0, 0, 0xC6));
    }

    #[test]
    fn parse_viscolor() {
        let text = "0,0,0 // color 0\n24,33,41\n239,49,16\n";
        let v = VisColors::parse(text);
        assert_eq!(v.0[1], Rgb(24, 33, 41));
    }
}
