use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use winisland_render::text::{DrawTextCachedParams, FontManager};
use winisland_render::{FontStyle, Painter, Point, Radius, Rect, Rgba, StrokeCap, Vec2};

const BODY_SIZE: f32 = 12.0;
const LINE_HEIGHT: f32 = 18.0;
const BLOCK_GAP: f32 = 10.0;
const CODE_PAD: f32 = 10.0;
const INDENT: f32 = 18.0;

thread_local! {
    static LAYOUT_CACHE: RefCell<HashMap<u64, Rc<MarkdownLayout>>> = RefCell::new(HashMap::new());
}

#[derive(Clone)]
pub(crate) struct MarkdownLink {
    pub(crate) rect: Rect,
    pub(crate) url: String,
}

pub(crate) struct MarkdownRenderResult {
    pub(crate) height: f32,
}

pub(crate) struct MarkdownColors {
    pub(crate) text: Rgba,
    pub(crate) secondary: Rgba,
    pub(crate) accent: Rgba,
    pub(crate) code_background: Rgba,
    pub(crate) quote_background: Rgba,
    pub(crate) separator: Rgba,
}

pub(crate) struct MarkdownRenderParams<'a> {
    pub(crate) painter: Painter<'a>,
    pub(crate) markdown: &'a str,
    pub(crate) origin: (f32, f32),
    pub(crate) width: f32,
    pub(crate) visible_range: (f32, f32),
    pub(crate) colors: MarkdownColors,
}

#[derive(Clone, Default)]
struct InlineStyle {
    bold: bool,
    italic: bool,
    strike: bool,
    code: bool,
    code_block: bool,
    link: Option<String>,
}

#[derive(Clone)]
struct InlineSpan {
    text: String,
    style: InlineStyle,
}

#[derive(Clone)]
enum Block {
    Paragraph(Vec<InlineSpan>),
    Heading(HeadingLevel, Vec<InlineSpan>),
    ListItem {
        depth: usize,
        marker: String,
        checked: Option<bool>,
        content: Vec<InlineSpan>,
    },
    Quote(Vec<InlineSpan>),
    Code {
        language: Option<String>,
        text: String,
    },
    TableRow {
        header: bool,
        cells: Vec<Vec<InlineSpan>>,
    },
    Rule,
}

#[derive(Clone)]
enum DrawOp {
    Text {
        text: String,
        x: f32,
        baseline: f32,
        size: f32,
        style: InlineStyle,
        secondary: bool,
    },
    Background {
        rect: Rect,
        kind: BackgroundKind,
    },
    Line {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        width: f32,
        kind: LineKind,
    },
    Checkbox {
        rect: Rect,
        checked: bool,
    },
}

#[derive(Clone, Copy)]
enum BackgroundKind {
    Code,
    TableHeader,
}

#[derive(Clone, Copy)]
enum LineKind {
    Separator,
    Quote,
    Strike,
}

#[derive(Clone)]
struct MarkdownLayout {
    height: f32,
    ops: Vec<DrawOp>,
    links: Vec<MarkdownLink>,
}

#[derive(Clone, Copy)]
enum InlineBlockKind {
    Paragraph,
    Heading(HeadingLevel),
    ListItem,
    Quote,
    TableCell,
}

struct MarkdownParser {
    blocks: Vec<Block>,
    current: Vec<InlineSpan>,
    current_kind: Option<InlineBlockKind>,
    style: InlineStyle,
    lists: Vec<Option<u64>>,
    item_marker: Option<String>,
    task_checked: Option<bool>,
    quote_depth: usize,
    code: Option<(Option<String>, String)>,
    table_header: bool,
    table_cells: Vec<Vec<InlineSpan>>,
    image_url: Option<String>,
}

impl MarkdownParser {
    fn new() -> Self {
        Self {
            blocks: Vec::new(),
            current: Vec::new(),
            current_kind: None,
            style: InlineStyle::default(),
            lists: Vec::new(),
            item_marker: None,
            task_checked: None,
            quote_depth: 0,
            code: None,
            table_header: false,
            table_cells: Vec::new(),
            image_url: None,
        }
    }

    fn parse(mut self, markdown: &str) -> Vec<Block> {
        let options = Options::ENABLE_STRIKETHROUGH
            | Options::ENABLE_TABLES
            | Options::ENABLE_TASKLISTS
            | Options::ENABLE_FOOTNOTES;
        for event in Parser::new_ext(markdown, options) {
            self.event(event);
        }
        self.flush_inline();
        self.blocks
    }

    fn event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) => {
                if let Some((_, code)) = self.code.as_mut() {
                    code.push_str(&text);
                } else {
                    self.push_text(&text);
                }
            }
            Event::Code(text) => {
                let previous = self.style.code;
                self.style.code = true;
                self.push_text(&text);
                self.style.code = previous;
            }
            Event::InlineMath(text) | Event::DisplayMath(text) => {
                self.push_text(&format!("${text}$"));
            }
            Event::SoftBreak => self.push_text(" "),
            Event::HardBreak => self.push_text("\n"),
            Event::Rule => {
                self.flush_inline();
                self.blocks.push(Block::Rule);
            }
            Event::TaskListMarker(checked) => self.task_checked = Some(checked),
            Event::FootnoteReference(label) => self.push_text(&format!("[{label}]")),
            Event::Html(_) | Event::InlineHtml(_) => {}
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => self.begin_inline(if self.item_marker.is_some() {
                InlineBlockKind::ListItem
            } else if self.quote_depth > 0 {
                InlineBlockKind::Quote
            } else {
                InlineBlockKind::Paragraph
            }),
            Tag::Heading { level, .. } => self.begin_inline(InlineBlockKind::Heading(level)),
            Tag::BlockQuote(_) => self.quote_depth += 1,
            Tag::CodeBlock(kind) => {
                self.flush_inline();
                let language = match kind {
                    CodeBlockKind::Indented => None,
                    CodeBlockKind::Fenced(language) if language.is_empty() => None,
                    CodeBlockKind::Fenced(language) => Some(language.into_string()),
                };
                self.code = Some((language, String::new()));
            }
            Tag::List(start) => self.lists.push(start),
            Tag::Item => {
                let marker = if let Some(Some(next)) = self.lists.last_mut() {
                    let marker = format!("{next}.");
                    *next += 1;
                    marker
                } else {
                    "•".to_string()
                };
                self.item_marker = Some(marker);
                self.task_checked = None;
            }
            Tag::Strong => self.style.bold = true,
            Tag::Emphasis => self.style.italic = true,
            Tag::Strikethrough => self.style.strike = true,
            Tag::Link { dest_url, .. } => self.style.link = Some(dest_url.into_string()),
            Tag::Image { dest_url, .. } => {
                self.image_url = Some(dest_url.into_string());
                self.push_text("[Image: ");
            }
            Tag::Table(_) => {
                self.flush_inline();
                self.table_cells.clear();
            }
            Tag::TableHead => self.table_header = true,
            Tag::TableRow => self.table_cells.clear(),
            Tag::TableCell => {
                self.current.clear();
                self.current_kind = Some(InlineBlockKind::TableCell);
            }
            Tag::HtmlBlock
            | Tag::FootnoteDefinition(_)
            | Tag::MetadataBlock(_)
            | Tag::DefinitionList
            | Tag::DefinitionListTitle
            | Tag::DefinitionListDefinition
            | Tag::Superscript
            | Tag::Subscript => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph | TagEnd::Heading(_) => self.flush_inline(),
            TagEnd::BlockQuote(_) => self.quote_depth = self.quote_depth.saturating_sub(1),
            TagEnd::CodeBlock => {
                if let Some((language, text)) = self.code.take() {
                    self.blocks.push(Block::Code { language, text });
                }
            }
            TagEnd::List(_) => {
                self.lists.pop();
            }
            TagEnd::Item => {
                self.flush_inline();
                self.item_marker = None;
                self.task_checked = None;
            }
            TagEnd::Strong => self.style.bold = false,
            TagEnd::Emphasis => self.style.italic = false,
            TagEnd::Strikethrough => self.style.strike = false,
            TagEnd::Link => self.style.link = None,
            TagEnd::Image => {
                self.push_text("]");
                self.image_url = None;
                self.style.link = None;
            }
            TagEnd::TableCell => {
                self.table_cells.push(std::mem::take(&mut self.current));
                self.current_kind = None;
            }
            TagEnd::TableRow => self.blocks.push(Block::TableRow {
                header: self.table_header,
                cells: std::mem::take(&mut self.table_cells),
            }),
            TagEnd::TableHead => self.table_header = false,
            TagEnd::Table
            | TagEnd::HtmlBlock
            | TagEnd::FootnoteDefinition
            | TagEnd::MetadataBlock(_)
            | TagEnd::DefinitionList
            | TagEnd::DefinitionListTitle
            | TagEnd::DefinitionListDefinition
            | TagEnd::Superscript
            | TagEnd::Subscript => {}
        }
    }

    fn begin_inline(&mut self, kind: InlineBlockKind) {
        self.flush_inline();
        self.current_kind = Some(kind);
    }

    fn push_text(&mut self, text: &str) {
        if self.current_kind.is_none() {
            self.current_kind = Some(if self.item_marker.is_some() {
                InlineBlockKind::ListItem
            } else if self.quote_depth > 0 {
                InlineBlockKind::Quote
            } else {
                InlineBlockKind::Paragraph
            });
        }
        let mut style = self.style.clone();
        if let Some(url) = self.image_url.as_ref() {
            style.link = Some(url.clone());
        }
        if let Some(last) = self.current.last_mut()
            && last.style.bold == style.bold
            && last.style.italic == style.italic
            && last.style.strike == style.strike
            && last.style.code == style.code
            && last.style.code_block == style.code_block
            && last.style.link == style.link
        {
            last.text.push_str(text);
        } else {
            self.current.push(InlineSpan {
                text: text.to_string(),
                style,
            });
        }
    }

    fn flush_inline(&mut self) {
        let Some(kind) = self.current_kind.take() else {
            return;
        };
        let content = std::mem::take(&mut self.current);
        if content.is_empty() {
            return;
        }
        let block = match kind {
            InlineBlockKind::Paragraph => Block::Paragraph(content),
            InlineBlockKind::Heading(level) => Block::Heading(level, content),
            InlineBlockKind::Quote => Block::Quote(content),
            InlineBlockKind::ListItem => Block::ListItem {
                depth: self.lists.len().saturating_sub(1),
                marker: self.item_marker.clone().unwrap_or_else(|| "•".to_string()),
                checked: self.task_checked,
                content,
            },
            InlineBlockKind::TableCell => {
                self.current = content;
                self.current_kind = Some(InlineBlockKind::TableCell);
                return;
            }
        };
        self.blocks.push(block);
    }
}

struct LayoutBuilder<'a> {
    fm: &'a FontManager,
    width: f32,
    y: f32,
    ops: Vec<DrawOp>,
    links: Vec<MarkdownLink>,
}

struct InlineLayout<'a> {
    spans: &'a [InlineSpan],
    origin: (f32, f32),
    width: f32,
    size: f32,
    force_bold: bool,
    secondary: bool,
}

impl<'a> LayoutBuilder<'a> {
    fn new(fm: &'a FontManager, width: f32) -> Self {
        Self {
            fm,
            width,
            y: 0.0,
            ops: Vec::new(),
            links: Vec::new(),
        }
    }

    fn build(mut self, blocks: &[Block]) -> MarkdownLayout {
        for block in blocks {
            self.block(block);
        }
        MarkdownLayout {
            height: self.y.max(LINE_HEIGHT),
            ops: self.ops,
            links: self.links,
        }
    }

    fn block(&mut self, block: &Block) {
        match block {
            Block::Paragraph(spans) => {
                self.inline(spans, 0.0, self.width, BODY_SIZE, false, false);
                self.y += BLOCK_GAP;
            }
            Block::Heading(level, spans) => {
                let size = match level {
                    HeadingLevel::H1 => 22.0,
                    HeadingLevel::H2 => 19.0,
                    HeadingLevel::H3 => 16.0,
                    HeadingLevel::H4 => 14.0,
                    HeadingLevel::H5 | HeadingLevel::H6 => 13.0,
                };
                self.inline(spans, 0.0, self.width, size, true, false);
                self.y += BLOCK_GAP + 2.0;
            }
            Block::ListItem {
                depth,
                marker,
                checked,
                content,
            } => {
                let indent = *depth as f32 * INDENT;
                if let Some(checked) = checked {
                    self.ops.push(DrawOp::Checkbox {
                        rect: Rect::from_xywh(indent + 1.0, self.y + 2.0, 10.0, 10.0),
                        checked: *checked,
                    });
                } else {
                    self.ops.push(DrawOp::Text {
                        text: marker.clone(),
                        x: indent + 1.0,
                        baseline: self.y + BODY_SIZE,
                        size: BODY_SIZE,
                        style: InlineStyle::default(),
                        secondary: true,
                    });
                }
                self.inline(
                    content,
                    indent + INDENT,
                    self.width - indent - INDENT,
                    BODY_SIZE,
                    false,
                    false,
                );
                self.y += 4.0;
            }
            Block::Quote(spans) => {
                let start = self.y;
                self.inline(spans, 13.0, self.width - 13.0, BODY_SIZE, false, true);
                let height = (self.y - start).max(LINE_HEIGHT);
                self.ops.push(DrawOp::Line {
                    x1: 2.0,
                    y1: start,
                    x2: 2.0,
                    y2: start + height,
                    width: 2.0,
                    kind: LineKind::Quote,
                });
                self.y += BLOCK_GAP;
            }
            Block::Code { language, text } => {
                let start = self.y;
                let mut lines = text.lines().collect::<Vec<_>>();
                if lines.is_empty() {
                    lines.push("");
                }
                if let Some(language) = language {
                    self.ops.push(DrawOp::Text {
                        text: language.clone(),
                        x: CODE_PAD,
                        baseline: self.y + 11.0,
                        size: 10.0,
                        style: InlineStyle::default(),
                        secondary: true,
                    });
                    self.y += 18.0;
                }
                for line in lines {
                    let line = [InlineSpan {
                        text: line.to_string(),
                        style: InlineStyle {
                            code: true,
                            code_block: true,
                            ..InlineStyle::default()
                        },
                    }];
                    self.inline_at(InlineLayout {
                        spans: &line,
                        origin: (CODE_PAD, self.y),
                        width: self.width - CODE_PAD * 2.0,
                        size: 11.0,
                        force_bold: false,
                        secondary: false,
                    });
                }
                self.ops.push(DrawOp::Background {
                    rect: Rect::from_xywh(0.0, start - 5.0, self.width, self.y - start + 10.0),
                    kind: BackgroundKind::Code,
                });
                self.y += BLOCK_GAP;
            }
            Block::TableRow { header, cells } => {
                let start = self.y;
                let columns = cells.len().max(1);
                let cell_width = self.width / columns as f32;
                let mut row_height: f32 = LINE_HEIGHT + 8.0;
                for (index, cell) in cells.iter().enumerate() {
                    let before = self.y;
                    self.inline_at(InlineLayout {
                        spans: cell,
                        origin: (index as f32 * cell_width + 6.0, start),
                        width: cell_width - 12.0,
                        size: 11.0,
                        force_bold: *header,
                        secondary: false,
                    });
                    row_height = row_height.max(self.y - before + 8.0);
                    self.y = start;
                }
                if *header {
                    self.ops.push(DrawOp::Background {
                        rect: Rect::from_xywh(0.0, start - 4.0, self.width, row_height),
                        kind: BackgroundKind::TableHeader,
                    });
                }
                self.y = start + row_height;
                self.ops.push(DrawOp::Line {
                    x1: 0.0,
                    y1: self.y,
                    x2: self.width,
                    y2: self.y,
                    width: 1.0,
                    kind: LineKind::Separator,
                });
            }
            Block::Rule => {
                self.y += 5.0;
                self.ops.push(DrawOp::Line {
                    x1: 0.0,
                    y1: self.y,
                    x2: self.width,
                    y2: self.y,
                    width: 1.0,
                    kind: LineKind::Separator,
                });
                self.y += BLOCK_GAP + 5.0;
            }
        }
    }

    fn inline(
        &mut self,
        spans: &[InlineSpan],
        x: f32,
        width: f32,
        size: f32,
        force_bold: bool,
        secondary: bool,
    ) {
        let start_y = self.y;
        self.inline_at(InlineLayout {
            spans,
            origin: (x, start_y),
            width,
            size,
            force_bold,
            secondary,
        });
    }

    fn inline_at(&mut self, params: InlineLayout<'_>) {
        let InlineLayout {
            spans,
            origin: (start_x, start_y),
            width,
            size,
            force_bold,
            secondary,
        } = params;
        let mut x = start_x;
        let mut baseline = start_y + size;
        let line_height = size * 1.5;
        let max_x = start_x + width;
        for span in spans {
            let mut style = span.style.clone();
            style.bold |= force_bold;
            let font_style = if style.bold {
                FontStyle::bold()
            } else {
                FontStyle::normal()
            };
            let mut run = String::new();
            let mut run_width = 0.0;
            let flush = |builder: &mut Self,
                         run: &mut String,
                         run_width: &mut f32,
                         x: &mut f32,
                         baseline: f32| {
                if run.is_empty() {
                    return;
                }
                let run_x = *x;
                let text = std::mem::take(run);
                builder.ops.push(DrawOp::Text {
                    text,
                    x: run_x,
                    baseline,
                    size,
                    style: style.clone(),
                    secondary,
                });
                if let Some(url) = style.link.as_ref()
                    && safe_web_url(url)
                {
                    builder.links.push(MarkdownLink {
                        rect: Rect::from_xywh(run_x, baseline - size, *run_width, line_height),
                        url: url.clone(),
                    });
                }
                if style.strike {
                    builder.ops.push(DrawOp::Line {
                        x1: run_x,
                        y1: baseline - size * 0.35,
                        x2: run_x + *run_width,
                        y2: baseline - size * 0.35,
                        width: 1.0,
                        kind: LineKind::Strike,
                    });
                }
                *x += *run_width;
                *run_width = 0.0;
            };
            for character in span.text.chars() {
                if character == '\n' {
                    flush(self, &mut run, &mut run_width, &mut x, baseline);
                    x = start_x;
                    baseline += line_height;
                    continue;
                }
                let char_width =
                    self.fm
                        .measure_text_cached(&character.to_string(), size, font_style);
                if x + run_width + char_width > max_x && (x > start_x || !run.is_empty()) {
                    flush(self, &mut run, &mut run_width, &mut x, baseline);
                    x = start_x;
                    baseline += line_height;
                    if character.is_whitespace() {
                        continue;
                    }
                }
                run.push(character);
                run_width += char_width;
            }
            flush(self, &mut run, &mut run_width, &mut x, baseline);
        }
        self.y = self.y.max(baseline - size + line_height);
    }
}

pub(crate) fn clear_cache() {
    LAYOUT_CACHE.with(|cache| cache.borrow_mut().clear());
}

pub(crate) fn markdown_height(markdown: &str, width: f32) -> f32 {
    layout(markdown, width).height
}

pub(crate) fn render(params: MarkdownRenderParams<'_>) -> MarkdownRenderResult {
    let MarkdownRenderParams {
        painter,
        markdown,
        origin: (x, y),
        width,
        visible_range: (visible_top, visible_bottom),
        colors,
    } = params;
    let layout = layout(markdown, width);
    let save_count = painter.save();
    painter.clip_rect(Rect::from_xywh(
        x - 3.0,
        y - 6.0,
        width + 3.0,
        layout.height + 12.0,
    ));
    for op in &layout.ops {
        let DrawOp::Background { rect, kind } = op else {
            continue;
        };
        let rect = Rect::from_xywh(x + rect.left, y + rect.top, rect.width(), rect.height());
        if rect.bottom < visible_top || rect.top > visible_bottom {
            continue;
        }
        let color = match kind {
            BackgroundKind::Code => colors.code_background,
            BackgroundKind::TableHeader => colors.quote_background,
        };
        painter.fill_round_rect(rect, Radius::uniform(7.0), color);
    }
    for op in &layout.ops {
        match op {
            DrawOp::Text {
                text,
                x: op_x,
                baseline,
                size,
                style,
                secondary,
            } => {
                let baseline = y + baseline;
                if baseline + 4.0 < visible_top || baseline - size > visible_bottom {
                    continue;
                }
                let color = if style.link.is_some() {
                    colors.accent
                } else if *secondary {
                    colors.secondary
                } else {
                    colors.text
                };
                if style.code && !style.code_block {
                    let width = FontManager::global().measure_text_cached(
                        text,
                        *size,
                        if style.bold {
                            FontStyle::bold()
                        } else {
                            FontStyle::normal()
                        },
                    );
                    painter.fill_round_rect(
                        Rect::from_xywh(
                            x + op_x - 2.0,
                            baseline - size - 2.0,
                            width + 4.0,
                            size + 6.0,
                        ),
                        Radius::uniform(3.0),
                        colors.code_background,
                    );
                }
                draw_text(painter, text, x + op_x, baseline, *size, style, color);
            }
            DrawOp::Background { .. } => {}
            DrawOp::Checkbox { rect, checked } => {
                let rect =
                    Rect::from_xywh(x + rect.left, y + rect.top, rect.width(), rect.height());
                if rect.bottom < visible_top || rect.top > visible_bottom {
                    continue;
                }
                let color = if *checked {
                    colors.accent
                } else {
                    colors.code_background
                };
                painter.fill_round_rect(rect, Radius::uniform(2.0), color);
                if *checked {
                    painter.stroke_line(
                        Point::new(rect.left + 2.3, rect.top + 5.2),
                        Point::new(rect.left + 4.4, rect.top + 7.2),
                        1.4,
                        Rgba::WHITE,
                        StrokeCap::Butt,
                    );
                    painter.stroke_line(
                        Point::new(rect.left + 4.4, rect.top + 7.2),
                        Point::new(rect.right - 2.0, rect.top + 2.7),
                        1.4,
                        Rgba::WHITE,
                        StrokeCap::Butt,
                    );
                } else {
                    painter.stroke_round_rect(rect, Radius::uniform(2.0), 1.0, colors.separator);
                }
            }
            DrawOp::Line {
                x1,
                y1,
                x2,
                y2,
                width,
                kind,
            } => {
                if y + y1 < visible_top || y + y1 > visible_bottom {
                    continue;
                }
                let color = match kind {
                    LineKind::Quote => colors.accent,
                    LineKind::Separator => colors.separator,
                    LineKind::Strike => colors.secondary,
                };
                painter.stroke_line(
                    Point::new(x + x1, y + y1),
                    Point::new(x + x2, y + y2),
                    *width,
                    color,
                    StrokeCap::Butt,
                );
            }
        }
    }
    painter.restore_to(save_count);
    MarkdownRenderResult {
        height: layout.height,
    }
}

pub(crate) fn links(markdown: &str, x: f32, y: f32, width: f32) -> Vec<MarkdownLink> {
    let layout = layout(markdown, width);
    layout
        .links
        .iter()
        .map(|link| MarkdownLink {
            rect: Rect::from_xywh(
                x + link.rect.left,
                y + link.rect.top,
                link.rect.width(),
                link.rect.height(),
            ),
            url: link.url.clone(),
        })
        .collect()
}

fn layout(markdown: &str, width: f32) -> Rc<MarkdownLayout> {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    markdown.hash(&mut hasher);
    width.to_bits().hash(&mut hasher);
    let key = hasher.finish();
    LAYOUT_CACHE.with(|cache| {
        if let Some(layout) = cache.borrow().get(&key) {
            return Rc::clone(layout);
        }
        let blocks = MarkdownParser::new().parse(markdown);
        let layout = Rc::new(LayoutBuilder::new(FontManager::global(), width).build(&blocks));
        let mut cache = cache.borrow_mut();
        if cache.len() >= 16 {
            cache.clear();
        }
        cache.insert(key, Rc::clone(&layout));
        layout
    })
}

fn draw_text(
    painter: Painter<'_>,
    text: &str,
    x: f32,
    baseline: f32,
    size: f32,
    style: &InlineStyle,
    color: Rgba,
) {
    if style.italic {
        painter.save();
        painter.translate(Vec2::new(x, baseline));
        painter.skew(Vec2::new(-0.12, 0.0));
        FontManager::global().draw_text_cached(DrawTextCachedParams {
            painter,
            text,
            x: 0.0,
            y: 0.0,
            size,
            bold: style.bold,
            color,
            blur: None,
        });
        painter.restore();
    } else {
        FontManager::global().draw_text_cached(DrawTextCachedParams {
            painter,
            text,
            x,
            y: baseline,
            size,
            bold: style.bold,
            color,
            blur: None,
        });
    }
}

pub(crate) fn safe_web_url(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}
