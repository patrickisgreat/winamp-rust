//! Playlist model plus M3U / M3U8 / PLS reading and writing.

use std::collections::{BTreeMap, HashSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use rand::seq::SliceRandom;
use winamp_audio::{read_meta, Source, TrackMeta};

/// One row of the playlist.
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
            Source::File(path) => path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
            Source::Url(url) => url.clone(),
        };
        Self {
            source,
            title,
            duration: None,
            meta: None,
            selected: false,
        }
    }

    /// Read tags for local files (cheap enough to do synchronously for a few
    /// hundred files; the app also defers this to a worker for big loads).
    pub fn read_tags(&mut self) {
        if let Source::File(path) = &self.source {
            let meta = read_meta(path);
            self.title = meta.display_title(path);
            self.duration = meta.duration;
            self.meta = Some(meta);
        }
    }

    /// The title as shown in the playlist editor, e.g. "3. Artist - Title".
    pub fn display_with_number(&self, number: usize) -> String {
        format!("{number}. {}", self.title)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortKey {
    Title,
    Filename,
    Path,
}

/// A sum of entry durations. Entries whose length is not known yet are
/// skipped, which `all_known` reports so the UI can show a "+" suffix.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RunningTime {
    pub total: Duration,
    pub all_known: bool,
}

impl RunningTime {
    fn of<'a>(entries: impl Iterator<Item = &'a Entry>) -> Self {
        entries.fold(
            RunningTime {
                total: Duration::ZERO,
                all_known: true,
            },
            |sum, entry| match entry.duration {
                Some(duration) => RunningTime {
                    total: sum.total + duration,
                    ..sum
                },
                None => RunningTime {
                    all_known: false,
                    ..sum
                },
            },
        )
    }
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

    /// The source of the current track, if any.
    pub fn current_source(&self) -> Option<&Source> {
        self.current
            .and_then(|index| self.entries.get(index))
            .map(|entry| &entry.source)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.current = None;
        self.invalidate_shuffle();
    }

    /// Append an entry and return its index.
    pub fn push(&mut self, entry: Entry) -> usize {
        self.entries.push(entry);
        self.invalidate_shuffle();
        self.entries.len() - 1
    }

    pub fn insert(&mut self, index: usize, entry: Entry) {
        let index = index.min(self.entries.len());
        self.entries.insert(index, entry);
        if let Some(current) = self.current {
            if index <= current {
                self.current = Some(current + 1);
            }
        }
        self.invalidate_shuffle();
    }

    /// Add a file, URL, directory (recursively) or playlist file. Returns the
    /// number of entries added.
    pub fn add_path(&mut self, path: impl AsRef<Path>) -> usize {
        let path = path.as_ref();
        if path.is_dir() {
            let mut files: Vec<PathBuf> = walkdir::WalkDir::new(path)
                .follow_links(true)
                .into_iter()
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry.file_type().is_file())
                .map(|entry| entry.into_path())
                .filter(|file| winamp_audio::is_supported_path(file))
                .collect();
            files.sort();
            let added = files.len();
            for file in files {
                self.push(Entry::from_source(Source::File(file)));
            }
            added
        } else if is_playlist_path(path) {
            match load_playlist_file(path) {
                Ok(entries) => {
                    let added = entries.len();
                    for entry in entries {
                        self.push(entry);
                    }
                    added
                }
                Err(e) => {
                    log::warn!("loading playlist {}: {e}", path.display());
                    0
                }
            }
        } else {
            self.push(Entry::from_source(Source::File(path.to_path_buf())));
            1
        }
    }

    pub fn add_url(&mut self, url: impl Into<String>) {
        self.push(Entry::from_source(Source::Url(url.into())));
    }

    /// Remove the entries at `indices` (duplicates and out-of-range indices
    /// are ignored), keeping `current` pointing at the same track.
    pub fn remove_indices(&mut self, mut indices: Vec<usize>) {
        indices.sort_unstable();
        indices.dedup();
        for &index in indices.iter().rev() {
            if index >= self.entries.len() {
                continue;
            }
            self.entries.remove(index);
            self.current = match self.current {
                Some(current) if index < current => Some(current - 1),
                Some(current) if index == current => None,
                other => other,
            };
        }
        self.invalidate_shuffle();
    }

    pub fn remove_selected(&mut self) {
        self.remove_indices(self.selected_indices());
    }

    /// Keep only the selected entries ("Crop").
    pub fn crop(&mut self) {
        self.remove_indices(self.indices_where(|entry| !entry.selected));
    }

    pub fn remove_missing_files(&mut self) {
        self.remove_indices(
            self.indices_where(|entry| matches!(&entry.source, Source::File(p) if !p.exists())),
        );
    }

    /// Remove later repeats of the same source, keeping the first occurrence.
    pub fn remove_duplicates(&mut self) {
        let mut seen = HashSet::new();
        let duplicates = self.indices_where(|entry| !seen.insert(&entry.source));
        self.remove_indices(duplicates);
    }

    pub fn selected_indices(&self) -> Vec<usize> {
        self.indices_where(|entry| entry.selected)
    }

    fn indices_where<'a>(&'a self, mut predicate: impl FnMut(&'a Entry) -> bool) -> Vec<usize> {
        self.entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| predicate(entry).then_some(index))
            .collect()
    }

    pub fn select_all(&mut self) {
        self.entries
            .iter_mut()
            .for_each(|entry| entry.selected = true);
    }

    pub fn select_none(&mut self) {
        self.entries
            .iter_mut()
            .for_each(|entry| entry.selected = false);
    }

    pub fn invert_selection(&mut self) {
        self.entries
            .iter_mut()
            .for_each(|entry| entry.selected = !entry.selected);
    }

    pub fn sort(&mut self, key: SortKey, reverse: bool) {
        self.reorder(|entries| {
            entries.sort_by_cached_key(|entry| match key {
                SortKey::Title => entry.title.to_lowercase(),
                SortKey::Filename => match &entry.source {
                    Source::File(path) => path
                        .file_name()
                        .map(|s| s.to_string_lossy().to_lowercase())
                        .unwrap_or_default(),
                    Source::Url(url) => url.to_lowercase(),
                },
                SortKey::Path => entry.source.to_string().to_lowercase(),
            });
            if reverse {
                entries.reverse();
            }
        });
    }

    pub fn reverse(&mut self) {
        self.reorder(|entries| entries.reverse());
    }

    pub fn randomize(&mut self) {
        self.reorder(|entries| entries.shuffle(&mut rand::thread_rng()));
    }

    /// Move the selected entries up (`delta < 0`) or down by one slot.
    pub fn move_selected(&mut self, delta: i32) {
        let selected = self.selected_indices();
        let (Some(&first), Some(&last)) = (selected.first(), selected.last()) else {
            return;
        };
        if delta < 0 {
            if first == 0 {
                return;
            }
            self.reorder(|entries| {
                for &index in &selected {
                    entries.swap(index, index - 1);
                }
            });
        } else {
            if last + 1 >= self.entries.len() {
                return;
            }
            self.reorder(|entries| {
                for &index in selected.iter().rev() {
                    entries.swap(index, index + 1);
                }
            });
        }
    }

    /// Move a block of entries so that it starts at `target` (drag and drop
    /// reorder). `target` is a position in the list *before* the move.
    pub fn move_entries(&mut self, indices: &[usize], target: usize) {
        if indices.is_empty() {
            return;
        }
        self.reorder(|entries| {
            let mut moving = Vec::new();
            let mut rest = Vec::new();
            let mut moved_from_before_target = 0usize;
            for (index, entry) in entries.drain(..).enumerate() {
                if indices.contains(&index) {
                    if index < target {
                        moved_from_before_target += 1;
                    }
                    moving.push(entry);
                } else {
                    rest.push(entry);
                }
            }
            let insert_at = target
                .saturating_sub(moved_from_before_target)
                .min(rest.len());
            let tail = rest.split_off(insert_at);
            rest.extend(moving);
            rest.extend(tail);
            *entries = rest;
        });
    }

    /// Apply a reordering of `entries`, keeping `current` on the same track
    /// and discarding the now-stale shuffle order.
    fn reorder(&mut self, reorder: impl FnOnce(&mut Vec<Entry>)) {
        let current_source = self.current_source().cloned();
        reorder(&mut self.entries);
        self.current = current_source
            .and_then(|source| self.entries.iter().position(|entry| entry.source == source));
        self.invalidate_shuffle();
    }

    pub fn total_duration(&self) -> RunningTime {
        RunningTime::of(self.entries.iter())
    }

    pub fn selected_duration(&self) -> RunningTime {
        RunningTime::of(self.entries.iter().filter(|entry| entry.selected))
    }

    // --- navigation -------------------------------------------------------

    fn invalidate_shuffle(&mut self) {
        self.shuffle_order.clear();
        self.shuffle_pos = 0;
    }

    fn ensure_shuffle(&mut self) {
        if self.shuffle_order.len() == self.entries.len() {
            return;
        }
        self.shuffle_order = (0..self.entries.len()).collect();
        self.shuffle_order.shuffle(&mut rand::thread_rng());
        // Start from the current track so "next" doesn't replay it immediately.
        if let Some(current) = self.current {
            if let Some(position) = self.shuffle_order.iter().position(|&i| i == current) {
                self.shuffle_order.swap(0, position);
            }
        }
        self.shuffle_pos = 0;
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
            if !repeat {
                return None;
            }
            self.invalidate_shuffle();
            self.ensure_shuffle();
            self.shuffle_pos = usize::from(self.shuffle_order.len() > 1);
            return Some(self.shuffle_order[self.shuffle_pos]);
        }
        match self.current {
            None => Some(0),
            Some(current) if current + 1 < self.entries.len() => Some(current + 1),
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
            if !repeat {
                return None;
            }
            self.shuffle_pos = self.shuffle_order.len() - 1;
            return Some(self.shuffle_order[self.shuffle_pos]);
        }
        match self.current {
            None => Some(0),
            Some(0) if repeat => Some(self.entries.len() - 1),
            Some(0) => None,
            Some(current) => Some(current - 1),
        }
    }

    /// Record that `index` became the current track (keeps shuffle history in sync).
    pub fn set_current(&mut self, index: usize) {
        if index >= self.entries.len() {
            return;
        }
        self.current = Some(index);
        if self.shuffle_order.len() == self.entries.len() {
            if let Some(position) = self.shuffle_order.iter().position(|&i| i == index) {
                // Move the chosen track to the current shuffle slot so history stays linear.
                let slot = self.shuffle_pos.min(self.shuffle_order.len() - 1);
                self.shuffle_order.swap(slot, position);
            }
        }
    }

    // --- persistence --------------------------------------------------------

    /// Replace the contents with a playlist file and remember its path.
    pub fn load_file(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let entries = load_playlist_file(path)?;
        self.clear();
        self.entries = entries;
        self.path = Some(path.to_path_buf());
        Ok(())
    }

    /// Write the playlist as PLS (for a `.pls` extension) or extended M3U.
    pub fn save_file(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let text = match lowercase_extension(path).as_deref() {
            Some("pls") => to_pls(&self.entries),
            _ => to_m3u(&self.entries, path.parent()),
        };
        std::fs::write(path, text).with_context(|| format!("writing {}", path.display()))?;
        self.path = Some(path.to_path_buf());
        Ok(())
    }
}

fn lowercase_extension(path: &Path) -> Option<String> {
    path.extension()
        .map(|ext| ext.to_string_lossy().to_ascii_lowercase())
}

pub fn is_playlist_path(path: impl AsRef<Path>) -> bool {
    matches!(
        lowercase_extension(path.as_ref()).as_deref(),
        Some("m3u" | "m3u8" | "pls")
    )
}

pub fn load_playlist_file(path: impl AsRef<Path>) -> Result<Vec<Entry>> {
    let path = path.as_ref();
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let text = decode_playlist_text(&bytes);
    let base = path.parent();
    Ok(match lowercase_extension(path).as_deref() {
        Some("pls") => parse_pls(&text, base),
        _ => parse_m3u(&text, base),
    })
}

/// Parse playlist text fetched from a URL (remote .pls / .m3u).
pub fn parse_remote_playlist(text: &str, url: &str) -> Vec<Entry> {
    let looks_like_pls = url.to_ascii_lowercase().contains(".pls")
        || text
            .trim_start()
            .to_ascii_lowercase()
            .starts_with("[playlist]");
    if looks_like_pls {
        parse_pls(text, None)
    } else {
        parse_m3u(text, None)
    }
}

/// Decode a playlist file as UTF-8 (with or without BOM), falling back to
/// Latin-1 for legacy `.m3u` files.
fn decode_playlist_text(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_string(),
        Err(_) => bytes.iter().map(|&b| b as char).collect(),
    }
}

/// Turn one playlist item into a source: URLs stay URLs, `file://` is
/// decoded, and relative paths are resolved against `base`.
fn resolve_source(item: &str, base: Option<&Path>) -> Source {
    let item = item.trim().trim_matches('"');
    if winamp_audio::decode::is_http_url(item) {
        return Source::Url(item.to_string());
    }
    if item.len() >= 7 && item[..7].eq_ignore_ascii_case("file://") {
        let decoded = percent_encoding::percent_decode_str(&item[7..]).decode_utf8_lossy();
        let path = decoded.strip_prefix("localhost").unwrap_or(&decoded);
        return Source::File(PathBuf::from(path));
    }
    let path = PathBuf::from(item.replace('\\', "/"));
    match base {
        Some(base) if !path.is_absolute() => Source::File(base.join(path)),
        _ => Source::File(path),
    }
}

pub fn parse_m3u(text: &str, base: Option<&Path>) -> Vec<Entry> {
    let mut entries = Vec::new();
    let mut pending_extinf: Option<(Option<Duration>, String)> = None;
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if let Some(rest) = line.strip_prefix("#EXTINF:") {
            let (length, title) = match rest.split_once(',') {
                Some((length, title)) => (length.trim(), title.trim()),
                None => (rest.trim(), ""),
            };
            let secs: Option<f64> = length
                .split_whitespace()
                .next()
                .and_then(|s| s.parse().ok());
            let duration = secs.filter(|s| *s > 0.0).map(Duration::from_secs_f64);
            pending_extinf = Some((duration, title.to_string()));
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        let mut entry = Entry::from_source(resolve_source(line, base));
        if let Some((duration, title)) = pending_extinf.take() {
            if !title.is_empty() {
                entry.title = title;
            }
            entry.duration = duration;
        }
        entries.push(entry);
    }
    entries
}

pub fn parse_pls(text: &str, base: Option<&Path>) -> Vec<Entry> {
    let mut files: BTreeMap<usize, String> = BTreeMap::new();
    let mut titles: BTreeMap<usize, String> = BTreeMap::new();
    let mut lengths: BTreeMap<usize, i64> = BTreeMap::new();
    for line in text.lines() {
        let Some((key, value)) = line.trim().split_once('=') else {
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        let numbered = |prefix: &str| {
            key.strip_prefix(prefix)
                .and_then(|n| n.parse::<usize>().ok())
        };
        if let Some(n) = numbered("File") {
            files.insert(n, value.to_string());
        } else if let Some(n) = numbered("Title") {
            titles.insert(n, value.to_string());
        } else if let Some(n) = numbered("Length") {
            lengths.insert(n, value.parse().unwrap_or(-1));
        }
    }
    files
        .into_iter()
        .map(|(n, file)| {
            let mut entry = Entry::from_source(resolve_source(&file, base));
            if let Some(title) = titles.remove(&n).filter(|t| !t.is_empty()) {
                entry.title = title;
            }
            if let Some(length) = lengths.get(&n).copied().filter(|&l| l > 0) {
                entry.duration = Some(Duration::from_secs(length as u64));
            }
            entry
        })
        .collect()
}

/// How a source is written into a playlist file: URLs verbatim, files
/// relative to `base` when possible.
fn source_string(source: &Source, base: Option<&Path>) -> String {
    match source {
        Source::Url(url) => url.clone(),
        Source::File(path) => base
            .and_then(|base| path.strip_prefix(base).ok())
            .unwrap_or(path)
            .to_string_lossy()
            .into_owned(),
    }
}

fn length_secs(entry: &Entry) -> i64 {
    entry.duration.map(|d| d.as_secs() as i64).unwrap_or(-1)
}

pub fn to_m3u(entries: &[Entry], base: Option<&Path>) -> String {
    let mut text = String::from("#EXTM3U\n");
    for entry in entries {
        let _ = writeln!(text, "#EXTINF:{},{}", length_secs(entry), entry.title);
        let _ = writeln!(text, "{}", source_string(&entry.source, base));
    }
    text
}

pub fn to_pls(entries: &[Entry]) -> String {
    let mut text = String::from("[playlist]\n");
    for (i, entry) in entries.iter().enumerate() {
        let n = i + 1;
        let _ = writeln!(text, "File{n}={}", source_string(&entry.source, None));
        let _ = writeln!(text, "Title{n}={}", entry.title);
        let _ = writeln!(text, "Length{n}={}", length_secs(entry));
    }
    let _ = write!(text, "NumberOfEntries={}\nVersion=2\n", entries.len());
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url_entry(name: &str) -> Entry {
        Entry::from_source(Source::Url(name.to_string()))
    }

    fn playlist_of(names: &[&str]) -> Playlist {
        let mut playlist = Playlist::new();
        for name in names {
            playlist.push(url_entry(name));
        }
        playlist
    }

    fn order(playlist: &Playlist) -> Vec<String> {
        playlist
            .entries
            .iter()
            .map(|e| e.source.to_string())
            .collect()
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "winamp-playlist-tests-{}-{name}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    // --- parsing ------------------------------------------------------------

    #[test]
    fn m3u_roundtrip() {
        let text = "#EXTM3U\n#EXTINF:123,Artist - Song\n/music/a.mp3\nhttp://example.com/stream\n";
        let entries = parse_m3u(text, None);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].title, "Artist - Song");
        assert_eq!(entries[0].duration, Some(Duration::from_secs(123)));
        assert!(entries[1].source.is_url());
        assert_eq!(entries[1].duration, None);
        let back = to_m3u(&entries, None);
        assert_eq!(parse_m3u(&back, None)[0].title, "Artist - Song");
    }

    #[test]
    fn m3u_resolves_relative_paths_and_file_urls() {
        let text = "sub\\track.mp3\n/abs/track.flac\nfile:///tmp/with%20space.ogg\nfile://localhost/tmp/x.wav\n#comment\n";
        let entries = parse_m3u(text, Some(Path::new("/base")));
        assert_eq!(
            order(&Playlist {
                entries,
                ..Default::default()
            }),
            vec![
                "/base/sub/track.mp3",
                "/abs/track.flac",
                "/tmp/with space.ogg",
                "/tmp/x.wav",
            ]
        );
    }

    #[test]
    fn m3u_extinf_without_title_keeps_file_stem() {
        let entries = parse_m3u("#EXTINF:-1,\n/a/b/song.mp3\n#EXTINF:0\n/a/c.mp3", None);
        assert_eq!(entries[0].title, "song");
        assert_eq!(entries[0].duration, None);
        assert_eq!(entries[1].duration, None);
    }

    #[test]
    fn pls_parse_and_roundtrip() {
        let text = "[playlist]\nFile1=http://x/y\nTitle1=Radio\nLength1=-1\nFile2=/m/a.mp3\nLength2=200\nNumberOfEntries=2\n";
        let entries = parse_pls(text, None);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].title, "Radio");
        assert_eq!(entries[1].title, "a");
        assert_eq!(entries[1].duration, Some(Duration::from_secs(200)));
        let back = parse_pls(&to_pls(&entries), None);
        assert_eq!(back.len(), 2);
        assert_eq!(back[1].duration, Some(Duration::from_secs(200)));
    }

    #[test]
    fn remote_playlist_detects_pls_by_url_or_body() {
        let pls = "[playlist]\nFile1=http://a\n";
        assert_eq!(parse_remote_playlist(pls, "http://x/list.m3u").len(), 1);
        assert_eq!(
            parse_remote_playlist("http://a\nhttp://b\n", "http://x/list.pls").len(),
            0
        );
        assert_eq!(
            parse_remote_playlist("http://a\nhttp://b\n", "http://x/list.m3u").len(),
            2
        );
    }

    #[test]
    fn decodes_bom_and_latin1() {
        assert_eq!(decode_playlist_text(b"\xEF\xBB\xBFabc"), "abc");
        assert_eq!(decode_playlist_text(b"caf\xE9"), "café");
    }

    #[test]
    fn recognises_playlist_extensions() {
        assert!(is_playlist_path("a.M3U"));
        assert!(is_playlist_path("/x/a.m3u8"));
        assert!(is_playlist_path("a.pls"));
        assert!(!is_playlist_path("a.mp3"));
    }

    #[test]
    fn save_and_load_file_use_relative_paths() {
        let dir = temp_dir("save");
        let mut playlist = Playlist::new();
        playlist.push(Entry::from_source(Source::File(dir.join("song.mp3"))));
        playlist.add_url("http://stream");
        let path = dir.join("list.m3u8");
        playlist.save_file(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\nsong.mp3\n"), "{text}");

        let mut loaded = Playlist::new();
        loaded.load_file(&path).unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded.entries[0].source, Source::File(dir.join("song.mp3")));
        assert_eq!(loaded.path.as_deref(), Some(path.as_path()));

        let pls = dir.join("list.pls");
        playlist.save_file(&pls).unwrap();
        assert!(std::fs::read_to_string(&pls)
            .unwrap()
            .starts_with("[playlist]"));
    }

    #[test]
    fn load_file_reports_missing_file() {
        let mut playlist = Playlist::new();
        assert!(playlist.load_file("/no/such/list.m3u").is_err());
    }

    #[test]
    fn add_path_scans_directories_for_supported_files() {
        let dir = temp_dir("scan");
        std::fs::write(dir.join("b.mp3"), b"").unwrap();
        std::fs::write(dir.join("a.flac"), b"").unwrap();
        std::fs::write(dir.join("notes.txt"), b"").unwrap();
        std::fs::create_dir_all(dir.join("nested")).unwrap();
        std::fs::write(dir.join("nested/c.ogg"), b"").unwrap();
        let mut playlist = Playlist::new();
        assert_eq!(playlist.add_path(&dir), 3);
        let titles: Vec<&str> = playlist.entries.iter().map(|e| e.title.as_str()).collect();
        assert_eq!(titles, vec!["a", "b", "c"]);
        assert_eq!(playlist.add_path(dir.join("single.wav")), 1);
        assert_eq!(playlist.len(), 4);
    }

    // --- editing --------------------------------------------------------------

    #[test]
    fn insert_and_remove_keep_current_on_the_same_track() {
        let mut playlist = playlist_of(&["a", "b", "c"]);
        playlist.set_current(1);
        playlist.insert(0, url_entry("z"));
        assert_eq!(playlist.current, Some(2));
        playlist.insert(99, url_entry("end"));
        assert_eq!(order(&playlist), vec!["z", "a", "b", "c", "end"]);
        assert_eq!(playlist.current, Some(2));
        playlist.remove_indices(vec![0, 0, 4, 42]);
        assert_eq!(order(&playlist), vec!["a", "b", "c"]);
        assert_eq!(playlist.current, Some(1));
        assert_eq!(playlist.current_source(), Some(&Source::Url("b".into())));
        playlist.remove_indices(vec![1]);
        assert_eq!(playlist.current, None);
        assert_eq!(playlist.current_source(), None);
    }

    #[test]
    fn selection_helpers() {
        let mut playlist = playlist_of(&["a", "b", "c", "d"]);
        playlist.entries[1].selected = true;
        playlist.entries[3].selected = true;
        assert_eq!(playlist.selected_indices(), vec![1, 3]);
        playlist.invert_selection();
        assert_eq!(playlist.selected_indices(), vec![0, 2]);
        playlist.crop();
        assert_eq!(order(&playlist), vec!["a", "c"]);
        playlist.select_all();
        playlist.remove_selected();
        assert!(playlist.is_empty());
    }

    #[test]
    fn remove_duplicates_keeps_first_occurrence() {
        let mut playlist = playlist_of(&["a", "b", "a", "c", "b"]);
        playlist.set_current(2);
        playlist.remove_duplicates();
        assert_eq!(order(&playlist), vec!["a", "b", "c"]);
    }

    #[test]
    fn remove_missing_files_ignores_urls() {
        let dir = temp_dir("missing");
        let present = dir.join("here.mp3");
        std::fs::write(&present, b"").unwrap();
        let mut playlist = Playlist::new();
        playlist.push(Entry::from_source(Source::File(present)));
        playlist.push(Entry::from_source(Source::File(dir.join("gone.mp3"))));
        playlist.add_url("http://x");
        playlist.remove_missing_files();
        assert_eq!(playlist.len(), 2);
    }

    #[test]
    fn sort_reverse_and_randomize_follow_current() {
        let mut playlist = playlist_of(&["Charlie", "alpha", "Bravo"]);
        playlist.set_current(2);
        playlist.sort(SortKey::Title, false);
        assert_eq!(order(&playlist), vec!["alpha", "Bravo", "Charlie"]);
        assert_eq!(playlist.current, Some(1));
        playlist.sort(SortKey::Title, true);
        assert_eq!(order(&playlist), vec!["Charlie", "Bravo", "alpha"]);
        playlist.reverse();
        assert_eq!(order(&playlist), vec!["alpha", "Bravo", "Charlie"]);
        assert_eq!(playlist.current, Some(1));
        playlist.randomize();
        assert_eq!(
            playlist.current_source(),
            Some(&Source::Url("Bravo".into()))
        );
    }

    #[test]
    fn sort_by_filename_uses_the_last_path_segment() {
        let mut playlist = Playlist::new();
        playlist.push(Entry::from_source(Source::File("/z/a.mp3".into())));
        playlist.push(Entry::from_source(Source::File("/a/z.mp3".into())));
        playlist.sort(SortKey::Filename, false);
        assert_eq!(order(&playlist), vec!["/z/a.mp3", "/a/z.mp3"]);
        playlist.sort(SortKey::Path, false);
        assert_eq!(order(&playlist), vec!["/a/z.mp3", "/z/a.mp3"]);
    }

    #[test]
    fn move_selected_respects_list_bounds() {
        let mut playlist = playlist_of(&["a", "b", "c"]);
        playlist.entries[0].selected = true;
        playlist.move_selected(-1);
        assert_eq!(
            order(&playlist),
            vec!["a", "b", "c"],
            "cannot move the first entry up"
        );
        playlist.move_selected(1);
        assert_eq!(order(&playlist), vec!["b", "a", "c"]);
        playlist.move_selected(1);
        assert_eq!(order(&playlist), vec!["b", "c", "a"]);
        playlist.move_selected(1);
        assert_eq!(
            order(&playlist),
            vec!["b", "c", "a"],
            "cannot move the last entry down"
        );
        playlist.select_none();
        playlist.move_selected(-1);
        assert_eq!(order(&playlist), vec!["b", "c", "a"]);
    }

    #[test]
    fn move_entries_reorders() {
        let mut playlist = playlist_of(&["0", "1", "2", "3", "4"]);
        playlist.set_current(0);
        playlist.move_entries(&[0, 1], 4);
        assert_eq!(order(&playlist), vec!["2", "3", "0", "1", "4"]);
        assert_eq!(playlist.current, Some(2));
        playlist.move_entries(&[4], 0);
        assert_eq!(order(&playlist), vec!["4", "2", "3", "0", "1"]);
        playlist.move_entries(&[], 0);
        assert_eq!(order(&playlist), vec!["4", "2", "3", "0", "1"]);
    }

    #[test]
    fn running_time_flags_unknown_lengths() {
        let mut playlist = playlist_of(&["a", "b", "c"]);
        playlist.entries[0].duration = Some(Duration::from_secs(60));
        playlist.entries[1].duration = Some(Duration::from_secs(30));
        playlist.entries[1].selected = true;
        assert_eq!(
            playlist.total_duration(),
            RunningTime {
                total: Duration::from_secs(90),
                all_known: false
            }
        );
        assert_eq!(
            playlist.selected_duration(),
            RunningTime {
                total: Duration::from_secs(30),
                all_known: true
            }
        );
        assert_eq!(
            Playlist::new().total_duration(),
            RunningTime {
                total: Duration::ZERO,
                all_known: true
            }
        );
    }

    // --- navigation -----------------------------------------------------------

    #[test]
    fn linear_navigation() {
        let mut playlist = playlist_of(&["a", "b", "c"]);
        assert_eq!(playlist.next_index(false, false), Some(0));
        assert_eq!(playlist.prev_index(false, false), Some(0));
        playlist.set_current(2);
        assert_eq!(playlist.next_index(false, false), None);
        assert_eq!(playlist.next_index(false, true), Some(0));
        assert_eq!(playlist.prev_index(false, false), Some(1));
        playlist.set_current(0);
        assert_eq!(playlist.prev_index(false, false), None);
        assert_eq!(playlist.prev_index(false, true), Some(2));
        assert_eq!(Playlist::new().next_index(false, true), None);
    }

    #[test]
    fn shuffle_visits_every_track_once_and_can_wrap() {
        let mut playlist = playlist_of(&["a", "b", "c", "d"]);
        playlist.set_current(0);
        let mut seen = HashSet::from([0]);
        for _ in 0..3 {
            let next = playlist.next_index(true, false).unwrap();
            assert!(seen.insert(next), "track {next} repeated");
            playlist.set_current(next);
        }
        assert_eq!(seen.len(), 4);
        assert_eq!(playlist.next_index(true, false), None);
        let wrapped = playlist.next_index(true, true).unwrap();
        assert!(wrapped < 4);
    }

    #[test]
    fn shuffle_prev_walks_back_through_history() {
        let mut playlist = playlist_of(&["a", "b", "c"]);
        playlist.set_current(0);
        let first = playlist.next_index(true, false).unwrap();
        playlist.set_current(first);
        let second = playlist.next_index(true, false).unwrap();
        playlist.set_current(second);
        assert_eq!(playlist.prev_index(true, false), Some(first));
        assert_eq!(playlist.prev_index(true, false), Some(0));
        assert_eq!(playlist.prev_index(true, false), None);
        assert_eq!(playlist.prev_index(true, true), Some(second));
    }

    #[test]
    fn set_current_ignores_out_of_range() {
        let mut playlist = playlist_of(&["a"]);
        playlist.set_current(5);
        assert_eq!(playlist.current, None);
    }
}
