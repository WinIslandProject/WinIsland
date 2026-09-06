use std::collections::BTreeMap;
use std::sync::Arc;

use super::{MOZILLA_UA, get_json, query_matches_song, url_encode};
use crate::core::lyrics::{LyricLine, LyricTiming, parse_lyrics};

pub(super) async fn fetch(title: &str, artist: &str) -> Option<Arc<Vec<LyricLine>>> {
    if let Some(lyrics) = fetch_inner(title, artist).await {
        return Some(lyrics);
    }
    if artist.is_empty() {
        None
    } else {
        fetch_inner(title, "").await
    }
}

async fn fetch_inner(title: &str, artist: &str) -> Option<Arc<Vec<LyricLine>>> {
    let query = if artist.is_empty() {
        title.to_string()
    } else {
        format!("{title} {artist}")
    };
    let url = format!(
        "https://music.163.com/api/search/get/web?s={}&type=1&offset=0&total=true&limit=10",
        url_encode(&query)
    );

    let json = get_json(
        &url,
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36",
    )
    .await?;

    let songs = json.get("result")?.get("songs")?.as_array()?;
    if songs.is_empty() {
        return None;
    }

    let artist_lower = artist.to_lowercase();
    let mut song_id = None;

    if !artist_lower.is_empty() {
        for song in songs {
            if let Some(artists) = song.get("artists").and_then(|artists| artists.as_array()) {
                for candidate in artists {
                    if let Some(name) = candidate.get("name").and_then(|name| name.as_str())
                        && name.to_lowercase() == artist_lower
                    {
                        song_id = song.get("id").and_then(serde_json::Value::as_i64);
                        break;
                    }
                }
            }
            if song_id.is_some() {
                break;
            }
        }
    }

    if song_id.is_none() {
        let first = songs.first()?;
        if let Some(name) = first.get("name").and_then(|name| name.as_str())
            && !query_matches_song(&query, name)
        {
            return None;
        }
        song_id = first.get("id")?.as_i64();
    }

    let lyric_url = format!(
        "https://music.163.com/api/song/lyric?id={}&lv=1&kv=1&tv=-1&yv=1&ytv=1",
        song_id?
    );
    let lyric_json = get_json(&lyric_url, MOZILLA_UA).await?;
    let translated_lrc = lyric_json
        .get("ytlrc")
        .or_else(|| lyric_json.get("tlyric"))
        .and_then(|lyrics| lyrics.get("lyric"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    if let Some(yrc) = lyric_json
        .get("yrc")
        .and_then(|lyrics| lyrics.get("lyric"))
        .and_then(serde_json::Value::as_str)
    {
        let lines = parse_yrc(yrc, translated_lrc);
        if !lines.is_empty() {
            return Some(Arc::new(lines));
        }
    }

    let lrc = lyric_json.get("lrc")?.get("lyric")?.as_str().unwrap_or("");
    let lines = parse_lyrics(lrc, translated_lrc);
    (!lines.is_empty()).then(|| Arc::new(lines))
}

fn parse_yrc(yrc: &str, translated_lrc: &str) -> Vec<LyricLine> {
    let translations = parse_lyrics(translated_lrc, "")
        .into_iter()
        .map(|line| (line.time_ms, line.text))
        .collect::<BTreeMap<_, _>>();
    yrc.lines()
        .filter_map(parse_yrc_line)
        .map(|mut line| {
            line.secondary_text = translations.get(&line.time_ms).cloned();
            line
        })
        .collect()
}

fn parse_yrc_line(line: &str) -> Option<LyricLine> {
    let line = line.trim_start_matches('\u{feff}').strip_prefix('[')?;
    let header_end = line.find(']')?;
    let time_ms = parse_pair(line.get(..header_end)?)?.0;
    let content = line.get(header_end + 1..)?;

    let mut tags = Vec::new();
    let mut search_from = 0;
    while let Some(relative_start) = content.get(search_from..)?.find('(') {
        let start = search_from + relative_start;
        let Some(relative_end) = content.get(start + 1..)?.find(')') else {
            break;
        };
        let end = start + relative_end + 2;
        if let Some((word_start_ms, word_duration_ms)) =
            parse_pair(content.get(start + 1..end - 1)?)
        {
            tags.push((start, end, word_start_ms, word_duration_ms));
        }
        search_from = end;
    }

    let mut text = String::new();
    let mut timings = Vec::new();
    for (index, &(_, segment_start, start_time_ms, duration_ms)) in tags.iter().enumerate() {
        let segment_end = tags
            .get(index + 1)
            .map_or(content.len(), |(start, _, _, _)| *start);
        let segment = content.get(segment_start..segment_end)?;
        if segment.is_empty() {
            continue;
        }
        text.push_str(segment);
        timings.push(LyricTiming {
            start_time_ms,
            end_time_ms: start_time_ms.checked_add(duration_ms),
            end_byte: text.len(),
        });
    }

    if text.is_empty() || timings.is_empty() {
        return None;
    }
    Some(LyricLine {
        time_ms,
        text,
        secondary_text: None,
        timings,
    })
}

fn parse_pair(value: &str) -> Option<(u64, u64)> {
    let mut fields = value.split(',');
    let start = fields.next()?.parse().ok()?;
    let duration = fields.next()?.parse().ok()?;
    Some((start, duration))
}
