use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use encoding_rs::GBK;
use fuzzengine::{PreprocessingOptions, partial_ratio, partial_token_set_ratio};
use lrc::Lyrics;

mod providers;

#[derive(Clone, Default, Debug)]
pub struct LyricLine {
    pub time_ms: u64,
    pub text: String,
    pub(crate) secondary_text: Option<String>,
    timings: Vec<LyricTiming>,
}

#[derive(Clone, Debug)]
struct LyricTiming {
    start_time_ms: u64,
    end_time_ms: Option<u64>,
    end_byte: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct LyricHighlight {
    pub(crate) start_byte: usize,
    pub(crate) end_byte: usize,
    pub(crate) progress: f32,
}

impl LyricLine {
    pub(crate) fn is_word_synced(&self) -> bool {
        !self.timings.is_empty()
    }

    pub(crate) fn replace_text_preserving_timings(&mut self, text: String) -> bool {
        if self.timings.is_empty() {
            self.text = text;
            return true;
        }

        let boundaries = self
            .timings
            .iter()
            .map(|timing| {
                self.text
                    .get(..timing.end_byte)
                    .map(|prefix| prefix.chars().count())
            })
            .collect::<Option<Vec<_>>>();
        let Some(boundaries) = boundaries else {
            return false;
        };
        let byte_offsets = text
            .char_indices()
            .map(|(index, _)| index)
            .chain(std::iter::once(text.len()))
            .collect::<Vec<_>>();
        if byte_offsets.len() != self.text.chars().count() + 1 {
            return false;
        }
        for (timing, boundary) in self.timings.iter_mut().zip(boundaries) {
            let Some(&end_byte) = byte_offsets.get(boundary) else {
                return false;
            };
            timing.end_byte = end_byte;
        }
        self.text = text;
        true
    }

    pub(crate) fn highlight_at(
        &self,
        position_ms: u64,
        next_line_time_ms: Option<u64>,
    ) -> Option<LyricHighlight> {
        if self.timings.is_empty() {
            return None;
        }

        let started = self
            .timings
            .partition_point(|timing| timing.start_time_ms <= position_ms);
        let index = started.saturating_sub(1);
        let timing = &self.timings[index];
        let start_byte = index
            .checked_sub(1)
            .map_or(0, |previous| self.timings[previous].end_byte);
        let end_time_ms = timing
            .end_time_ms
            .or_else(|| self.timings.get(index + 1).map(|next| next.start_time_ms))
            .unwrap_or_else(|| {
                let previous_duration = index
                    .checked_sub(1)
                    .map(|previous| {
                        timing
                            .start_time_ms
                            .saturating_sub(self.timings[previous].start_time_ms)
                    })
                    .filter(|duration| *duration > 0)
                    .unwrap_or(400)
                    .clamp(80, 1000);
                let estimated_end = timing.start_time_ms.saturating_add(previous_duration);
                next_line_time_ms
                    .filter(|next| *next > timing.start_time_ms)
                    .map_or(estimated_end, |next| next.min(estimated_end))
            })
            .max(timing.start_time_ms.saturating_add(1));
        let progress = if started == 0 {
            0.0
        } else {
            position_ms
                .saturating_sub(timing.start_time_ms)
                .min(end_time_ms - timing.start_time_ms) as f32
                / (end_time_ms - timing.start_time_ms) as f32
        };

        Some(LyricHighlight {
            start_byte,
            end_byte: timing.end_byte,
            progress,
        })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum LyricsMode {
    #[default]
    Online,
    Lrc,
}

impl From<&str> for LyricsMode {
    fn from(value: &str) -> Self {
        match value {
            "lrc" => Self::Lrc,
            _ => Self::Online,
        }
    }
}

const MAX_LOCAL_LRC_FILES: usize = 2048;

fn fetch_lyrics_local(title: &str, artist: &str, local_dir: &str) -> Option<Arc<Vec<LyricLine>>> {
    let title_key = MatchKey::new(title);
    if title_key.compact.is_empty() {
        return None;
    }
    let artist_key = MatchKey::new(artist);
    let local_dir = Path::new(local_dir);
    if !local_dir.is_dir() {
        return None;
    }

    let mut candidates = collect_lrc_candidates(local_dir, &title_key, &artist_key);
    candidates.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    for (_, path) in candidates {
        if let Some(content) = read_lrc_text(&path) {
            let lines = parse_lyrics(&content, "");
            if !lines.is_empty() {
                return Some(Arc::new(lines));
            }
        }
    }
    None
}

#[derive(Clone)]
struct MatchKey {
    normalized: String,
    compact: String,
    simplified_compact: String,
}

impl MatchKey {
    fn new(value: &str) -> Self {
        let normalized = normalize_match_text(value, false);
        let simplified = normalize_match_text(value, true);
        Self {
            compact: compact(&normalized),
            simplified_compact: compact(&simplified),
            normalized,
        }
    }

    fn variants(&self) -> impl Iterator<Item = &str> {
        [self.compact.as_str(), self.simplified_compact.as_str()]
            .into_iter()
            .filter(|value| !value.is_empty())
    }

    fn matches(&self, other: &Self) -> bool {
        self.variants()
            .any(|left| other.variants().any(|right| left == right))
    }
}

fn collect_lrc_candidates(
    local_dir: &Path,
    title: &MatchKey,
    artist: &MatchKey,
) -> Vec<(u16, PathBuf)> {
    let Ok(entries) = fs::read_dir(local_dir) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .take(MAX_LOCAL_LRC_FILES)
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_file()
                || !path
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("lrc"))
            {
                return None;
            }
            let stem = path.file_stem()?.to_str()?;
            let filename_score = score_lrc_match(&MatchKey::new(stem), title, artist);
            let metadata_score = read_lrc_text_prefix(&path)
                .map(|content| score_lrc_metadata(&content, title, artist))
                .unwrap_or_default();
            let score = filename_score.max(metadata_score);
            (score >= 80).then_some((score, path))
        })
        .collect()
}

fn score_lrc_match(candidate: &MatchKey, title: &MatchKey, artist: &MatchKey) -> u16 {
    if candidate.matches(title) {
        return 110;
    }
    if candidate.variants().any(|candidate| {
        title.variants().any(|title| {
            artist.variants().any(|artist| {
                !artist.is_empty()
                    && (candidate == format!("{artist}{title}")
                        || candidate == format!("{title}{artist}"))
            })
        })
    }) {
        return 120;
    }
    if let Some(score) = fuzzy_title_score(candidate, title, artist) {
        return score;
    }
    if title.simplified_compact.chars().count() < 3 {
        return 0;
    }
    let contains_title = candidate
        .variants()
        .any(|candidate| title.variants().any(|title| candidate.contains(title)));
    if !contains_title {
        return 0;
    }
    let contains_artist = artist.variants().any(|artist| {
        candidate
            .variants()
            .any(|candidate| candidate.contains(artist))
    });
    if contains_artist { 100 } else { 80 }
}

fn fuzzy_title_score(candidate: &MatchKey, title: &MatchKey, artist: &MatchKey) -> Option<u16> {
    let options = PreprocessingOptions {
        force_ascii: false,
        strip: true,
    };
    let mut queries = vec![title.simplified_compact.as_str()];
    let title_without_artist = remove_component(&title.simplified_compact, artist);
    if title_without_artist != title.simplified_compact && !title_without_artist.is_empty() {
        queries.push(&title_without_artist);
    }

    for query in queries {
        let query_len = query.chars().count();
        let score = partial_ratio(query, &candidate.simplified_compact, &options);
        if query_len >= 8 && score >= 0.90 {
            return Some(100);
        }
        if query_len >= 4 && score >= 0.98 {
            return Some(90);
        }
    }

    let common_len =
        longest_common_substring_len(&candidate.simplified_compact, &title.simplified_compact);
    let shorter_len = candidate
        .simplified_compact
        .chars()
        .count()
        .min(title.simplified_compact.chars().count());
    if common_len >= 8 && common_len * 100 >= shorter_len * 50 {
        return Some(95);
    }

    let (shared_tokens, shared_chars) =
        shared_token_signal(&candidate.normalized, &title.normalized);
    if shared_tokens >= 2
        && shared_chars >= 10
        && partial_token_set_ratio(&candidate.normalized, &title.normalized, &options) >= 0.90
    {
        return Some(90);
    }
    None
}

fn remove_component(value: &str, component: &MatchKey) -> String {
    component
        .variants()
        .filter(|component| component.chars().count() >= 3)
        .fold(value.to_string(), |value, component| {
            value.replace(component, "")
        })
}

fn longest_common_substring_len(left: &str, right: &str) -> usize {
    let left: Vec<char> = left.chars().collect();
    let right: Vec<char> = right.chars().collect();
    let mut previous = vec![0usize; right.len() + 1];
    let mut longest = 0;
    for left_char in left {
        let mut current = vec![0usize; right.len() + 1];
        for (index, right_char) in right.iter().enumerate() {
            if left_char == *right_char {
                current[index + 1] = previous[index] + 1;
                longest = longest.max(current[index + 1]);
            }
        }
        previous = current;
    }
    longest
}

fn shared_token_signal(left: &str, right: &str) -> (usize, usize) {
    let left_tokens: Vec<&str> = left.split_whitespace().collect();
    let mut shared_tokens = 0;
    let mut shared_chars = 0;
    for token in right.split_whitespace() {
        if token.chars().count() >= 3 && left_tokens.contains(&token) {
            shared_tokens += 1;
            shared_chars += token.chars().count();
        }
    }
    (shared_tokens, shared_chars)
}

fn score_lrc_metadata(content: &str, title: &MatchKey, artist: &MatchKey) -> u16 {
    let mut metadata_title = None;
    let mut metadata_artist = None;
    for line in content.lines().take(80) {
        let line = line.trim().trim_start_matches('\u{feff}');
        if let Some(value) = lrc_metadata_value(line, "ti") {
            metadata_title = Some(MatchKey::new(value));
        } else if let Some(value) = lrc_metadata_value(line, "ar") {
            metadata_artist = Some(MatchKey::new(value));
        }
    }
    if !metadata_title
        .as_ref()
        .is_some_and(|metadata| metadata.matches(title))
    {
        return 0;
    }
    if metadata_artist
        .as_ref()
        .is_some_and(|metadata| metadata.matches(artist))
    {
        140
    } else {
        115
    }
}

fn lrc_metadata_value<'a>(line: &'a str, tag: &str) -> Option<&'a str> {
    let prefix = format!("[{tag}:");
    line.get(..prefix.len())?
        .eq_ignore_ascii_case(&prefix)
        .then(|| line.get(prefix.len()..)?.strip_suffix(']'))
        .flatten()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn normalize_match_text(value: &str, simplify: bool) -> String {
    let mut output = String::new();
    let mut bracket_depth = 0u32;
    let mut previous_space = true;
    for character in value.trim().to_lowercase().chars() {
        if simplify && matches!(character, '(' | '[' | '{' | '（' | '【') {
            bracket_depth += 1;
            continue;
        }
        if simplify && matches!(character, ')' | ']' | '}' | '）' | '】') {
            bracket_depth = bracket_depth.saturating_sub(1);
            continue;
        }
        if bracket_depth > 0 {
            continue;
        }
        if character.is_alphanumeric() {
            output.push(character);
            previous_space = false;
        } else if !previous_space {
            output.push(' ');
            previous_space = true;
        }
    }
    let normalized = output.trim().to_string();
    if !simplify {
        return normalized;
    }
    let words: Vec<&str> = normalized.split_whitespace().collect();
    let end = words
        .iter()
        .position(|word| matches!(*word, "feat" | "featuring" | "ft"))
        .unwrap_or(words.len());
    words[..end].join(" ")
}

fn compact(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

fn read_lrc_text_prefix(path: &Path) -> Option<String> {
    let mut file = fs::File::open(path).ok()?;
    let mut bytes = Vec::with_capacity(16 * 1024);
    file.by_ref().take(16 * 1024).read_to_end(&mut bytes).ok()?;
    Some(decode_lrc_text(&bytes))
}

fn read_lrc_text(path: &Path) -> Option<String> {
    fs::read(path).ok().map(|bytes| decode_lrc_text(&bytes))
}

fn decode_lrc_text(bytes: &[u8]) -> String {
    if let Ok(text) = std::str::from_utf8(bytes) {
        return text.trim_start_matches('\u{feff}').to_string();
    }
    if let Some(bytes) = bytes.strip_prefix(&[0xff, 0xfe]) {
        let utf16 = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect::<Vec<_>>();
        return String::from_utf16_lossy(&utf16);
    }
    if let Some(bytes) = bytes.strip_prefix(&[0xfe, 0xff]) {
        let utf16 = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
            .collect::<Vec<_>>();
        return String::from_utf16_lossy(&utf16);
    }
    let (text, _, _) = GBK.decode(bytes);
    text.into_owned()
}

pub async fn fetch_lyrics(
    title: &str,
    artist: &str,
    duration_secs: u64,
    mode: LyricsMode,
    source: &str,
    local_dir: Option<&str>,
) -> Option<Arc<Vec<LyricLine>>> {
    if title.is_empty() {
        return None;
    }

    if mode == LyricsMode::Lrc {
        let dir = local_dir.filter(|dir| !dir.trim().is_empty())?;
        let title = title.to_string();
        let artist = artist.to_string();
        let dir = dir.to_string();
        return tokio::task::spawn_blocking(move || fetch_lyrics_local(&title, &artist, &dir))
            .await
            .ok()
            .flatten();
    }

    log::info!("Lyrics: fetching '{title}' - '{artist}' from source '{source}'");
    if let Some(lyrics) = providers::fetch(source, title, artist, duration_secs).await {
        let word_synced = lyrics.iter().filter(|line| line.is_word_synced()).count();
        log::info!(
            "Lyrics: source '{source}' succeeded ({} lines, {word_synced} word-synced)",
            lyrics.len()
        );
        log::logger().flush();
        return Some(lyrics);
    }
    log::warn!(
        "Lyrics: source '{source}' returned no lyrics for '{title}' - '{artist}', starting fallback"
    );
    for fallback_source in providers::fallback_sources(source) {
        log::info!("Lyrics: trying fallback source '{fallback_source}'");
        if let Some(lyrics) = providers::fetch(fallback_source, title, artist, duration_secs).await
        {
            let word_synced = lyrics.iter().filter(|line| line.is_word_synced()).count();
            log::warn!(
                "Lyrics: fallback source '{fallback_source}' succeeded ({} lines, {word_synced} word-synced)",
                lyrics.len()
            );
            log::logger().flush();
            return Some(lyrics);
        }
    }
    None
}

fn parse_lyrics(lrc: &str, tlrc: &str) -> Vec<LyricLine> {
    let mut map: BTreeMap<u64, LyricLine> = BTreeMap::new();

    let mut process_content = |content: &str, keep_empty: bool| {
        let mut standard_lrc = String::with_capacity(content.len());
        for source_line in content.lines() {
            if let Some(line) = parse_word_synced_line(source_line) {
                merge_lyric_line(&mut map, line);
            } else {
                standard_lrc.push_str(source_line);
                standard_lrc.push('\n');
            }
        }

        let normalized = normalize_lrc_timestamps(&standard_lrc);
        let Ok(lyrics) = Lyrics::from_str(&normalized) else {
            return;
        };
        for (timestamp, text) in lyrics.get_timed_lines() {
            let text = text.trim();
            if text.is_empty() && !keep_empty {
                continue;
            }
            let timestamp = timestamp.get_timestamp();
            if timestamp < 0 {
                continue;
            }
            merge_lyric_line(
                &mut map,
                LyricLine {
                    time_ms: timestamp as u64,
                    text: text.to_string(),
                    secondary_text: None,
                    timings: Vec::new(),
                },
            );
        }
    };

    process_content(lrc, true);
    process_content(tlrc, false);

    map.into_values().collect()
}

fn merge_lyric_line(map: &mut BTreeMap<u64, LyricLine>, line: LyricLine) {
    let Some(current) = map.get_mut(&line.time_ms) else {
        map.insert(line.time_ms, line);
        return;
    };
    if line.text.is_empty() {
        return;
    }
    if current.text.is_empty() {
        current.text = line.text;
        current.timings = line.timings;
    } else if current.text != line.text && current.secondary_text.is_none() {
        current.secondary_text = Some(line.text);
    }
}

fn parse_word_synced_line(line: &str) -> Option<LyricLine> {
    let line = line.trim_start_matches('\u{feff}');
    let mut tags = Vec::new();
    let mut search_from = 0;
    while let Some(relative_start) = line.get(search_from..)?.find('[') {
        let start = search_from + relative_start;
        let Some(relative_end) = line.get(start + 1..)?.find(']') else {
            break;
        };
        let end = start + relative_end + 2;
        if let Some(time_ms) = parse_lrc_timestamp(line.get(start + 1..end - 1)?) {
            tags.push((start, end, time_ms));
        }
        search_from = end;
    }

    let mut text = String::new();
    let mut timings = Vec::new();
    for (index, &(_, segment_start, start_time_ms)) in tags.iter().enumerate() {
        let segment_end = tags
            .get(index + 1)
            .map_or(line.len(), |(start, _, _)| *start);
        let segment = line.get(segment_start..segment_end)?;
        if segment.is_empty() {
            continue;
        }
        if timings
            .last()
            .is_some_and(|previous: &LyricTiming| previous.start_time_ms > start_time_ms)
        {
            return None;
        }
        text.push_str(segment);
        timings.push(LyricTiming {
            start_time_ms,
            end_time_ms: None,
            end_byte: text.len(),
        });
    }

    if timings.len() < 2 || text.is_empty() {
        return None;
    }
    Some(LyricLine {
        time_ms: timings.first()?.start_time_ms,
        text,
        secondary_text: None,
        timings,
    })
}

fn parse_lrc_timestamp(timestamp: &str) -> Option<u64> {
    let (minutes, second_part) = timestamp.split_once(':')?;
    let (seconds, fraction) = second_part.split_once('.').unwrap_or((second_part, ""));
    if minutes.is_empty()
        || seconds.is_empty()
        || seconds.len() > 2
        || fraction.len() > 3
        || !minutes.bytes().all(|byte| byte.is_ascii_digit())
        || !seconds.bytes().all(|byte| byte.is_ascii_digit())
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let minutes = minutes.parse::<u64>().ok()?;
    let seconds = seconds.parse::<u64>().ok()?;
    if seconds >= 60 {
        return None;
    }
    let fraction = match fraction.len() {
        0 => 0,
        1 => fraction.parse::<u64>().ok()? * 100,
        2 => fraction.parse::<u64>().ok()? * 10,
        3 => fraction.parse::<u64>().ok()?,
        _ => return None,
    };
    minutes
        .checked_mul(60_000)?
        .checked_add(seconds * 1000)?
        .checked_add(fraction)
}

fn normalize_lrc_timestamps(content: &str) -> String {
    let bytes = content.as_bytes();
    let mut output = String::with_capacity(content.len());
    let mut copied_until = 0;
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] != b'[' && bytes[index] != b'<' {
            index += 1;
            continue;
        }
        let tag_start = index;
        index += 1;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        if index == tag_start + 1 || bytes.get(index) != Some(&b':') {
            continue;
        }
        index += 1;
        let seconds_start = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        if index == seconds_start || bytes.get(index) != Some(&b'.') {
            continue;
        }
        index += 1;
        let fraction_start = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        if index - fraction_start <= 2 || !matches!(bytes.get(index), Some(b']') | Some(b'>')) {
            continue;
        }

        output.push_str(&content[copied_until..fraction_start]);
        output.push(char::from(bytes[fraction_start]));
        output.push(char::from(bytes[fraction_start + 1]));
        copied_until = index;
    }

    if copied_until == 0 {
        content.to_string()
    } else {
        output.push_str(&content[copied_until..]);
        output
    }
}
