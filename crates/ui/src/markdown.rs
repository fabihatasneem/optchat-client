//! Markdown (GitHub-flavoured: nested and loose lists, task lists, tables, strikethrough, autolinks) parsed once with
//! pulldown-cmark into a small block tree, rendered to GPUI elements with the theme's colors. Code blocks get a
//! language bar, light syntax colors and a copy button. Nothing is ever rendered as HTML.
use crate::theme::{c, C};
use crate::ui::zx;
use crate::ui::icon;
use gpui::prelude::FluentBuilder;
use gpui::*;
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use std::ops::Range;
use std::sync::Arc;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
    pub strike: bool,
    pub link: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct Inline {
    pub text: String,
    pub runs: Vec<(Range<usize>, Style)>,
}
impl Inline {
    fn push(&mut self, s: &str, st: &Style) {
        if s.is_empty() {
            return;
        }
        let a = self.text.len();
        self.text.push_str(s);
        let b = self.text.len();
        if let Some((r, last)) = self.runs.last_mut() {
            if last == st && r.end == a {
                r.end = b;
                return;
            }
        }
        self.runs.push((a..b, st.clone()));
    }
    fn is_empty(&self) -> bool {
        self.text.trim().is_empty()
    }
}

#[derive(Clone, Debug)]
pub enum Block {
    Para(Inline),
    Heading(u8, Inline),
    Code { lang: String, text: String },
    List { start: Option<u64>, items: Vec<(Option<bool>, Vec<Block>)> },
    Quote(Vec<Block>),
    Table { head: Vec<Inline>, rows: Vec<Vec<Inline>> },
    Rule,
}

pub type Doc = Arc<Vec<Block>>;

pub fn parse(text: &str) -> Doc {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);
    let parser = Parser::new_ext(text, opts);
    // a stack of block containers
    enum Frame {
        Root(Vec<Block>),
        Quote(Vec<Block>),
        List { start: Option<u64>, items: Vec<(Option<bool>, Vec<Block>)> },
        Item { task: Option<bool>, blocks: Vec<Block> },
    }
    let mut stack: Vec<Frame> = vec![Frame::Root(vec![])];
    let mut inline: Option<Inline> = None;
    let mut heading: Option<u8> = None;
    let mut code: Option<(String, String)> = None;
    let mut style = Style::default();
    let mut table: Option<(Vec<Inline>, Vec<Vec<Inline>>, bool)> = None;
    let mut row: Vec<Inline> = vec![];
    fn push_block(stack: &mut Vec<Frame>, b: Block) {
        match stack.last_mut().unwrap() {
            Frame::Root(v) | Frame::Quote(v) => v.push(b),
            Frame::Item { blocks, .. } => blocks.push(b),
            Frame::List { items, .. } => {
                if let Some(last) = items.last_mut() {
                    last.1.push(b)
                }
            }
        }
    }
    let flush_inline = |stack: &mut Vec<Frame>, inline: &mut Option<Inline>| {
        if let Some(i) = inline.take() {
            if !i.is_empty() {
                push_block(stack, Block::Para(i));
            }
        }
    };
    for ev in parser {
        match ev {
            Event::Start(t) => match t {
                Tag::Paragraph => {
                    flush_inline(&mut stack, &mut inline);
                    inline = Some(Inline::default());
                }
                Tag::Heading { level, .. } => {
                    flush_inline(&mut stack, &mut inline);
                    heading = Some(level as u8);
                    inline = Some(Inline::default());
                }
                Tag::BlockQuote(_) => {
                    flush_inline(&mut stack, &mut inline);
                    stack.push(Frame::Quote(vec![]));
                }
                Tag::CodeBlock(k) => {
                    flush_inline(&mut stack, &mut inline);
                    let lang = match k {
                        CodeBlockKind::Fenced(l) => l.split_whitespace().next().unwrap_or("").to_string(),
                        CodeBlockKind::Indented => String::new(),
                    };
                    code = Some((lang, String::new()));
                }
                Tag::List(start) => {
                    flush_inline(&mut stack, &mut inline);
                    stack.push(Frame::List { start, items: vec![] });
                }
                Tag::Item => {
                    flush_inline(&mut stack, &mut inline);
                    stack.push(Frame::Item { task: None, blocks: vec![] });
                    inline = Some(Inline::default());
                }
                Tag::Table(_) => {
                    flush_inline(&mut stack, &mut inline);
                    table = Some((vec![], vec![], false));
                }
                Tag::TableHead => {
                    if let Some(t) = table.as_mut() {
                        t.2 = true;
                    }
                    row = vec![];
                }
                Tag::TableRow => row = vec![],
                Tag::TableCell => inline = Some(Inline::default()),
                Tag::Emphasis => style.italic = true,
                Tag::Strong => style.bold = true,
                Tag::Strikethrough => style.strike = true,
                Tag::Link { dest_url, .. } => {
                    let u = dest_url.to_string();
                    style.link = Some(if u.starts_with("http:") || u.starts_with("https:") || u.starts_with("mailto:") { u } else { String::new() });
                }
                Tag::Image { dest_url, .. } => {
                    if inline.is_none() {
                        inline = Some(Inline::default());
                    }
                    inline.as_mut().unwrap().push(&format!("[image {dest_url}]"), &style);
                }
                _ => {}
            },
            Event::End(t) => match t {
                TagEnd::Paragraph => flush_inline(&mut stack, &mut inline),
                TagEnd::Heading(_) => {
                    if let Some(i) = inline.take() {
                        push_block(&mut stack, Block::Heading(heading.take().unwrap_or(1), i));
                    }
                }
                TagEnd::BlockQuote(_) => {
                    flush_inline(&mut stack, &mut inline);
                    if let Some(Frame::Quote(v)) = stack.pop() {
                        push_block(&mut stack, Block::Quote(v));
                    }
                }
                TagEnd::CodeBlock => {
                    if let Some((lang, mut text)) = code.take() {
                        if text.ends_with('\n') {
                            text.pop();
                        }
                        push_block(&mut stack, Block::Code { lang, text });
                    }
                }
                TagEnd::List(_) => {
                    flush_inline(&mut stack, &mut inline);
                    if let Some(Frame::List { start, items }) = stack.pop() {
                        push_block(&mut stack, Block::List { start, items });
                    }
                }
                TagEnd::Item => {
                    flush_inline(&mut stack, &mut inline);
                    if let Some(Frame::Item { task, blocks }) = stack.pop() {
                        if let Some(Frame::List { items, .. }) = stack.last_mut() {
                            items.push((task, blocks));
                        }
                    }
                }
                TagEnd::TableCell => row.push(inline.take().unwrap_or_default()),
                TagEnd::TableHead => {
                    if let Some(t) = table.as_mut() {
                        t.0 = std::mem::take(&mut row);
                        t.2 = false;
                    }
                }
                TagEnd::TableRow => {
                    if let Some(t) = table.as_mut() {
                        t.1.push(std::mem::take(&mut row));
                    }
                }
                TagEnd::Table => {
                    if let Some((head, rows, _)) = table.take() {
                        push_block(&mut stack, Block::Table { head, rows });
                    }
                }
                TagEnd::Emphasis => style.italic = false,
                TagEnd::Strong => style.bold = false,
                TagEnd::Strikethrough => style.strike = false,
                TagEnd::Link => style.link = None,
                _ => {}
            },
            Event::Text(s) => {
                if let Some((_, t)) = code.as_mut() {
                    t.push_str(&s);
                } else {
                    inline.get_or_insert_with(Inline::default).push(&s, &style);
                }
            }
            Event::Code(s) => {
                let mut st = style.clone();
                st.code = true;
                inline.get_or_insert_with(Inline::default).push(&s, &st);
            }
            Event::SoftBreak => inline.get_or_insert_with(Inline::default).push(" ", &style),
            Event::HardBreak => inline.get_or_insert_with(Inline::default).push("\n", &style),
            Event::Rule => {
                flush_inline(&mut stack, &mut inline);
                push_block(&mut stack, Block::Rule);
            }
            Event::TaskListMarker(done) => {
                if let Some(Frame::Item { task, .. }) = stack.last_mut() {
                    *task = Some(done);
                }
            }
            Event::Html(s) | Event::InlineHtml(s) => {
                if let Some((_, t)) = code.as_mut() {
                    t.push_str(&s);
                } else {
                    inline.get_or_insert_with(Inline::default).push(&s, &style);
                }
            }
            _ => {}
        }
    }
    flush_inline(&mut stack, &mut inline);
    while stack.len() > 1 {
        match stack.pop().unwrap() {
            Frame::Quote(v) => push_block(&mut stack, Block::Quote(v)),
            Frame::List { start, items } => push_block(&mut stack, Block::List { start, items }),
            Frame::Item { task, blocks } => {
                if let Some(Frame::List { items, .. }) = stack.last_mut() {
                    items.push((task, blocks));
                }
            }
            Frame::Root(_) => {}
        }
    }
    match stack.pop() {
        Some(Frame::Root(v)) => Arc::new(v),
        _ => Arc::new(vec![]),
    }
}

/// Plain text of a doc (copy).
pub fn plain(text: &str) -> String {
    text.to_string()
}

pub struct Opts {
    pub size: f32,
    pub color: Hsla,
}

fn styled(id: ElementId, il: &Inline, size: f32, color: Hsla, k: &C, window: &Window) -> AnyElement {
    let base = window.text_style().font();
    let mono = Font { family: crate::ui::MONO.into(), ..base.clone() };
    let mut runs = vec![];
    let mut links: Vec<(Range<usize>, String)> = vec![];
    let mut at = 0;
    let fill = |runs: &mut Vec<TextRun>, n: usize| {
        if n > 0 {
            runs.push(TextRun { len: n, font: base.clone(), color, background_color: None, underline: None, strikethrough: None });
        }
    };
    for (r, st) in &il.runs {
        if r.start > at {
            fill(&mut runs, r.start - at);
        }
        let mut f = if st.code { mono.clone() } else { base.clone() };
        if st.bold {
            f.weight = FontWeight::SEMIBOLD;
        }
        if st.italic {
            f.style = FontStyle::Italic;
        }
        let link = st.link.as_ref().filter(|l| !l.is_empty());
        for (on, name) in [(st.code, "code"), (st.bold, "bold"), (st.italic, "italic"), (st.strike, "strike"), (link.is_some(), "link")] {
            if on {
                crate::ui::seen("mdInline", name);
            }
        }
        let col = if link.is_some() { k.accent } else if st.code { k.text } else { color };
        runs.push(TextRun {
            len: r.end - r.start,
            font: f,
            color: col,
            background_color: if st.code { Some(k.panel2) } else { None },
            underline: link.map(|_| UnderlineStyle { thickness: zx(1.), color: Some(k.accent.opacity(0.35)), wavy: false }),
            strikethrough: if st.strike { Some(StrikethroughStyle { thickness: zx(1.), color: Some(color) }) } else { None },
        });
        if let Some(l) = link {
            links.push((r.clone(), l.clone()));
        }
        at = r.end;
    }
    if il.text.len() > at {
        fill(&mut runs, il.text.len() - at);
    }
    let st = StyledText::new(SharedString::from(il.text.clone())).with_runs(runs);
    let line_h = size * 1.6;
    if links.is_empty() {
        div().text_size(zx(size)).line_height(zx(line_h)).text_color(color).child(st).into_any_element()
    } else {
        let ranges: Vec<Range<usize>> = links.iter().map(|l| l.0.clone()).collect();
        let urls: Vec<String> = links.into_iter().map(|l| l.1).collect();
        div()
            .text_size(zx(size))
            .line_height(zx(line_h))
            .text_color(color)
            .child(InteractiveText::new(id, st).on_click(ranges, move |ix, _w, cx| cx.open_url(&urls[ix])))
            .into_any_element()
    }
}

pub fn render(id: &str, doc: &Doc, o: &Opts, window: &Window, cx: &App) -> Div {
    let k = c(cx).clone();
    let mut out = div().flex().flex_col().gap(zx(8.)).min_w_0();
    for (n, b) in doc.iter().enumerate() {
        out = out.child(block(&format!("{id}.{n}"), b, o, &k, window, cx));
    }
    out
}

fn block(id: &str, b: &Block, o: &Opts, k: &C, window: &Window, cx: &App) -> AnyElement {
    // UI tests: which kinds of markdown blocks the feed has drawn
    crate::ui::seen("mdBlocks", match b {
        Block::Para(_) => "para",
        Block::Heading(..) => "heading",
        Block::Code { .. } => "code",
        Block::Rule => "rule",
        Block::Quote(_) => "quote",
        Block::List { items, .. } => if items.iter().any(|x| x.0.is_some()) { "tasklist" } else { "list" },
        Block::Table { .. } => "table",
    });
    match b {
        Block::Para(il) => styled(ElementId::Name(id.to_string().into()), il, o.size, o.color, k, window),
        Block::Heading(l, il) => {
            let size = match l {
                1 => 20.0,
                2 => 17.0,
                3 => 15.5,
                _ => 14.5,
            };
            div().mt(zx(6.)).font_weight(FontWeight::SEMIBOLD).child(styled(ElementId::Name(id.to_string().into()), il, size, k.text, k, window)).into_any_element()
        }
        Block::Code { lang, text } => code_block(id, lang, text, k, cx).into_any_element(),
        Block::Rule => div().h(zx(1.)).bg(k.line2).my(zx(6.)).into_any_element(),
        Block::Quote(bs) => {
            let mut d = div().pl(zx(12.)).border_l_2().border_color(k.line2).flex().flex_col().gap(zx(6.));
            let o2 = Opts { size: o.size, color: k.text2 };
            for (n, b) in bs.iter().enumerate() {
                d = d.child(block(&format!("{id}.q{n}"), b, &o2, k, window, cx));
            }
            d.into_any_element()
        }
        Block::List { start, items } => {
            let mut d = div().flex().flex_col().gap(zx(3.));
            for (n, (task, bs)) in items.iter().enumerate() {
                let marker: AnyElement = match task {
                    Some(done) => div()
                        .size(zx(13.))
                        .mt(zx(4.))
                        .rounded(zx(3.))
                        .border_1()
                        .border_color(k.line2)
                        .when(*done, |d| d.bg(k.accent2).child(icon("check", 11., k.on_accent)))
                        .into_any_element(),
                    None => match start {
                        Some(s) => div().text_size(zx(o.size)).line_height(zx(o.size * 1.6)).text_color(k.muted).child(format!("{}.", s + n as u64)).into_any_element(),
                        None => div().mt(zx(o.size * 0.62)).size(zx(5.)).rounded_full().bg(k.muted).into_any_element(),
                    },
                };
                let mut body = div().flex_1().min_w_0().flex().flex_col().gap(zx(4.));
                for (m, b) in bs.iter().enumerate() {
                    body = body.child(block(&format!("{id}.l{n}.{m}"), b, o, k, window, cx));
                }
                d = d.child(div().flex().gap(zx(8.)).child(div().w(zx(16.)).flex_none().flex().justify_end().child(marker)).child(body));
            }
            d.into_any_element()
        }
        Block::Table { head, rows } => {
            let cols = head.len().max(rows.iter().map(|r| r.len()).max().unwrap_or(0));
            let cell = |il: Option<&Inline>, key: String, bold: bool| {
                let mut d = div().flex_1().min_w(zx(60.)).px(zx(9.)).py(zx(5.)).border_1().border_color(k.line2);
                if bold {
                    d = d.bg(k.panel2).font_weight(FontWeight::SEMIBOLD);
                }
                match il {
                    Some(il) => d.child(styled(ElementId::Name(key.into()), il, 13.0, k.text, k, window)),
                    None => d,
                }
            };
            let mut t = div().flex().flex_col().text_size(zx(13.));
            let mut hr = div().flex();
            for i in 0..cols {
                hr = hr.child(cell(head.get(i), format!("{id}.h{i}"), true));
            }
            t = t.child(hr);
            for (r, row) in rows.iter().enumerate() {
                let mut rr = div().flex();
                for i in 0..cols {
                    rr = rr.child(cell(row.get(i), format!("{id}.r{r}.{i}"), false));
                }
                t = t.child(rr);
            }
            t.into_any_element()
        }
    }
}

pub fn code_block(id: &str, lang: &str, text: &str, k: &C, _cx: &App) -> Stateful<Div> {
    let copy = text.to_string();
    let lang_l = if lang.is_empty() { "text".to_string() } else { lang.to_string() };
    let hover = k.hover;
    div()
        .id(ElementId::Name(format!("{id}.code").into()))
        .rounded(zx(10.))
        .border_1()
        .border_color(k.line)
        .bg(k.code_bg)
        .overflow_hidden()
        .child(
            div()
                .flex()
                .justify_between()
                .items_center()
                .px(zx(10.))
                .py(zx(4.))
                .bg(k.panel)
                .border_b_1()
                .border_color(k.line)
                .text_size(zx(11.5))
                .text_color(k.muted)
                .child(div().font_family(crate::ui::MONO).child(lang_l))
                .child(
                    div()
                        .id(ElementId::Name(format!("{id}.copy").into()))
                        .px(zx(6.))
                        .rounded(zx(5.))
                        .cursor_pointer()
                        .hover(move |s| s.bg(hover))
                        .child("copy")
                        .on_click(move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()))),
                ),
        )
        .child(
            div()
                .id(ElementId::Name(format!("{id}.pre").into()))
                .when(!compact(), |d| d.overflow_x_scroll())
                .px(zx(14.))
                .py(zx(10.))
                .font_family(crate::ui::MONO)
                .text_size(zx(12.5))
                .line_height(zx(19.))
                .child(highlight(text, lang, k)),
        )
}

/// A light, language-agnostic highlighter: comments, strings, numbers, keywords, capitalized types, calls.
pub fn highlight(text: &str, lang: &str, k: &C) -> StyledText {
    const KW: &[&str] = &[
        "fn", "let", "mut", "pub", "use", "mod", "impl", "struct", "enum", "trait", "match", "if", "else", "for", "while", "loop", "return", "async", "await", "const", "static", "type", "where",
        "function", "var", "export", "import", "from", "class", "new", "this", "self", "Self", "true", "false", "null", "undefined", "None", "Some", "def", "in", "of", "and", "or", "not", "is", "as",
        "break", "continue", "try", "catch", "throw", "finally", "interface", "extends", "implements", "package", "func", "go", "defer", "echo", "then", "fi", "do", "done", "case", "esac",
    ];
    let hash_comment = matches!(lang, "sh" | "bash" | "zsh" | "shell" | "python" | "py" | "toml" | "yaml" | "yml" | "ruby" | "rb" | "");
    let mut runs: Vec<(Range<usize>, Hsla)> = vec![];
    let b = text.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let ch = b[i];
        if (ch == b'/' && b.get(i + 1) == Some(&b'/')) || (hash_comment && ch == b'#' && (i == 0 || b[i - 1].is_ascii_whitespace())) {
            let e = text[i..].find('\n').map(|n| i + n).unwrap_or(b.len());
            runs.push((i..e, k.comment));
            i = e;
        } else if ch == b'"' || ch == b'\'' || ch == b'`' {
            let mut e = i + 1;
            while e < b.len() && b[e] != ch && b[e] != b'\n' {
                if b[e] == b'\\' {
                    e += 1;
                }
                e += 1;
            }
            let e = (e + 1).min(b.len());
            runs.push((i..e, k.string));
            i = e;
        } else if ch.is_ascii_digit() && (i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_')) {
            let mut e = i;
            while e < b.len() && (b[e].is_ascii_alphanumeric() || b[e] == b'.' || b[e] == b'_') {
                e += 1;
            }
            runs.push((i..e, k.number));
            i = e;
        } else if ch.is_ascii_alphabetic() || ch == b'_' {
            let mut e = i;
            while e < b.len() && (b[e].is_ascii_alphanumeric() || b[e] == b'_') {
                e += 1;
            }
            let w = &text[i..e];
            if KW.contains(&w) {
                runs.push((i..e, k.kw));
            } else if b.get(e) == Some(&b'(') {
                runs.push((i..e, k.function));
            } else if w.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) && w.len() > 1 {
                runs.push((i..e, k.type_));
            }
            i = e;
        } else {
            // skip a whole UTF-8 char
            let n = text[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
            i += n;
        }
    }
    let hl: Vec<(Range<usize>, HighlightStyle)> = runs.into_iter().map(|(r, col)| (r, HighlightStyle { color: Some(col), ..Default::default() })).collect();
    StyledText::new(SharedString::from(text.to_string())).with_highlights(hl)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn parses() {
        let d = parse("# Title\n\nsome **bold** and `code` [link](https://x.y)\n\n- a\n- [x] b\n  - nested\n\n```rust\nfn x() {}\n```\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n> quote");
        assert!(matches!(d[0], Block::Heading(1, _)));
        match &d[1] {
            Block::Para(i) => {
                assert_eq!(i.text, "some bold and code link");
                assert!(i.runs.iter().any(|(_, s)| s.bold));
                assert!(i.runs.iter().any(|(_, s)| s.code));
                assert!(i.runs.iter().any(|(_, s)| s.link.as_deref() == Some("https://x.y")));
            }
            _ => panic!(),
        }
        assert!(matches!(&d[2], Block::List { items, .. } if items.len() == 2 && items[1].0 == Some(true)));
        assert!(matches!(&d[3], Block::Code { lang, .. } if lang == "rust"));
        assert!(matches!(&d[4], Block::Table { head, rows } if head.len() == 2 && rows.len() == 1));
        assert!(matches!(&d[5], Block::Quote(_)));
    }
}

// the phone / narrow layout: code wraps instead of scrolling sideways (set while the feed draws its rows)
thread_local!(static COMPACT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) });
pub fn set_compact(on: bool) {
    COMPACT.with(|c| c.set(on));
}
pub fn compact() -> bool {
    COMPACT.with(|c| c.get())
}
