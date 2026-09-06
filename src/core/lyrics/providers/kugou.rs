use std::sync::Arc;

use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::Value;

use super::{MOZILLA_UA, artist_matches, get_json, query_matches_song, url_encode};
use crate::core::lyrics::{LyricLine, parse_lyrics};

pub(super) async fn fetch(
    title: &str,
    artist: &str,
    duration_secs: u64,
) -> Option<Arc<Vec<LyricLine>>> {
    if let Some(lyrics) = fetch_by_keyword(title, artist, duration_secs).await {
        return Some(lyrics);
    }
    fetch_by_song_hash(title, artist).await
}

async fn fetch_by_keyword(
    title: &str,
    artist: &str,
    duration_secs: u64,
) -> Option<Arc<Vec<LyricLine>>> {
    let search_url = format!(
        "https://lyrics.kugou.com/search?ver=1&man=yes&client=pc&keyword={}&duration={}",
        url_encode(title),
        duration_secs.saturating_mul(1000)
    );
    let search_json = get_json(&search_url, MOZILLA_UA).await?;
    let candidates = search_json.get("candidates")?.as_array()?;
    let candidate = select_lyric_candidate(candidates, title, artist)?;
    download(candidate).await
}

async fn fetch_by_song_hash(title: &str, artist: &str) -> Option<Arc<Vec<LyricLine>>> {
    let song_search_url = format!(
        "https://songsearch.kugou.com/song_search_v2?keyword={}&page=1&pagesize=20&platform=WebFilter&filter=2&iscorrection=1&privilege_filter=0",
        url_encode(title)
    );
    let song_search_json = get_json(&song_search_url, MOZILLA_UA).await?;
    let songs = song_search_json.get("data")?.get("lists")?.as_array()?;
    let song = select_song(songs, title, artist)?;
    let hash = song.get("FileHash")?.as_str()?;

    let lyrics_search_url =
        format!("https://lyrics.kugou.com/search?ver=1&man=yes&client=pc&hash={hash}");
    let lyrics_search_json = get_json(&lyrics_search_url, MOZILLA_UA).await?;
    let candidates = lyrics_search_json.get("candidates")?.as_array()?;
    let candidate = select_lyric_candidate(candidates, title, artist)?;
    download(candidate).await
}

fn select_lyric_candidate<'a>(
    candidates: &'a [Value],
    title: &str,
    artist: &str,
) -> Option<&'a Value> {
    let matches_title = |candidate: &&Value| {
        candidate
            .get("song")
            .and_then(Value::as_str)
            .is_some_and(|song| query_matches_song(title, song))
    };
    candidates
        .iter()
        .filter(matches_title)
        .find(|candidate| {
            candidate
                .get("singer")
                .and_then(Value::as_str)
                .is_some_and(|singer| artist_matches(artist, singer))
        })
        .or_else(|| candidates.iter().find(matches_title))
}

fn select_song<'a>(songs: &'a [Value], title: &str, artist: &str) -> Option<&'a Value> {
    let matches_title = |song: &&Value| {
        song.get("SongName")
            .and_then(Value::as_str)
            .is_some_and(|song_name| query_matches_song(title, song_name))
    };
    songs
        .iter()
        .filter(matches_title)
        .find(|song| {
            song.get("SingerName")
                .and_then(Value::as_str)
                .is_some_and(|singer| artist_matches(artist, singer))
        })
        .or_else(|| songs.iter().find(matches_title))
}

async fn download(candidate: &Value) -> Option<Arc<Vec<LyricLine>>> {
    let id = candidate.get("id")?.as_str()?;
    let access_key = candidate.get("accesskey")?.as_str()?;
    let download_url = format!(
        "https://lyrics.kugou.com/download?ver=1&client=pc&id={id}&accesskey={access_key}&fmt=lrc&charset=utf8"
    );
    let download_json = get_json(&download_url, MOZILLA_UA).await?;
    let content = download_json.get("content")?.as_str()?;
    let decoded = STANDARD.decode(content).ok()?;
    let lrc = std::str::from_utf8(&decoded).ok()?;
    let lines = parse_lyrics(lrc, "");
    (!lines.is_empty()).then(|| Arc::new(lines))
}
