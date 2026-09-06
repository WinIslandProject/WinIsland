use std::sync::Arc;

use serde_json::Value;

use super::{get_json, query_matches_song, url_encode, winisland_ua};
use crate::core::lyrics::{LyricLine, parse_lyrics};

pub(super) async fn fetch(
    title: &str,
    artist: &str,
    duration_secs: u64,
) -> Option<Arc<Vec<LyricLine>>> {
    if let Some(lyrics) = fetch_exact(title, artist, duration_secs).await {
        return Some(lyrics);
    }
    fetch_search(title, artist).await
}

async fn fetch_exact(title: &str, artist: &str, duration_secs: u64) -> Option<Arc<Vec<LyricLine>>> {
    let url = format!(
        "https://lrclib.net/api/get?track_name={}&artist_name={}&duration={}",
        url_encode(title),
        url_encode(artist),
        duration_secs
    );
    let json = get_json(&url, &winisland_ua()).await?;
    let synced = json.get("syncedLyrics")?.as_str()?;
    let lines = parse_lyrics(synced, "");
    (!lines.is_empty()).then(|| Arc::new(lines))
}

async fn fetch_search(title: &str, artist: &str) -> Option<Arc<Vec<LyricLine>>> {
    let query = if artist.is_empty() {
        title.to_string()
    } else {
        format!("{title} {artist}")
    };
    let url = format!("https://lrclib.net/api/search?q={}", url_encode(&query));
    let json = get_json(&url, &winisland_ua()).await?;

    for item in json.as_array()? {
        let Some(synced) = item.get("syncedLyrics").and_then(Value::as_str) else {
            continue;
        };
        if item
            .get("trackName")
            .and_then(Value::as_str)
            .is_some_and(|name| !query_matches_song(&query, name))
        {
            continue;
        }
        let lines = parse_lyrics(synced, "");
        if !lines.is_empty() {
            return Some(Arc::new(lines));
        }
    }
    None
}
