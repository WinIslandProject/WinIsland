use std::collections::HashMap;
use std::sync::Arc;

use quick_xml::Reader;
use quick_xml::XmlVersion;
use quick_xml::encoding::Decoder;
use quick_xml::events::{BytesStart, BytesText, Event};
use serde_json::Value;

use super::{artist_matches, get_json, query_matches_song, url_encode, winisland_ua};
use crate::core::lyrics::{LyricLine, LyricTiming, MatchKey};

const API_BASE: &str = "https://api.amll.dev/v1/lyrics";

pub(super) async fn fetch(title: &str, artist: &str) -> Option<Arc<Vec<LyricLine>>> {
    let search_url = format!(
        "{API_BASE}/search?musicName={}&pageSize=50",
        url_encode(title)
    );
    let Some(search_json) = get_json(&search_url, &winisland_ua()).await else {
        log::info!("AMLL: search request failed for '{title}' - '{artist}'");
        return None;
    };
    let Some(items) = search_json
        .get("data")
        .and_then(|data| data.get("items"))
        .and_then(Value::as_array)
    else {
        log::info!("AMLL: search returned an invalid response for '{title}' - '{artist}'");
        return None;
    };
    log::info!(
        "AMLL: search returned {} candidate(s) for '{title}' - '{artist}'",
        items.len()
    );
    let Some(candidate) = select_candidate(items, title, artist) else {
        log::info!("AMLL: no matching candidate for '{title}' - '{artist}'");
        return None;
    };
    let Some(id) = candidate.get("id").and_then(Value::as_u64) else {
        log::info!("AMLL: selected candidate has no valid ID");
        return None;
    };

    let lyric_url = format!("{API_BASE}/get?id={id}");
    let Some(lyric_json) = get_json(&lyric_url, &winisland_ua()).await else {
        log::info!("AMLL: lyric request failed for candidate {id}");
        return None;
    };
    let Some(ttml) = lyric_json
        .get("data")
        .and_then(|data| data.get("lyrics"))
        .and_then(Value::as_str)
    else {
        log::info!("AMLL: candidate {id} returned no TTML content");
        return None;
    };
    let Some(lines) = parse_ttml(ttml) else {
        log::info!("AMLL: failed to parse TTML for candidate {id}");
        return None;
    };
    log::info!(
        "AMLL: parsed {} word-synced lines for candidate {id}",
        lines.len()
    );
    (!lines.is_empty()).then(|| Arc::new(lines))
}

fn select_candidate<'a>(items: &'a [Value], title: &str, artist: &str) -> Option<&'a Value> {
    let title_key = MatchKey::new(title);
    let mut best = None;
    for item in items {
        let Some(names) = item.get("musicNames").and_then(Value::as_array) else {
            continue;
        };
        if !names.iter().any(|name| {
            name.as_str()
                .is_some_and(|name| query_matches_song(title, name))
        }) {
            continue;
        }
        let exact_title = names.iter().any(|name| {
            name.as_str()
                .is_some_and(|name| MatchKey::new(name).matches(&title_key))
        });
        let artist_match = artist.is_empty()
            || item
                .get("artistNames")
                .and_then(Value::as_array)
                .is_some_and(|names| {
                    names.iter().any(|name| {
                        name.as_str()
                            .is_some_and(|name| artist_matches(artist, name))
                    })
                });
        if !artist_match {
            continue;
        }
        let score = u8::from(exact_title);
        if best.is_none_or(|(best_score, _)| score > best_score) {
            best = Some((score, item));
        }
    }
    best.map(|(_, item)| item)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AuxiliaryKind {
    Translation,
    Romanization,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SpanKind {
    Main,
    Translation,
    Romanization,
    Skip,
}

struct SpanContext {
    kind: SpanKind,
    start_time_ms: Option<u64>,
    end_time_ms: Option<u64>,
    text: String,
}

struct LineBuilder {
    time_ms: u64,
    key: Option<String>,
    text: String,
    translation: Option<String>,
    romanization: Option<String>,
    timings: Vec<LyricTiming>,
}

impl LineBuilder {
    fn append_word(&mut self, span: SpanContext) {
        if span.text.is_empty() {
            return;
        }
        if span.text.chars().all(char::is_whitespace) {
            self.append_interstitial(&span.text);
            return;
        }
        let (Some(start_time_ms), Some(end_time_ms)) = (span.start_time_ms, span.end_time_ms)
        else {
            return;
        };
        if end_time_ms <= start_time_ms
            || self
                .timings
                .last()
                .is_some_and(|timing| timing.start_time_ms > start_time_ms)
        {
            return;
        }
        self.text.push_str(&span.text);
        self.timings.push(LyricTiming {
            start_time_ms,
            end_time_ms: Some(end_time_ms),
            end_byte: self.text.len(),
        });
    }

    fn append_interstitial(&mut self, text: &str) {
        if self.text.is_empty() || text.contains(['\r', '\n', '\t']) {
            return;
        }
        self.text.push_str(text);
        if let Some(timing) = self.timings.last_mut() {
            timing.end_byte = self.text.len();
        }
    }

    fn append_auxiliary(&mut self, kind: SpanKind, text: &str) {
        let target = match kind {
            SpanKind::Translation => &mut self.translation,
            SpanKind::Romanization => &mut self.romanization,
            SpanKind::Main | SpanKind::Skip => return,
        };
        if target.is_none() && !text.trim().is_empty() {
            *target = Some(text.to_string());
        }
    }

    fn finish(
        self,
        translations: &HashMap<String, String>,
        romanizations: &HashMap<String, String>,
    ) -> Option<LyricLine> {
        if self.text.is_empty() || self.timings.is_empty() {
            return None;
        }
        let inline_translation = self.translation.as_deref().and_then(normalize_auxiliary);
        let inline_romanization = self.romanization.as_deref().and_then(normalize_auxiliary);
        let secondary_text = inline_translation
            .or_else(|| {
                self.key
                    .as_ref()
                    .and_then(|key| translations.get(key).cloned())
            })
            .or(inline_romanization)
            .or_else(|| {
                self.key
                    .as_ref()
                    .and_then(|key| romanizations.get(key).cloned())
            });
        Some(LyricLine {
            time_ms: self.time_ms,
            text: self.text,
            secondary_text,
            timings: self.timings,
        })
    }
}

fn parse_ttml(ttml: &str) -> Option<Vec<LyricLine>> {
    let mut reader = Reader::from_str(ttml);
    reader.config_mut().trim_text(false);

    let mut translations = HashMap::new();
    let mut romanizations = HashMap::new();
    let mut header_kind = None;
    let mut header_text: Option<(String, String)> = None;
    let mut current_line = None;
    let mut spans: Vec<SpanContext> = Vec::new();
    let mut lines = Vec::new();

    loop {
        let decoder = reader.decoder();
        match reader.read_event() {
            Ok(Event::Start(start)) => match start.local_name().as_ref() {
                b"translation" => header_kind = Some(AuxiliaryKind::Translation),
                b"transliteration" => header_kind = Some(AuxiliaryKind::Romanization),
                b"text" if header_kind.is_some() => {
                    let key = attribute_value(&start, b"for", decoder)?;
                    header_text = Some((key, String::new()));
                }
                b"p" => {
                    let time_ms = parse_ttml_time(&attribute_value(&start, b"begin", decoder)?)?;
                    current_line = Some(LineBuilder {
                        time_ms,
                        key: attribute_value(&start, b"key", decoder),
                        text: String::new(),
                        translation: None,
                        romanization: None,
                        timings: Vec::new(),
                    });
                    spans.clear();
                }
                b"span" if current_line.is_some() => {
                    spans.push(start_span(
                        &start,
                        decoder,
                        spans.last().map(|span| span.kind),
                    ));
                }
                _ => {}
            },
            Ok(Event::End(end)) => match end.local_name().as_ref() {
                b"translation" | b"transliteration" => header_kind = None,
                b"text" if header_text.is_some() => {
                    let (key, text) = header_text.take()?;
                    if let Some(text) = normalize_auxiliary(&text) {
                        match header_kind {
                            Some(AuxiliaryKind::Translation) => {
                                translations.entry(key).or_insert(text);
                            }
                            Some(AuxiliaryKind::Romanization) => {
                                romanizations.entry(key).or_insert(text);
                            }
                            None => {}
                        }
                    }
                }
                b"span" if current_line.is_some() => {
                    let span = spans.pop()?;
                    if let Some(parent) = spans.last_mut() {
                        if parent.kind == span.kind && parent.kind != SpanKind::Skip {
                            parent.text.push_str(&span.text);
                        }
                    } else if let Some(line) = current_line.as_mut() {
                        match span.kind {
                            SpanKind::Main => line.append_word(span),
                            SpanKind::Translation | SpanKind::Romanization => {
                                line.append_auxiliary(span.kind, &span.text);
                            }
                            SpanKind::Skip => {}
                        }
                    }
                }
                b"p" => {
                    spans.clear();
                    if let Some(line) = current_line
                        .take()
                        .and_then(|line| line.finish(&translations, &romanizations))
                    {
                        lines.push(line);
                    }
                }
                _ => {}
            },
            Ok(Event::Text(text)) => {
                let text = decode_text(&text)?;
                if let Some((_, header_text)) = header_text.as_mut() {
                    header_text.push_str(&text);
                } else if let Some(span) = spans.last_mut() {
                    if span.kind != SpanKind::Skip {
                        span.text.push_str(&text);
                    }
                } else if let Some(line) = current_line.as_mut() {
                    line.append_interstitial(&text);
                }
            }
            Ok(Event::CData(text)) => {
                let text = text.decode().ok()?;
                if let Some((_, header_text)) = header_text.as_mut() {
                    header_text.push_str(&text);
                } else if let Some(span) = spans.last_mut()
                    && span.kind != SpanKind::Skip
                {
                    span.text.push_str(&text);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => return None,
            _ => {}
        }
    }

    lines.sort_by_key(|line| line.time_ms);
    Some(lines)
}

fn start_span(start: &BytesStart<'_>, decoder: Decoder, parent: Option<SpanKind>) -> SpanContext {
    let role = attribute_value(start, b"role", decoder);
    let kind = if parent == Some(SpanKind::Skip) || role.as_deref() == Some("x-bg") {
        SpanKind::Skip
    } else if matches!(parent, Some(SpanKind::Translation))
        || role.as_deref() == Some("x-translation")
    {
        SpanKind::Translation
    } else if matches!(parent, Some(SpanKind::Romanization)) || role.as_deref() == Some("x-roman") {
        SpanKind::Romanization
    } else {
        SpanKind::Main
    };
    SpanContext {
        kind,
        start_time_ms: attribute_value(start, b"begin", decoder)
            .as_deref()
            .and_then(parse_ttml_time),
        end_time_ms: attribute_value(start, b"end", decoder)
            .as_deref()
            .and_then(parse_ttml_time),
        text: String::new(),
    }
}

fn attribute_value(start: &BytesStart<'_>, name: &[u8], decoder: Decoder) -> Option<String> {
    start
        .attributes()
        .with_checks(false)
        .filter_map(Result::ok)
        .find(|attribute| attribute.key.local_name().as_ref() == name)?
        .decoded_and_normalized_value(XmlVersion::Implicit1_0, decoder)
        .ok()
        .map(|value| value.into_owned())
}

fn decode_text(text: &BytesText<'_>) -> Option<String> {
    let decoded = text.decode().ok()?;
    quick_xml::escape::unescape(&decoded)
        .ok()
        .map(|text| text.into_owned())
}

fn normalize_auxiliary(text: &str) -> Option<String> {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    (!normalized.is_empty()).then_some(normalized)
}

fn parse_ttml_time(value: &str) -> Option<u64> {
    let value = value.trim().strip_suffix('s').unwrap_or(value.trim());
    let parts = value.split(':').collect::<Vec<_>>();
    match parts.as_slice() {
        [seconds] => parse_seconds(seconds, false),
        [minutes, seconds] => minutes
            .parse::<u64>()
            .ok()?
            .checked_mul(60_000)?
            .checked_add(parse_seconds(seconds, true)?),
        [hours, minutes, seconds] => {
            let minutes = minutes.parse::<u64>().ok()?;
            if minutes >= 60 {
                return None;
            }
            hours
                .parse::<u64>()
                .ok()?
                .checked_mul(3_600_000)?
                .checked_add(minutes * 60_000)?
                .checked_add(parse_seconds(seconds, true)?)
        }
        _ => None,
    }
}

fn parse_seconds(value: &str, bounded: bool) -> Option<u64> {
    let (seconds, fraction) = value.split_once('.').unwrap_or((value, ""));
    if seconds.is_empty()
        || fraction.len() > 3
        || !seconds.bytes().all(|byte| byte.is_ascii_digit())
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let seconds = seconds.parse::<u64>().ok()?;
    if bounded && seconds >= 60 {
        return None;
    }
    let milliseconds = match fraction.len() {
        0 => 0,
        1 => fraction.parse::<u64>().ok()? * 100,
        2 => fraction.parse::<u64>().ok()? * 10,
        3 => fraction.parse::<u64>().ok()?,
        _ => return None,
    };
    seconds.checked_mul(1000)?.checked_add(milliseconds)
}
