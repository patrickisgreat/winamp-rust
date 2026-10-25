//! Playlist model plus M3U / M3U8 / PLS reading and writing.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use rand::seq::SliceRandom;
use winamp_audio::{read_meta, Source, TrackMeta};

#[derive(Clone, Debug)]
pub struct Entry {
    pub source: Source,
    /// Display title ("Artist - Title" or file stem / stream name).
    pub title: String,
    pub duration: Option<Duration>,
    pub meta: Option<TrackMeta>,
    pub selected: bool,
}

impl Entry {
    pub fn from_source(source: Source) -> Self {
        let title = match &source {
            Source::File(p) => p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(),
            Source::Url(u) => u.clone(),
        };
        Self { source, title, duration: None, meta: None, selected: false }
    }

    /// Read tags for local files (cheap enough to do synchronously for a few
    /// hundred files; the app also defers this to a worker for big loads).
    pub fn read_tags(&mut self) {
        if let Source::File(p) = &self.source {
            let meta = read_meta(p);
            self.title = meta.display_title(p);
            self.duration = meta.duration;
            self.meta = Some(meta);
        }
    }

    pub fn display_with_number(&self, n: usize) -> String {
        format!("{n}. {}", self.title)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortKey {
    Title,
    Filename,
    Path,
}

#[derive(Default, Clone, Debug)]
pub struct Playlist {
    pub entries: Vec<Entry>,
    /// Index of the track currently playing / loaded.
    pub current: Option<usize>,
    /// Shuffle order: a permutation of indices, consumed from `shuffle_pos`.
    shuffle_order: Vec<usize>,
    shuffle_pos: usize,
    /// Where this playlist was last saved/loaded from.
    pub path: Option<PathBuf>,
}

impl Playlist {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.current = None;
        self.shuffle_order.clear();
        self.shuffle_pos = 0;
    }

    pub fn push(&mut self, entry: Entry) -> usize {
        self.entries.push(entry);
        self.invalidate_shuffle();
        self.entries.len() - 1
    }

    pub fn insert(&mut self, idx: usize, entry: Entry) {
        let idx = idx.min(self.entries.len());
        self.entries.insert(idx, entry);
        if let Some(c) = self.current {
            if idx <= c {
                self.current = Some(c + 1);
            }
        }
        self.invalidate_shuffle();
    }

    /// Add a file, URL, directory (recursively) or playlist file.
    pub fn add_path(&mut self, path: &Path) -> usize {
        let mut added = 0;
        if path.is_dir() {
            let mut files: Vec<PathBuf> = walkdir::WalkDir::new(path)
                .follow_links(true)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .map(|e| e.into_path())
                .filter(|p| winamp_audio::is_supported_path(p))
                .collect();
            files.sort();
            for f in files {
                self.push(Entry::from_source(Source::File(f)));
                added += 1;
            }
        } else if is_playlist_path(path) {
            match load_playlist_file(path) {
                Ok(entries) => {
                    for e in entries {
                        self.push(e);
                        added += 1;
                    }
                }
                Err(e) => log::warn!("loading playlist {}: {e}", path.display()),
            }
        } else {
            self.push(Entry::from_source(Source::File(path.to_path_buf())));
            added += 1;
        }
        added
    }

    pub fn add_url(&mut self, url: &str) {
        self.push(Entry::from_source(Source::Url(url.to_string())));
    }

    pub fn remove_indices(&mut self, mut indices: Vec<usize>) {
        indices.sort_unstable();
        indices.dedup();
        for &i in indices.iter().rev() {
            if i < self.entries.len() {
                self.entries.remove(i);
                if let Some(c) = self.current {
                    if i < c {
                        self.current = Some(c - 1);
                    } else if i == c {
                        self.current = None;
                    }
                }
            }
        }
        self.invalidate_shuffle();
    }

    pub fn remove_selected(&mut self) {
        let sel: Vec<usize> = self.selected_indices();
        self.remove_indices(sel);
    }

    /// Keep only the selected entries ("Crop").
    pub fn crop(&mut self) {
        let unsel: Vec<usize> = (0..self.entries.len()).filter(|&i| !self.entries[i].selected).collect();
        self.remove_indices(unsel);
    }

    pub fn remove_missing_files(&mut self) {
        let missing: Vec<usize> = (0..self.entries.len())
            .filter(|&i| matches!(&self.entries[i].source, Source::File(p) if !p.exists()))
            .collect();
        self.remove_indices(missing);
    }

    pub fn remove_duplicates(&mut self) {
        let mut seen = std::collections::HashSet::new();
        let dupes: Vec<usize> = (0..self.entries.len())
            .filter(|&i| !seen.insert(self.entries[i].source.clone()))
            .collect();
        self.remove_indices(dupes);
    }

    pub fn selected_indices(&self) -> Vec<usize> {
        (0..self.entries.len()).filter(|&i| self.entries[i].selected).collect()
    }

    pub fn select_all(&mut self) {
        self.entries.iter_mut().for_each(|e| e.selected = true);
    }
    pub fn select_none(&mut self) {
        self.entries.iter_mut().for_each(|e| e.selected = false);
    }
    pub fn invert_selection(&mut self) {
        self.entries.iter_mut().for_each(|e| e.selected = !e.selected);
    }

    pub fn sort(&mut self, key: SortKey, reverse: bool) {
        let cur = self.current.map(|c| self.entries[c].source.clone());
        self.entries.sort_by_cached_key(|e| match key {
            SortKey::Title => e.title.to_lowercase(),
            SortKey::Filename => match &e.source {
                Source::File(p) => p.file_name().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default(),
                Source::Url(u) => u.to_lowercase(),
            },
            SortKey::Path => e.source.as_display().to_lowercase(),
        });
        if reverse {
            self.entries.reverse();
        }
        self.restore_current(cur);
        self.invalidate_shuffle();
    }

    pub fn reverse(&mut self) {
        let cur = self.current.map(|c| self.entries[c].source.clone());
        self.entries.reverse();
        self.restore_current(cur);
        self.invalidate_shuffle();
    }

    pub fn randomize(&mut self) {
        let cur = self.current.map(|c| self.entries[c].source.clone());
        self.entries.shuffle(&mut rand::thread_rng());
        self.restore_current(cur);
        self.invalidate_shuffle();
    }

    fn restore_current(&mut self, cur: Option<Source>) {
        self.current = cur.and_then(|c| self.entries.iter().position(|e| e.source == c));
    }

    /// Move the selected entries up or down by one slot.
    pub fn move_selected(&mut self, delta: i32) {
        let sel = self.selected_indices();
        if sel.is_empty() {
            return;
        }
        let cur = self.current.map(|c| self.entries[c].source.clone());
        if delta < 0 {
            if sel[0] == 0 {
                return;
            }
            for &i in &sel {
                self.entries.swap(i, i - 1);
            }
        } else {
            if *sel.last().unwrap() + 1 >= self.entries.len() {
                return;
            }
            for &i in sel.iter().rev() {
                self.entries.swap(i, i + 1);
            }
        }
        self.restore_current(cur);
        self.invalidate_shuffle();
    }

    /// Move a block of entries to `target` (drag and drop reorder).
    pub fn move_entries(&mut self, indices: &[usize], target: usize) {
        if indices.is_empty() {
            return;
        }
        let cur = self.current.map(|c| self.entries[c].source.clone());
        let mut moving = Vec::new();
        let mut rest = Vec::new();
        let mut before_target = 0usize;
        for (i, e) in self.entries.drain(..).enumerate() {
            if indices.contains(&i) {
                if i < target {
                    before_target += 1;
                }
                moving.push(e);
            } else {
                rest.push(e);
            }
        }
        let at = target.saturating_sub(before_target).min(rest.len());
        let tail = rest.split_off(at);
        rest.extend(moving);
        rest.extend(tail);
        self.entries = rest;
        self.restore_current(cur);
        self.invalidate_shuffle();
    }

    pub fn total_duration(&self) -> (Duration, bool) {
        let mut total = Duration::ZERO;
        let mut all_known = true;
        for e in &self.entries {
            match e.duration {
                Some(d) => total += d,
                None => all_known = false,
            }
        }
        (total, all_known)
    }

    pub fn selected_duration(&self) -> (Duration, bool) {
        let mut total = Duration::ZERO;
        let mut all_known = true;
        for e in self.entries.iter().filter(|e| e.selected) {
            match e.duration {
                Some(d) => total += d,
                None => all_known = false,
            }
        }
        (total, all_known)
    }

    // --- navigation -------------------------------------------------------

    fn invalidate_shuffle(&mut self) {
        self.shuffle_order.clear();
        self.shuffle_pos = 0;
    }

    fn ensure_shuffle(&mut self) {
        if self.shuffle_order.len() != self.entries.len() {
            self.shuffle_order = (0..self.entries.len()).collect();
            self.shuffle_order.shuffle(&mut rand::thread_rng());
            // Start from the current track so "next" doesn't replay it immediately.
            if let Some(c) = self.current {
                if let Some(p) = self.shuffle_order.iter().position(|&i| i == c) {
                    self.shuffle_order.swap(0, p);
                }
            }
            self.shuffle_pos = 0;
        }
    }

    /// Index of the track to play after `current`. Returns None at the end
    /// of the list when repeat is off.
    pub fn next_index(&mut self, shuffle: bool, repeat: bool) -> Option<usize> {
        if self.entries.is_empty() {
            return None;
        }
        if shuffle {
            self.ensure_shuffle();
            if self.shuffle_pos + 1 < self.shuffle_order.len() {
                self.shuffle_pos += 1;
                return Some(self.shuffle_order[self.shuffle_pos]);
            }
            if repeat {
                self.invalidate_shuffle();
                self.ensure_shuffle();
                self.shuffle_pos = if self.shuffle_order.len() > 1 { 1 } else { 0 };
                return Some(self.shuffle_order[self.shuffle_pos]);
            }
            return None;
        }
        match self.current {
            None => Some(0),
            Some(c) if c + 1 < self.entries.len() => Some(c + 1),
            Some(_) if repeat => Some(0),
            Some(_) => None,
        }
    }

    pub fn prev_index(&mut self, shuffle: bool, repeat: bool) -> Option<usize> {
        if self.entries.is_empty() {
            return None;
        }
        if shuffle {
            self.ensure_shuffle();
            if self.shuffle_pos > 0 {
                self.shuffle_pos -= 1;
                return Some(self.shuffle_order[self.shuffle_pos]);
            }
            if repeat {
                self.shuffle_pos = self.shuffle_order.len() - 1;
                return Some(self.shuffle_order[self.shuffle_pos]);
            }
            return None;
        }
        match self.current {
            None => Some(0),
            Some(0) if repeat => Some(self.entries.len() - 1),
            Some(0) => None,
            Some(c) => Some(c - 1),
        }
    }

    /// Record that `idx` became the current track (keeps shuffle history in sync).
    pub fn set_current(&mut self, idx: usize) {
        if idx >= self.entries.len() {
            return;
        }
        self.current = Some(idx);
        if self.shuffle_order.len() == self.entries.len() {
            if let Some(p) = self.shuffle_order.iter().position(|&i| i == idx) {
                // Move the chosen track to the current shuffle slot so history stays linear.
                let cur = self.shuffle_pos.min(self.shuffle_order.len() - 1);
                self.shuffle_order.swap(cur, p);
            }
        }
    }

    // --- persistence --------------------------------------------------------

    pub fn load_file(&mut self, path: &Path) -> Result<()> {
        let entries = load_playlist_file(path)?;
        self.clear();
        for e in entries {
            self.push(e);
        }
        self.path = Some(path.to_path_buf());
        Ok(())
    }

    pub fn save_file(&mut self, path: &Path) -> Result<()> {
        let text = match path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).as_deref() {
            Some("pls") => to_pls(&self.entries),
            _ => to_m3u(&self.entries, path.parent()),
        };
        std::fs::write(path, text).with_context(|| format!("writing {}", path.display()))?;
        self.path = Some(path.to_path_buf());
        Ok(())
    }
}

pub fn is_playlist_path(path: &Path) -> bool {
    matches!(
        path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).as_deref(),
        Some("m3u") | Some("m3u8") | Some("pls")
    )
}

pub fn load_playlist_file(path: &Path) -> Result<Vec<Entry>> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let text = decode_text(&bytes);
    let base = path.parent();
    let ext = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase());
    Ok(match ext.as_deref() {
        Some("pls") => parse_pls(&text, base),
        _ => parse_m3u(&text, base),
    })
}

/// Parse playlist text fetched from a URL (remote .pls / .m3u).
pub fn parse_remote_playlist(text: &str, url: &str) -> Vec<Entry> {
    let lower = url.to_ascii_lowercase();
    let entries = if lower.contains(".pls") || text.trim_start().to_ascii_lowercase().starts_with("[playlist]") {
        parse_pls(text, None)
    } else {
        parse_m3u(text, None)
    };
    entries
}

fn decode_text(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        // Latin-1 fallback for legacy .m3u files.
        Err(_) => bytes.iter().map(|&b| b as char).collect(),
    }
}

fn resolve(item: &str, base: Option<&Path>) -> Source {
    let item = item.trim().trim_matches('"');
    let lower = item.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return Source::Url(item.to_string());
    }
    if let Some(rest) = lower.strip_prefix("file://") {
        let _ = rest;
        let decoded = percent_encoding::percent_decode_str(&item[7..]).decode_utf8_lossy().to_string();
        let decoded = decoded.strip_prefix("localhost").unwrap_or(&decoded).to_string();
        return Source::File(PathBuf::from(decoded));
    }
    let p = PathBuf::from(item.replace('\\', "/"));
    if p.is_absolute() {
        Source::File(p)
    } else {
        match base {
            Some(b) => Source::File(b.join(p)),
            None => Source::File(p),
        }
    }
}

pub fn parse_m3u(text: &str, base: Option<&Path>) -> Vec<Entry> {
    let mut out = Vec::new();
    let mut pending: Option<(Option<Duration>, String)> = None;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("#EXTINF:") {
            let (len, title) = match rest.split_once(',') {
                Some((l, t)) => (l.trim(), t.trim()),
                None => (rest.trim(), ""),
            };
            let secs: Option<f64> = len.split_whitespace().next().and_then(|s| s.parse().ok());
            let dur = secs.filter(|s| *s > 0.0).map(Duration::from_secs_f64);
            pending = Some((dur, title.to_string()));
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        let mut e = Entry::from_source(resolve(line, base));
        if let Some((dur, title)) = pending.take() {
            if !title.is_empty() {
                e.title = title;
            }
            e.duration = dur;
        }
        out.push(e);
    }
    out
}

pub fn parse_pls(text: &str, base: Option<&Path>) -> Vec<Entry> {
    use std::collections::BTreeMap;
    let mut files: BTreeMap<usize, String> = BTreeMap::new();
    let mut titles: BTreeMap<usize, String> = BTreeMap::new();
    let mut lengths: BTreeMap<usize, i64> = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        let Some((k, v)) = line.split_once('=') else { continue };
        let k = k.trim();
        let v = v.trim();
        let num = |prefix: &str| k.strip_prefix(prefix).and_then(|n| n.parse::<usize>().ok());
        if let Some(n) = num("File") {
            files.insert(n, v.to_string());
        } else if let Some(n) = num("Title") {
            titles.insert(n, v.to_string());
        } else if let Some(n) = num("Length") {
            lengths.insert(n, v.parse().unwrap_or(-1));
        }
    }
    files
        .into_iter()
        .map(|(n, f)| {
            let mut e = Entry::from_source(resolve(&f, base));
            if let Some(t) = titles.get(&n) {
                if !t.is_empty() {
                    e.title = t.clone();
                }
            }
            if let Some(&l) = lengths.get(&n) {
                if l > 0 {
                    e.duration = Some(Duration::from_secs(l as u64));
                }
            }
            e
        })
        .collect()
}

fn source_string(src: &Source, base: Option<&Path>) -> String {
    match src {
        Source::Url(u) => u.clone(),
        Source::File(p) => {
            if let Some(b) = base {
                if let Ok(rel) = p.strip_prefix(b) {
                    return rel.to_string_lossy().to_string();
                }
            }
            p.to_string_lossy().to_string()
        }
    }
}

pub fn to_m3u(entries: &[Entry], base: Option<&Path>) -> String {
    let mut s = String::from("#EXTM3U\n");
    for e in entries {
        let secs = e.duration.map(|d| d.as_secs() as i64).unwrap_or(-1);
        s.push_str(&format!("#EXTINF:{},{}\n{}\n", secs, e.title, source_string(&e.source, base)));
    }
    s
}

pub fn to_pls(entries: &[Entry]) -> String {
    let mut s = String::from("[playlist]\n");
    for (i, e) in entries.iter().enumerate() {
        let n = i + 1;
        s.push_str(&format!("File{n}={}\n", source_string(&e.source, None)));
        s.push_str(&format!("Title{n}={}\n", e.title));
        s.push_str(&format!("Length{n}={}\n", e.duration.map(|d| d.as_secs() as i64).unwrap_or(-1)));
    }
    s.push_str(&format!("NumberOfEntries={}\nVersion=2\n", entries.len()));
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn m3u_roundtrip() {
        let text = "#EXTM3U\n#EXTINF:123,Artist - Song\n/music/a.mp3\nhttp://example.com/stream\n";
        let entries = parse_m3u(text, None);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].title, "Artist - Song");
        assert_eq!(entries[0].duration, Some(Duration::from_secs(123)));
        assert!(entries[1].source.is_url());
        let back = to_m3u(&entries, None);
        let again = parse_m3u(&back, None);
        assert_eq!(again[0].title, "Artist - Song");
    }

    #[test]
    fn pls_parse() {
        let text = "[playlist]\nFile1=http://x/y\nTitle1=Radio\nLength1=-1\nNumberOfEntries=1\n";
        let e = parse_pls(text, None);
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].title, "Radio");
    }

    #[test]
    fn navigation() {
        let mut pl = Playlist::new();
        for i in 0..3 {
            pl.push(Entry::from_source(Source::Url(format!("http://x/{i}"))));
        }
        assert_eq!(pl.next_index(false, false), Some(0));
        pl.set_current(2);
        assert_eq!(pl.next_index(false, false), None);
        assert_eq!(pl.next_index(false, true), Some(0));
        assert_eq!(pl.prev_index(false, false), Some(1));
        let mut seen = std::collections::HashSet::new();
        pl.set_current(0);
        seen.insert(0);
        for _ in 0..2 {
            seen.insert(pl.next_index(true, false).unwrap());
        }
        assert_eq!(seen.len(), 3);
        assert_eq!(pl.next_index(true, false), None);
    }

    #[test]
    fn move_entries_reorders() {
        let mut pl = Playlist::new();
        for i in 0..5 {
            pl.push(Entry::from_source(Source::Url(format!("{i}"))));
        }
        pl.move_entries(&[0, 1], 4);
        let order: Vec<String> = pl.entries.iter().map(|e| e.source.as_display()).collect();
        assert_eq!(order, vec!["2", "3", "0", "1", "4"]);
    }
}
