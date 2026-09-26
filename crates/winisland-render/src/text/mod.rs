//! Cached font measurement and drawing for the rendering layer.

use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::OnceLock;

use skia_safe::{Font, FontMgr, FontStyle as SkFontStyle, Paint, Path, Typeface, image_filters};

use crate::painter::Painter;
use crate::types::{BlurSpec, FontStyle, Rgba, Slant};

static GLOBAL_FONT_MANAGER: OnceLock<FontManager> = OnceLock::new();

type TextGroup = (String, Typeface, bool, f32);
type TextGroups = Vec<TextGroup>;
type TextCacheValue = (f32, TextGroups);
type TextCacheMap = HashMap<u64, TextCacheValue>;
type TextPathCacheMap = HashMap<u64, Vec<Path>>;
type TextPrefixCacheMap = HashMap<u64, Vec<(usize, f32)>>;

fn text_paint(color: Rgba, blur: Option<BlurSpec>) -> Paint {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(crate::convert::to_skia_color(color));
    if let Some(BlurSpec { sigma, tile }) = blur
        && let Some(filter) = image_filters::blur(
            sigma,
            tile.map(crate::convert::to_skia_tile_mode),
            None,
            None,
        )
    {
        paint.set_image_filter(filter);
    }
    paint
}

pub struct DrawTextInRectParams<'a> {
    pub painter: Painter<'a>,
    pub text: &'a str,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub size: f32,
    pub bold: bool,
    pub color: Rgba,
    pub blur: Option<BlurSpec>,
}

pub struct DrawTextCachedParams<'a> {
    pub painter: Painter<'a>,
    pub text: &'a str,
    pub x: f32,
    pub y: f32,
    pub size: f32,
    pub bold: bool,
    pub color: Rgba,
    pub blur: Option<BlurSpec>,
}

pub struct FontManager;

struct CustomTypefaceState {
    path: Option<String>,
    typeface: Option<Typeface>,
    load_attempted: bool,
}

thread_local! {
    static FONT_MGR: FontMgr = FontMgr::new();
    static FALLBACK_CACHE: RefCell<HashMap<(char, u32), Typeface>> = RefCell::new(HashMap::new());
    static TEXT_CACHE: RefCell<TextCacheMap> = RefCell::new(HashMap::new());
    static TEXT_PATH_CACHE: RefCell<TextPathCacheMap> = RefCell::new(HashMap::new());
    static TEXT_PREFIX_CACHE: RefCell<TextPrefixCacheMap> = RefCell::new(HashMap::new());
    static CUSTOM_TYPEFACE: RefCell<CustomTypefaceState> = const { RefCell::new(CustomTypefaceState {
        path: None,
        typeface: None,
        load_attempted: false,
    }) };
}

const FALLBACK_CACHE_LIMIT: usize = 2000;
const TEXT_CACHE_LIMIT: usize = 500;
const TEXT_PATH_CACHE_LIMIT: usize = 100;
const TEXT_PREFIX_CACHE_LIMIT: usize = 100;
const TEXT_PREFIX_WIDTH_LIMIT: usize = 256;

fn to_skia_font_style(style: FontStyle) -> SkFontStyle {
    let slant = match style.slant() {
        Slant::Upright => skia_safe::font_style::Slant::Upright,
        Slant::Italic => skia_safe::font_style::Slant::Italic,
        Slant::Oblique => skia_safe::font_style::Slant::Oblique,
    };
    SkFontStyle::new(
        i32::from(style.weight().value()).into(),
        i32::from(style.width().value()).into(),
        slant,
    )
}

fn evict_one_if_full<K, V>(cache: &mut HashMap<K, V>, limit: usize)
where
    K: Clone + std::cmp::Eq + std::hash::Hash,
{
    if cache.len() >= limit
        && let Some(key) = cache.keys().next().cloned()
    {
        cache.remove(&key);
    }
}

fn hash_cache_key(text: &str, style: FontStyle, size: f32) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    style.cache_key().hash(&mut hasher);
    ((size * 100.0).round() as i32).hash(&mut hasher);
    hasher.finish()
}

fn needs_synthetic_bold(tf: &Typeface, style: FontStyle) -> bool {
    style.weight().value() >= 600 && *tf.font_style().weight() < 600
}

fn make_font(tf: Typeface, size: f32, style: FontStyle) -> Font {
    let embolden = needs_synthetic_bold(&tf, style);
    let mut font = Font::from_typeface(tf, size);
    font.set_subpixel(true);
    if embolden {
        font.set_embolden(true);
    }
    font
}

fn get_custom_typeface() -> Option<Typeface> {
    CUSTOM_TYPEFACE.with(|cache| {
        let mut state = cache.borrow_mut();
        if state.load_attempted {
            return state.typeface.clone();
        }
        state.load_attempted = true;
        let path = state.path.clone()?;
        let data = std::fs::read(path).ok()?;
        let typeface = FONT_MGR.with(|mgr| mgr.new_from_data(&data, None));
        state.typeface = typeface.clone();
        typeface
    })
}

fn measure_group(text: &str, typeface: &Typeface, embolden: bool, size: f32) -> f32 {
    let mut font = Font::from_typeface(typeface.clone(), size);
    font.set_subpixel(true);
    if embolden {
        font.set_embolden(true);
    }
    font.measure_str(text, None).0
}

fn typeface_supports_char(typeface: &Typeface, character: char) -> bool {
    let mut glyphs = [0u16; 1];
    typeface.unichars_to_glyphs(&[character as i32], &mut glyphs);
    glyphs[0] != 0
}

fn get_typeface_for_char(c: char, style: FontStyle) -> (Typeface, bool) {
    let sk_style = to_skia_font_style(style);
    let s_key = style.cache_key();
    FALLBACK_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(tf) = cache.get(&(c, s_key)) {
            let embolden = needs_synthetic_bold(tf, style);
            return (tf.clone(), embolden);
        }
        evict_one_if_full(&mut cache, FALLBACK_CACHE_LIMIT);

        if let Some(tf) = get_custom_typeface() {
            let mut glyphs = [0u16; 1];
            tf.unichars_to_glyphs(&[c as i32], &mut glyphs);
            if glyphs[0] != 0 {
                let embolden = needs_synthetic_bold(&tf, style);
                cache.insert((c, s_key), tf.clone());
                return (tf, embolden);
            }
        }

        let tf = FONT_MGR.with(|mgr| {
            mgr.match_family_style_character("", sk_style, &["zh-CN", "ja-JP", "en-US"], c as i32)
                .filter(|tf| typeface_supports_char(tf, c))
                .or_else(|| {
                    [
                        "Segoe UI Emoji",
                        "Microsoft YaHei",
                        "Segoe UI Symbol",
                        "Segoe UI",
                    ]
                    .into_iter()
                    .find_map(|family| {
                        mgr.match_family_style(family, sk_style)
                            .filter(|tf| typeface_supports_char(tf, c))
                    })
                })
                .unwrap_or_else(|| mgr.legacy_make_typeface(None, sk_style).unwrap())
        });
        let embolden = needs_synthetic_bold(&tf, style);
        cache.insert((c, s_key), tf.clone());
        (tf, embolden)
    })
}

fn inherits_previous_typeface(c: char) -> bool {
    matches!(c, '\u{200C}' | '\u{200D}' | '\u{FE00}'..='\u{FE0F}')
        || ('\u{E0100}'..='\u{E01EF}').contains(&c)
        || ('\u{0300}'..='\u{036F}').contains(&c)
}

fn is_ascii_text(text: &str) -> bool {
    text.bytes().all(|b| b.is_ascii())
}

/// Compute text groups and total width.
/// Falls back to a single typeface for ASCII-only text to skip per-char lookups.
fn compute_text_groups(text: &str, size: f32, style: FontStyle) -> (f32, TextGroups) {
    let sk_style = to_skia_font_style(style);
    let mut current_w = 0.0;
    let mut groups: TextGroups = Vec::new();

    if is_ascii_text(text) {
        let tf = get_custom_typeface().unwrap_or_else(|| {
            FONT_MGR.with(|mgr| {
                mgr.match_family_style("Microsoft YaHei", sk_style)
                    .or_else(|| mgr.match_family_style("Segoe UI", sk_style))
                    .unwrap_or_else(|| mgr.legacy_make_typeface(None, sk_style).unwrap())
            })
        });
        let embolden = needs_synthetic_bold(&tf, style);
        let mut font = Font::from_typeface(tf.clone(), size);
        font.set_subpixel(true);
        if embolden {
            font.set_embolden(true);
        }
        let (w, _) = font.measure_str(text, None);
        current_w += w;
        groups.push((text.to_string(), tf, embolden, w));
        return (current_w, groups);
    }

    let mut current_group = String::new();
    let mut last_tf: Option<Typeface> = None;
    let mut last_embolden = false;
    for c in text.chars() {
        let (tf, embolden) = if inherits_previous_typeface(c) {
            last_tf
                .as_ref()
                .map(|tf| (tf.clone(), last_embolden))
                .unwrap_or_else(|| get_typeface_for_char(c, style))
        } else {
            get_typeface_for_char(c, style)
        };
        if let Some(ref ltf) = last_tf
            && (ltf.unique_id() != tf.unique_id() || last_embolden != embolden)
        {
            let group_text = std::mem::take(&mut current_group);
            let width = measure_group(&group_text, ltf, last_embolden, size);
            current_w += width;
            groups.push((group_text, ltf.clone(), last_embolden, width));
        }
        last_tf = Some(tf);
        last_embolden = embolden;
        current_group.push(c);
    }
    if let Some(ltf) = last_tf {
        let width = measure_group(&current_group, &ltf, last_embolden, size);
        current_w += width;
        groups.push((current_group, ltf, last_embolden, width));
    }

    (current_w, groups)
}

fn compute_text_paths(text: &str, size: f32, style: FontStyle) -> Vec<Path> {
    let (_, groups) = compute_text_groups(text, size, style);
    let mut x = 0.0;
    groups
        .into_iter()
        .map(|(text, typeface, embolden, width)| {
            let mut font = Font::from_typeface(typeface, size);
            font.set_subpixel(true);
            if embolden {
                font.set_embolden(true);
            }
            let path = skia_safe::utils::text_utils::get_path(text.as_str(), (x, 0.0), &font);
            x += width;
            path
        })
        .collect()
}

impl FontManager {
    pub fn global() -> &'static FontManager {
        GLOBAL_FONT_MANAGER.get_or_init(|| FontManager)
    }

    pub fn set_custom_font_path(&self, path: Option<&str>) {
        let path = path.map(str::to_owned);
        CUSTOM_TYPEFACE.with(|cache| {
            let mut state = cache.borrow_mut();
            state.path = path;
            state.typeface = None;
            state.load_attempted = false;
        });
        self.clear_text_caches();
        FALLBACK_CACHE.with(|cache| cache.borrow_mut().clear());
    }

    pub fn clear_text_caches(&self) {
        TEXT_CACHE.with(|cache| cache.borrow_mut().clear());
        TEXT_PATH_CACHE.with(|cache| cache.borrow_mut().clear());
        TEXT_PREFIX_CACHE.with(|cache| cache.borrow_mut().clear());
    }

    fn get_font(&self, size: f32, bold: bool) -> Font {
        let style = if bold {
            FontStyle::bold()
        } else {
            FontStyle::normal()
        };
        if let Some(tf) = get_custom_typeface() {
            return make_font(tf, size, style);
        }
        let sk_style = to_skia_font_style(style);
        let typeface = FONT_MGR.with(|mgr| {
            mgr.match_family_style("Microsoft YaHei", sk_style)
                .or_else(|| mgr.match_family_style("Segoe UI", sk_style))
                .unwrap_or_else(|| mgr.legacy_make_typeface(None, sk_style).unwrap())
        });
        make_font(typeface, size, style)
    }

    pub fn measure_str(&self, text: &str, size: f32, bold: bool) -> crate::types::Rect {
        let font = self.get_font(size, bold);
        let (_, bounds) = font.measure_str(text, None);
        crate::types::Rect::from_ltrb(bounds.left, bounds.top, bounds.right, bounds.bottom)
    }

    pub fn ascent(&self, size: f32, bold: bool) -> f32 {
        let font = self.get_font(size, bold);
        let (_, metrics) = font.metrics();
        metrics.ascent
    }

    pub fn draw_str(
        &self,
        painter: Painter<'_>,
        text: &str,
        at: crate::types::Point,
        size: f32,
        bold: bool,
        color: Rgba,
    ) {
        let font = self.get_font(size, bold);
        let paint = text_paint(color, None);
        painter.canvas().draw_str(text, (at.x, at.y), &font, &paint);
    }

    pub fn draw_plugin_str_v1(
        &self,
        canvas: &skia_safe::Canvas,
        text: &str,
        at: crate::types::Point,
        size: f32,
        bold: bool,
        color: Rgba,
    ) {
        self.draw_str(Painter { canvas }, text, at, size, bold, color);
    }

    pub fn draw_text_in_rect(&self, params: DrawTextInRectParams<'_>) {
        let font = self.get_font(params.size, params.bold);
        let paint = text_paint(params.color, params.blur);
        let canvas = params.painter.canvas();
        let (_, rect) = font.measure_str(params.text, None);
        if rect.width() <= params.w {
            canvas.draw_str(
                params.text,
                (params.x + (params.w - rect.width()) / 2.0, params.y),
                &font,
                &paint,
            );
        } else {
            let mut truncated = String::new();
            let mut current_w = 0.0;
            let (ellipsis_w, _) = font.measure_str("...", None);
            let max_w = params.w - ellipsis_w;
            for c in params.text.chars() {
                let (cw, _) = font.measure_str(c.to_string(), None);
                if current_w + cw > max_w {
                    break;
                }
                current_w += cw;
                truncated.push(c);
            }
            truncated.push_str("...");
            canvas.draw_str(&truncated, (params.x, params.y), &font, &paint);
        }
    }

    pub fn measure_text_cached(&self, text: &str, size: f32, style: FontStyle) -> f32 {
        let cache_key = hash_cache_key(text, style, size);
        TEXT_CACHE.with(|cache| {
            let mut cache_mut = cache.borrow_mut();
            if !cache_mut.contains_key(&cache_key) {
                evict_one_if_full(&mut cache_mut, TEXT_CACHE_LIMIT);
            }
            let entry = cache_mut.entry(cache_key).or_insert_with(|| {
                let (width, groups) = compute_text_groups(text, size, style);
                (width, groups)
            });
            entry.0
        })
    }

    pub fn measure_text_prefixes_cached(
        &self,
        text: &str,
        first_end: usize,
        second_end: usize,
        size: f32,
        style: FontStyle,
    ) -> (f32, f32) {
        let cache_key = hash_cache_key(text, style, size);
        TEXT_PREFIX_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            if !cache.contains_key(&cache_key) {
                evict_one_if_full(&mut cache, TEXT_PREFIX_CACHE_LIMIT);
            }
            let widths = cache.entry(cache_key).or_default();
            let mut width_at = |end: usize| {
                if end == 0 {
                    return 0.0;
                }
                if let Some((_, width)) = widths.iter().find(|(cached_end, _)| *cached_end == end) {
                    return *width;
                }
                let width = compute_text_groups(&text[..end], size, style).0;
                if widths.len() >= TEXT_PREFIX_WIDTH_LIMIT {
                    widths.remove(0);
                }
                widths.push((end, width));
                width
            };
            (width_at(first_end), width_at(second_end))
        })
    }

    pub fn draw_text_cached(&self, params: DrawTextCachedParams<'_>) {
        let style = if params.bold {
            FontStyle::bold()
        } else {
            FontStyle::normal()
        };
        let cache_key = hash_cache_key(params.text, style, params.size);
        let paint = text_paint(params.color, params.blur);
        TEXT_CACHE.with(|cache| {
            let mut cache_mut = cache.borrow_mut();
            if !cache_mut.contains_key(&cache_key) {
                evict_one_if_full(&mut cache_mut, TEXT_CACHE_LIMIT);
            }
            let entry = cache_mut
                .entry(cache_key)
                .or_insert_with(|| compute_text_groups(params.text, params.size, style));
            let (_, groups) = entry;
            let canvas = params.painter.canvas();
            let mut x = params.x;
            let y = params.y;
            for (s, tf, embolden, width) in groups {
                let mut font = Font::from_typeface(tf.clone(), params.size);
                font.set_subpixel(true);
                if *embolden {
                    font.set_embolden(true);
                }
                canvas.draw_str(&**s, (x, y), &font, &paint);
                x += *width;
            }
        });
    }

    pub fn draw_text_as_paths_cached(&self, params: DrawTextCachedParams<'_>) {
        let style = if params.bold {
            FontStyle::bold()
        } else {
            FontStyle::normal()
        };
        let cache_key = hash_cache_key(params.text, style, params.size);
        let paint = text_paint(params.color, params.blur);
        TEXT_PATH_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            if !cache.contains_key(&cache_key) {
                evict_one_if_full(&mut cache, TEXT_PATH_CACHE_LIMIT);
            }
            let paths = cache
                .entry(cache_key)
                .or_insert_with(|| compute_text_paths(params.text, params.size, style));
            let canvas = params.painter.canvas();
            canvas.save();
            canvas.translate((params.x, params.y));
            for path in paths {
                canvas.draw_path(path, &paint);
            }
            canvas.restore();
        });
    }
}
