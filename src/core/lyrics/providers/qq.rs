use std::sync::Arc;

use serde_json::Value;

use super::{MOZILLA_UA, artist_matches, get_json_with_referer, query_matches_song, url_encode};
use crate::core::lyrics::{LyricLine, MatchKey, parse_lyrics};

pub(super) async fn fetch(
    title: &str,
    artist: &str,
    duration_secs: u64,
) -> Option<Arc<Vec<LyricLine>>> {
    if let Some(lyrics) = fetch_inner(title, artist, duration_secs).await {
        return Some(lyrics);
    }
    if artist.is_empty() {
        None
    } else {
        fetch_inner(title, "", duration_secs).await
    }
}

async fn fetch_inner(title: &str, artist: &str, duration_secs: u64) -> Option<Arc<Vec<LyricLine>>> {
    let query = if artist.is_empty() {
        title.to_string()
    } else {
        format!("{title} {artist}")
    };
    let search_url = format!(
        "https://c.y.qq.com/soso/fcgi-bin/client_search_cp?format=json&p=1&n=20&w={}",
        url_encode(&query)
    );
    let search_json = get_json_with_referer(&search_url, MOZILLA_UA, "https://y.qq.com/").await?;
    let songs = search_json
        .get("data")?
        .get("song")?
        .get("list")?
        .as_array()?;
    let song = select_song(songs, title, artist, duration_secs)?;
    let song_mid = song.get("songmid")?.as_str()?;

    let lyric_url = format!(
        "https://c.y.qq.com/lyric/fcgi-bin/fcg_query_lyric_new.fcg?songmid={song_mid}&format=json&nobase64=1&g_tk=5381"
    );
    let lyric_json = get_json_with_referer(&lyric_url, MOZILLA_UA, "https://y.qq.com/").await?;
    if lyric_json.get("retcode").and_then(Value::as_i64) != Some(0) {
        return None;
    }
    let lrc = lyric_json.get("lyric")?.as_str()?;
    let translated_lrc = lyric_json
        .get("trans")
        .and_then(Value::as_str)
        .unwrap_or("");
    let lines = parse_lyrics(lrc, translated_lrc);
    (!lines.is_empty()).then(|| Arc::new(lines))
}

fn select_song<'a>(
    songs: &'a [Value],
    title: &str,
    artist: &str,
    duration_secs: u64,
) -> Option<&'a Value> {
    let title_key = MatchKey::new(title);
    let mut best = None;
    let mut best_score = 0;
    for song in songs {
        let Some(song_name) = song.get("songname").and_then(Value::as_str) else {
            continue;
        };
        if !query_matches_song(title, song_name) {
            continue;
        }
        let exact_title = MatchKey::new(song_name).matches(&title_key);
        let artist_match = song
            .get("singer")
            .and_then(Value::as_array)
            .is_some_and(|singers| {
                singers.iter().any(|singer| {
                    singer
                        .get("name")
                        .and_then(Value::as_str)
                        .is_some_and(|singer| artist_matches(artist, singer))
                })
            });
        let duration_match = duration_secs > 0
            && song
                .get("interval")
                .and_then(Value::as_u64)
                .is_some_and(|duration| duration.abs_diff(duration_secs) <= 5);
        let score =
            u8::from(exact_title) * 4 + u8::from(artist_match) * 2 + u8::from(duration_match);
        if best.is_none() || score > best_score {
            best = Some(song);
            best_score = score;
        }
    }
    best
}
