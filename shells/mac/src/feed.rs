//! The chat feed (app/ui/chat.tsx): a virtualized, variable-height list (GPUI `list`, bottom-aligned like a chat log)
//! of rows built from the task's log grouped by turn: user/parent bubbles, report cards, markdown replies, tool cards
//! (input/output, images, per-tool stop), live streaming text and thinking, and each turn's chips.
use crate::fmt;
use crate::markdown::{self, Doc};
use crate::state::{task_name, AppState, ToolState};
use crate::theme::{c, C};
use crate::ui::*;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::menu::{ContextMenuExt, PopupMenuItem};
use optchat_engine::types::*;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[derive(Clone)]
pub enum Row {
    Divider,
    Empty,
    Skeleton,
    User(Arc<LogEntry>),
    Parent(Arc<LogEntry>),
    Report(Arc<LogEntry>),
    Work(Arc<LogEntry>),
    Talk(Arc<LogEntry>),
    Image(Arc<LogEntry>),
    Tool { e: Arc<LogEntry>, echo: Option<Arc<LogEntry>>, images: Vec<Arc<LogEntry>> },
    Live { turn_id: String },
    Foot { turn_id: String },
}

pub enum FeedEvent {
    SelectRequest(String),
    OpenTask(i64),
    Lightbox(String),
}
impl EventEmitter<FeedEvent> for Feed {}

pub struct Feed {
    pub state: Entity<AppState>,
    pub list: ListState,
    rows: Vec<Row>,
    keys: Vec<(String, u64)>,
    rev: u64,
    task: i64,
    seq: u64,
    md: HashMap<i64, Doc>,
    live_md: HashMap<String, (usize, Doc)>,
    open: HashSet<String>,
    full: HashSet<i64>,
    pub images: HashMap<String, Option<Arc<Image>>>,
    _ticker: Option<Task<()>>,
    _subs: Vec<Subscription>,
}

const WORK_RE: &str = r"^\[[ac]\d+\] ";

impl Feed {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Feed {
        let list = ListState::new(0, ListAlignment::Bottom, px(800.));
        let sub = cx.observe(&state, |this: &mut Feed, _, cx| this.sync(cx));
        let mut f = Feed {
            state,
            list,
            rows: vec![],
            keys: vec![],
            rev: u64::MAX,
            task: -1,
            seq: u64::MAX,
            md: HashMap::new(),
            live_md: HashMap::new(),
            open: HashSet::new(),
            full: HashSet::new(),
            images: HashMap::new(),
            _ticker: None,
            _subs: vec![sub],
        };
        f.sync(cx);
        // a 1 s tick while something runs: elapsed timers
        f._ticker = Some(cx.spawn(async move |this, cx| loop {
            Timer::after(std::time::Duration::from_millis(1000)).await;
            let alive = this.update(cx, |f, cx| {
                let running = f.state.read(cx).cur.running.is_some() || f.state.read(cx).cur.tools.values().any(|t| t.running);
                if running {
                    cx.notify();
                }
            });
            if alive.is_err() {
                break;
            }
        }));
        f
    }

    fn toggle(&mut self, key: &str, cx: &mut Context<Self>) {
        if !self.open.remove(key) {
            self.open.insert(key.to_string());
        }
        self.rev = u64::MAX; // force a rebuild of versions
        self.sync(cx);
    }

    pub fn scroll_to_bottom(&mut self) {
        let n = self.rows.len();
        if n > 0 {
            self.list.scroll_to_reveal_item(n - 1);
        }
    }

    /// Rebuild the rows when the state's feed revision moved; splice only from the first changed row.
    fn sync(&mut self, cx: &mut Context<Self>) {
        let s = self.state.read(cx);
        if s.feed_rev == self.rev && s.open_seq == self.seq {
            return;
        }
        let switched = s.open_seq != self.seq || s.current != self.task;
        self.rev = s.feed_rev;
        self.seq = s.open_seq;
        if switched {
            self.task = s.current;
            self.md.clear();
            self.live_md.clear();
            self.open.clear();
            self.full.clear();
        }
        let rows = build_rows(s);
        let keys: Vec<(String, u64)> = rows.iter().map(|r| row_key(r, s, &self.open)).collect();
        if switched {
            self.list.reset(rows.len());
        } else {
            let k = self.keys.iter().zip(keys.iter()).position(|(a, b)| a != b).unwrap_or(self.keys.len().min(keys.len()));
            let old = self.keys.len();
            if k < old || k < keys.len() {
                self.list.splice(k..old, keys.len() - k);
            }
        }
        self.rows = rows;
        self.keys = keys;
        cx.notify();
    }

    fn doc(&mut self, e: &LogEntry) -> Doc {
        if let Some(d) = self.md.get(&e.i) {
            return d.clone();
        }
        let d = markdown::parse(&e.text);
        self.md.insert(e.i, d.clone());
        d
    }

    fn image(&mut self, blob: &str, cx: &mut Context<Self>) -> Option<Arc<Image>> {
        if let Some(v) = self.images.get(blob) {
            return v.clone();
        }
        self.images.insert(blob.to_string(), None);
        let blob = blob.to_string();
        let b2 = blob.clone();
        let st = self.state.clone();
        let (tx, rx) = tokio::sync::oneshot::channel();
        if let Some(h) = st.read(cx).hub() {
            st.read(cx).rt.spawn(async move {
                let r = h.store.get_blob(&b2).ok().flatten();
                let _ = tx.send(r);
            });
        }
        cx.spawn(async move |this, cx| {
            if let Ok(Some(b)) = rx.await {
                let fmt = match b.media_type.as_str() {
                    "image/png" => ImageFormat::Png,
                    "image/jpeg" => ImageFormat::Jpeg,
                    "image/gif" => ImageFormat::Gif,
                    "image/webp" => ImageFormat::Webp,
                    _ => ImageFormat::Png,
                };
                let img = Arc::new(Image::from_bytes(fmt, b.bytes));
                let _ = this.update(cx, |f, cx| {
                    f.images.insert(blob.clone(), Some(img));
                    // rows holding this image change height once it loads
                    let idx: Vec<usize> = f
                        .rows
                        .iter()
                        .enumerate()
                        .filter(|(_, r)| row_blobs(r).iter().any(|x| *x == blob))
                        .map(|(i, _)| i)
                        .collect();
                    for i in idx {
                        f.list.splice(i..i + 1, 1);
                    }
                    cx.notify();
                });
            }
        })
        .detach();
        None
    }
}

fn row_blobs(r: &Row) -> Vec<String> {
    match r {
        Row::User(e) | Row::Parent(e) | Row::Report(e) | Row::Image(e) => e.image.iter().map(|i| i.blob.clone()).collect(),
        Row::Tool { images, .. } => images.iter().filter_map(|e| e.image.as_ref().map(|i| i.blob.clone())).collect(),
        _ => vec![],
    }
}

fn row_key(r: &Row, s: &AppState, open: &HashSet<String>) -> (String, u64) {
    let o = |k: &str| if open.contains(k) { 1u64 << 40 } else { 0 };
    match r {
        Row::Divider => ("div".into(), 0),
        Row::Empty => ("empty".into(), s.tasks.get(&s.current).map(|t| t.task.status as u64).unwrap_or(0)),
        Row::Skeleton => ("skel".into(), 0),
        Row::User(e) | Row::Parent(e) | Row::Work(e) | Row::Talk(e) | Row::Image(e) => (format!("e{}", e.i), e.text.len() as u64 + o(&format!("e{}", e.i))),
        Row::Report(e) => (format!("r{}", e.i), e.text.len() as u64 + o(&format!("r{}", e.i))),
        Row::Tool { e, echo, images } => {
            let k = format!("t{}", e.i);
            let st = e.meta_str("callId").and_then(|c| s.cur.tools.get(c)).map(|t| if t.running { 1 } else { 2 }).unwrap_or(0);
            (k.clone(), echo.as_ref().map(|x| x.i as u64 + 1).unwrap_or(0) * 8 + st + images.len() as u64 * 1000 + o(&k))
        }
        Row::Live { turn_id } => {
            let n: usize = s.cur.live.values().filter(|l| &l.turn_id == turn_id).map(|l| l.text.len() * 3 + l.thinking.len()).sum();
            let st = s.cur.turns.get(turn_id).map(|t| t.status.len()).unwrap_or(0);
            let tools = s.cur.tools.values().filter(|t| &t.turn_id == turn_id && t.running).count();
            let k = format!("live{turn_id}");
            (k.clone(), (n as u64) << 4 | st as u64 | (tools as u64) << 32 | o(&k))
        }
        Row::Foot { turn_id } => {
            let t = s.cur.turns.get(turn_id);
            let reqs = s.cur.requests.values().filter(|r| r.turn_id.as_deref() == Some(turn_id)).count();
            let k = format!("foot{turn_id}");
            let v = t.map(|t| (t.cost.total * 1e6) as u64 ^ (t.status.len() as u64) << 50 ^ t.usage.input as u64).unwrap_or(0) ^ (reqs as u64) << 20 ^ t.and_then(|t| t.error.as_ref()).map(|e| e.len() as u64).unwrap_or(0);
            (k.clone(), v ^ o(&k))
        }
    }
}

pub fn build_rows(s: &AppState) -> Vec<Row> {
    if !s.cur.loaded {
        return vec![Row::Skeleton];
    }
    let log = &s.cur.log;
    if log.is_empty() && s.cur.running.is_none() {
        return vec![Row::Empty];
    }
    let unrun = s.tasks.get(&s.current).and_then(|t| t.task.unrun);
    let work = regex_lite(WORK_RE);
    let running_id = s.cur.running.as_ref().map(|r| r.id.clone());
    let mut rows = vec![];
    // groups by turnId
    let mut k = 0;
    let mut seen_running = false;
    while k < log.len() {
        let tid = log[k].turn_id.clone();
        let mut end = k + 1;
        if tid.is_some() {
            while end < log.len() && log[end].turn_id == tid {
                end += 1;
            }
        }
        let g = &log[k..end];
        if unrun == Some(g[0].i) {
            rows.push(Row::Divider);
        }
        // tool echoes by call id
        let mut echoes: HashMap<String, Arc<LogEntry>> = HashMap::new();
        let mut echo_imgs: HashMap<String, Vec<Arc<LogEntry>>> = HashMap::new();
        for e in g {
            if e.kind != LogKind::Echo {
                continue;
            }
            if let Some(cid) = e.meta_str("callId") {
                if e.image.is_some() {
                    echo_imgs.entry(cid.into()).or_default().push(e.clone());
                } else {
                    echoes.entry(cid.into()).or_insert_with(|| e.clone());
                }
            }
        }
        for e in g {
            match e.kind {
                LogKind::User if work(&e.text) => rows.push(Row::Work(e.clone())),
                LogKind::User | LogKind::Note => rows.push(if e.image.is_some() { Row::Image(e.clone()) } else { Row::User(e.clone()) }),
                LogKind::Parent => rows.push(Row::Parent(e.clone())),
                LogKind::Task => rows.push(Row::Report(e.clone())),
                LogKind::Talk => rows.push(Row::Talk(e.clone())),
                LogKind::Tool => {
                    let cid = e.meta_str("callId").unwrap_or("").to_string();
                    rows.push(Row::Tool { e: e.clone(), echo: echoes.get(&cid).cloned(), images: echo_imgs.get(&cid).cloned().unwrap_or_default() });
                }
                LogKind::Echo => {
                    let has_tool = e.meta_str("callId").map(|c| g.iter().any(|x| x.kind == LogKind::Tool && x.meta_str("callId") == Some(c))).unwrap_or(false);
                    if !has_tool {
                        rows.push(if e.image.is_some() { Row::Image(e.clone()) } else { Row::Tool { e: e.clone(), echo: Some(e.clone()), images: vec![] } });
                    }
                }
            }
        }
        if let Some(t) = &tid {
            if Some(t) == running_id.as_ref() {
                rows.push(Row::Live { turn_id: t.clone() });
                seen_running = true;
            }
            if s.cur.turns.contains_key(t) {
                rows.push(Row::Foot { turn_id: t.clone() });
            }
        }
        k = end;
    }
    if let (Some(r), false) = (&running_id, seen_running) {
        rows.push(Row::Live { turn_id: r.clone() });
        rows.push(Row::Foot { turn_id: r.clone() });
    }
    rows
}

fn regex_lite(_: &str) -> impl Fn(&str) -> bool {
    // ^\[[ac]\d+\]
    |t: &str| {
        let b = t.as_bytes();
        if b.len() < 4 || b[0] != b'[' || !(b[1] == b'a' || b[1] == b'c') {
            return false;
        }
        let mut i = 2;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        i > 2 && i + 1 < b.len() && b[i] == b']' && b[i + 1] == b' '
    }
}

impl Render for Feed {
    fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity().downgrade();
        let k = c(cx).clone();
        div().size_full().bg(k.bg).child(
            list(self.list.clone(), move |ix, window, cx| {
                let Some(feed) = this.upgrade() else { return div().into_any_element() };
                feed.update(cx, |f, cx| f.render_row(ix, window, cx))
            })
            .size_full(),
        )
    }
}

const MAXW: f32 = 860.0;

impl Feed {
    fn wrap(&self, el: impl IntoElement) -> AnyElement {
        div().w_full().flex().justify_center().px(px(26.)).child(div().w_full().max_w(px(MAXW)).child(el)).into_any_element()
    }

    fn render_row(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(row) = self.rows.get(ix).cloned() else { return div().into_any_element() };
        let first = ix == 0;
        let last = ix + 1 == self.rows.len();
        let el = match row {
            Row::Skeleton => self.skeleton(cx),
            Row::Empty => self.empty(cx),
            Row::Divider => self.divider(cx),
            Row::User(e) => self.user(&e, window, cx),
            Row::Image(e) => self.image_msg(&e, cx),
            Row::Parent(e) => self.parent(&e, window, cx),
            Row::Report(e) => self.report(&e, window, cx),
            Row::Work(e) => self.work(&e, window, cx),
            Row::Talk(e) => self.talk(&e, window, cx),
            Row::Tool { e, echo, images } => self.tool(&e, echo.as_ref(), &images, cx),
            Row::Live { turn_id } => self.live(&turn_id, window, cx),
            Row::Foot { turn_id } => self.foot(&turn_id, cx),
        };
        let mut d = div().w_full().child(self.wrap(el));
        if first {
            d = d.pt(px(26.));
        }
        if last {
            d = d.pb(px(30.)).child(probe("feed.last"));
        }
        d.into_any_element()
    }

    fn skeleton(&self, cx: &App) -> AnyElement {
        let k = c(cx);
        let mut d = div().flex().flex_col().gap(px(24.));
        for w in [260.0, 180.0, 320.0] {
            d = d.child(
                div()
                    .flex()
                    .flex_col()
                    .child(div().flex().justify_end().mb(px(14.)).child(div().w(px(w)).h(px(40.)).rounded(px(14.)).bg(k.panel2)))
                    .child(skeleton_line(None, cx))
                    .child(skeleton_line(None, cx))
                    .child(skeleton_line(Some(220.), cx)),
            );
        }
        d.into_any_element()
    }

    fn empty(&self, cx: &App) -> AnyElement {
        let k = c(cx);
        let s = self.state.read(cx);
        let t = s.tasks.get(&s.current).map(|x| x.task.clone());
        match t {
            Some(t) if t.parent.is_some() => div()
                .mt(px(120.))
                .flex()
                .flex_col()
                .items_center()
                .gap(px(8.))
                .text_color(k.muted)
                .child(div().text_size(px(24.)).text_color(k.text).font_weight(FontWeight::SEMIBOLD).child(t.title.clone()))
                .child(format!("This task has no messages yet.{}", if t.status == TaskStatus::Active { " Its first turn is starting." } else { "" }))
                .into_any_element(),
            _ => div()
                .mt(px(110.))
                .flex()
                .flex_col()
                .items_center()
                .gap(px(10.))
                .text_color(k.muted)
                .max_w(px(520.))
                .mx_auto()
                .text_center()
                .child(logo(44.0, cx))
                .child(div().text_size(px(26.)).text_color(k.text).font_weight(FontWeight::SEMIBOLD).child("One chat. It never ends."))
                .child("Every message is kept forever and folded into a tree of summaries. Each turn starts fresh with a ~128 KB view of the whole history, and the agent zooms in when it needs detail. Work can branch into tasks: each one a chat of its own.")
                .child(div().text_size(px(12.)).child("Running on your local Claude Code and Codex logins."))
                .into_any_element(),
        }
    }

    fn divider(&self, cx: &App) -> AnyElement {
        let k = c(cx);
        div()
            .flex()
            .items_center()
            .gap(px(10.))
            .my(px(12.))
            .text_size(px(11.))
            .text_color(k.faint)
            .child(div().flex_1().h(px(1.)).bg(k.line2))
            .child("NOT RUN YET")
            .child(div().flex_1().h(px(1.)).bg(k.line2))
            .into_any_element()
    }

    fn cut_text<'a>(&self, e: &'a LogEntry) -> (std::borrow::Cow<'a, str>, Option<usize>) {
        // long texts are shown shortened until "show all" (the web's slim copy); the full text is in memory
        let max = if e.kind == LogKind::Tool || e.kind == LogKind::Echo { 2000 } else { 8000 };
        if self.full.contains(&e.i) || e.text.chars().count() <= max {
            return (std::borrow::Cow::Borrowed(&e.text), None);
        }
        let t = if e.kind == LogKind::Tool { optchat_engine::trim::trim_tool_text(&e.text, max).0 } else { optchat_engine::trim::head_tail(&e.text, max) };
        (std::borrow::Cow::Owned(t), Some(js_len(&e.text)))
    }
    fn cut_button(&self, e: &LogEntry, n: Option<usize>, cx: &mut Context<Self>) -> Option<AnyElement> {
        let n = n?;
        let i = e.i;
        Some(
            btn_small(ElementId::Name(format!("cut{i}").into()), format!("shortened · show all {} chars", group_thousands(n)), Tone::Ghost, cx)
                .mt(px(4.))
                .on_click(cx.listener(move |f, _, _, cx| {
                    f.full.insert(i);
                    f.md.remove(&i);
                    f.rev = u64::MAX;
                    if let Some(ix) = f.rows.iter().position(|r| matches!(r, Row::Talk(e) | Row::User(e) | Row::Parent(e) | Row::Report(e) if e.i == i) || matches!(r, Row::Tool { e, echo, .. } if e.i == i || echo.as_ref().map(|x| x.i) == Some(i))) {
                        f.list.splice(ix..ix + 1, 1);
                    }
                    cx.notify();
                }))
                .into_any_element(),
        )
    }

    fn meta_line(&self, e: &LogEntry, cx: &App) -> Div {
        let k = c(cx);
        div().flex().justify_end().gap(px(8.)).mt(px(4.)).text_size(px(10.5)).font_family("Menlo").text_color(k.faint).child(format!("#{}", e.i)).child(fmt::time(&e.date))
    }

    fn user(&mut self, e: &Arc<LogEntry>, _w: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let k = c(cx).clone();
        let (text, n) = self.cut_text(e);
        let copy = e.text.clone();
        let text = text.to_string();
        div()
            .flex()
            .justify_end()
            .my(px(10.))
            .child(
                div()
                    .max_w(relative(0.78))
                    .flex()
                    .flex_col()
                    .items_end()
                    .child(
                        div()
                            .id(ElementId::Name(format!("ub{}", e.i).into()))
                            .bg(k.panel2)
                            .border_1()
                            .border_color(k.line2)
                            .rounded(px(16.))
                            .rounded_br(px(4.))
                            .px(px(14.))
                            .py(px(10.))
                            .text_size(px(14.))
                            .line_height(px(21.5))
                            .text_color(k.text)
                            .child(text)
                            .context_menu(move |m, _, _| m.item(PopupMenuItem::new("Copy text").on_click({ let c2 = copy.clone(); move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(c2.clone())) }))),
                    )
                    .children(self.cut_button(e, n, cx))
                    .child(self.meta_line(e, cx)),
            )
            .into_any_element()
    }

    fn blob_img(&mut self, r: &ImageRef, max_w: f32, max_h: f32, cx: &mut Context<Self>) -> AnyElement {
        let k = c(cx).clone();
        let blob = r.blob.clone();
        let (w, h) = (r.w.unwrap_or(400) as f32, r.h.unwrap_or(300) as f32);
        let scale = (max_w / w).min(max_h / h).min(1.0);
        let (dw, dh) = (w * scale, h * scale);
        let img_el: AnyElement = match self.image(&blob, cx) {
            Some(i) => img(i).w(px(dw)).h(px(dh)).rounded(px(12.)).into_any_element(),
            None => div().w(px(dw)).h(px(dh)).rounded(px(12.)).bg(k.panel2).into_any_element(),
        };
        div()
            .id(ElementId::Name(format!("img{blob}{max_w}").into()))
            .rounded(px(12.))
            .border_1()
            .border_color(k.line2)
            .overflow_hidden()
            .cursor_pointer()
            .child(img_el)
            .on_click(cx.listener(move |_, _, _, cx| cx.emit(FeedEvent::Lightbox(blob.clone()))))
            .into_any_element()
    }

    fn image_msg(&mut self, e: &Arc<LogEntry>, cx: &mut Context<Self>) -> AnyElement {
        let user = e.kind == LogKind::User;
        let im = self.blob_img(e.image.as_ref().unwrap(), 420., 360., cx);
        let k = c(cx);
        let meta = div().mt(px(4.)).text_size(px(10.5)).font_family("Menlo").text_color(k.faint).child(format!("#{} {}", e.i, e.text));
        if user {
            div().flex().justify_end().my(px(10.)).child(div().flex().flex_col().items_end().child(im).child(meta)).into_any_element()
        } else {
            div().my(px(8.)).flex().flex_col().child(im).child(meta).into_any_element()
        }
    }

    fn parent(&mut self, e: &Arc<LogEntry>, w: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let k = c(cx).clone();
        let s = self.state.read(cx);
        let t = s.tasks.get(&s.current).map(|x| x.task.clone());
        let from = t.as_ref().and_then(|t| t.parent).or(e.meta_i64("from"));
        let by_user = e.meta_str("by") == Some("user");
        let who = |f: i64| if f == 0 { "the main chat".to_string() } else { format!("#{f}") };
        let label = match from {
            None => "from the parent".to_string(),
            Some(f) if by_user => format!("from you, as {}", who(f)),
            Some(f) => format!("from {}", who(f)),
        };
        let body: AnyElement = match &e.image {
            Some(r) => self.blob_img(r, 420., 360., cx),
            None => {
                let (txt, n) = self.cut_text(e);
                let doc = if n.is_some() { markdown::parse(&txt) } else { self.doc(e) };
                let mut d = div().child(markdown::render(&format!("p{}", e.i), &doc, &markdown::Opts { size: 14.0, color: k.text }, w, cx));
                if let Some(b) = self.cut_button(e, n, cx) {
                    d = d.child(b);
                }
                d.into_any_element()
            }
        };
        div()
            .my(px(10.))
            .border_l(px(3.))
            .border_color(k.violet)
            .bg(k.violet.opacity(0.06))
            .rounded_r(px(12.))
            .rounded_l(px(4.))
            .px(px(12.))
            .pt(px(8.))
            .pb(px(10.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .text_size(px(12.))
                    .mb(px(4.))
                    .child(
                        div()
                            .id(ElementId::Name(format!("pfrom{}", e.i).into()))
                            .text_color(k.violet)
                            .font_weight(FontWeight::MEDIUM)
                            .cursor_pointer()
                            .child(format!("↳ {label}"))
                            .when_some(from, |d, f| d.on_click(cx.listener(move |_, _, _, cx| cx.emit(FeedEvent::OpenTask(f))))),
                    )
                    .child(div().ml_auto().font_family("Menlo").text_size(px(10.5)).text_color(k.faint).child(format!("#{} · {}", e.i, fmt::time(&e.date)))),
            )
            .child(body)
            .into_any_element()
    }

    fn report(&mut self, e: &Arc<LogEntry>, w: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let k = c(cx).clone();
        let (id, kind, body) = parse_report(e);
        let s = self.state.read(cx);
        let child = s.tasks.get(&id).map(|x| task_name(&x.task)).unwrap_or_default();
        if let Some(r) = &e.image {
            let im = self.blob_img(r, 240., 200., cx);
            return div()
                .my(px(8.))
                .flex()
                .gap(px(8.))
                .px(px(12.))
                .py(px(8.))
                .border_1()
                .border_color(k.line2)
                .rounded(px(12.))
                .child(div().font_family("Menlo").text_color(k.violet).font_weight(FontWeight::BOLD).child(format!("#{id}")))
                .child(im)
                .into_any_element();
        }
        let key = format!("r{}", e.i);
        let open = self.open.contains(&key);
        let long = body.chars().count() > 320;
        let shown: String = if open || !long { body.clone() } else { format!("{}…", body.chars().take(320).collect::<String>()) };
        let (border, bg, kcol, klabel) = match kind.as_str() {
            "talk" => (k.green.opacity(0.28), k.panel, k.green, "replied"),
            "error" => (k.red.opacity(0.4), k.red.opacity(0.05), k.red, "error"),
            "status" => (k.line2, k.bg2, k.amber, "status"),
            "user" => (k.line2, k.bg2, k.muted, "you wrote to it"),
            _ => (k.line2, k.panel, k.muted, "report"),
        };
        let body_el: AnyElement = if kind == "talk" {
            let doc = markdown::parse(&shown);
            markdown::render(&format!("rb{}", e.i), &doc, &markdown::Opts { size: 13.5, color: k.text2 }, w, cx).into_any_element()
        } else {
            div().text_size(px(13.5)).line_height(px(21.)).text_color(k.text2).child(shown).into_any_element()
        };
        let hover = k.hover;
        div()
            .my(px(8.))
            .border_1()
            .border_color(border)
            .rounded(px(12.))
            .bg(bg)
            .overflow_hidden()
            .child(
                div()
                    .id(ElementId::Name(format!("rh{}", e.i).into()))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .px(px(12.))
                    .py(px(7.))
                    .text_size(px(12.5))
                    .border_b_1()
                    .border_color(k.line)
                    .cursor_pointer()
                    .hover(move |s| s.bg(hover))
                    .on_click(cx.listener(move |_, _, _, cx| cx.emit(FeedEvent::OpenTask(id))))
                    .child(div().font_family("Menlo").font_weight(FontWeight::BOLD).text_color(k.violet).child(format!("#{id}")))
                    .child(div().text_color(kcol).child(klabel))
                    .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().text_color(k.text2).child(child))
                    .child(div().font_family("Menlo").text_size(px(10.5)).text_color(k.faint).child(format!("#{} · {}", e.i, fmt::time(&e.date))))
                    .child(div().text_size(px(11.)).text_color(k.accent).child("open ↗")),
            )
            .child(
                div()
                    .id(ElementId::Name(format!("rbody{}", e.i).into()))
                    .px(px(12.))
                    .pt(px(8.))
                    .pb(px(10.))
                    .when(long, |d| d.cursor_pointer().on_click(cx.listener(move |f, _, _, cx| f.toggle(&format!("r{}", e_i(&key)), cx))))
                    .child(body_el),
            )
            .into_any_element()
    }

    fn work(&mut self, e: &Arc<LogEntry>, w: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let k = c(cx).clone();
        let parts: Vec<(String, String)> = split_work(&e.text);
        let mut d = div()
            .my(px(10.))
            .border_1()
            .border_color(k.line2)
            .rounded(px(12.))
            .bg(k.panel)
            .p(px(12.))
            .child(
                div()
                    .flex()
                    .text_size(px(12.))
                    .text_color(k.amber)
                    .mb(px(6.))
                    .child(if parts.len() == 1 { "⭑ report from a background task".to_string() } else { format!("⭑ {} reports from background tasks", parts.len()) })
                    .child(div().flex_1())
                    .child(div().font_family("Menlo").text_size(px(10.5)).text_color(k.faint).child(format!("#{} · {}", e.i, fmt::time(&e.date)))),
            );
        for (n, (id, text)) in parts.into_iter().enumerate() {
            let tid: Option<i64> = id.strip_prefix('a').and_then(|x| x.parse().ok());
            let doc = markdown::parse(&text);
            d = d.child(
                div()
                    .mt(px(6.))
                    .when(!id.is_empty(), |d| {
                        d.child(
                            div()
                                .id(ElementId::Name(format!("w{}.{n}", e.i).into()))
                                .text_size(px(12.))
                                .font_family("Menlo")
                                .text_color(k.violet)
                                .cursor_pointer()
                                .child(match tid {
                                    Some(t) => format!("{id} → #{t}"),
                                    None => id.clone(),
                                })
                                .when_some(tid, |d, t| d.on_click(cx.listener(move |_, _, _, cx| cx.emit(FeedEvent::OpenTask(t))))),
                        )
                    })
                    .child(markdown::render(&format!("w{}.{n}", e.i), &doc, &markdown::Opts { size: 13.5, color: k.text }, w, cx)),
            );
        }
        d.into_any_element()
    }

    fn talk(&mut self, e: &Arc<LogEntry>, w: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let k = c(cx).clone();
        let (txt, n) = self.cut_text(e);
        let doc = if n.is_some() { markdown::parse(&txt) } else { self.doc(e) };
        let copy = e.text.clone();
        div()
            .id(ElementId::Name(format!("talk{}", e.i).into()))
            .my(px(8.))
            .child(markdown::render(&format!("t{}", e.i), &doc, &markdown::Opts { size: 14.0, color: k.text }, w, cx))
            .children(self.cut_button(e, n, cx))
            .context_menu(move |m, _, _| {
                m.item(PopupMenuItem::new("Copy markdown").on_click({
                    let c2 = copy.clone();
                    move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(c2.clone()))
                }))
            })
            .into_any_element()
    }

    fn tool(&mut self, e: &Arc<LogEntry>, echo: Option<&Arc<LogEntry>>, images: &[Arc<LogEntry>], cx: &mut Context<Self>) -> AnyElement {
        let k = c(cx).clone();
        let key = format!("t{}", e.i);
        let open = self.open.contains(&key);
        let name = e.meta_str("name").map(String::from).unwrap_or_else(|| e.text.split(' ').next().unwrap_or("").to_string());
        let cid = e.meta_str("callId").unwrap_or("").to_string();
        let st: Option<ToolState> = self.state.read(cx).cur.tools.get(&cid).cloned();
        let input: serde_json::Value = match &st {
            Some(s) => s.input.clone(),
            None if e.kind == LogKind::Tool => {
                let rest = e.text.splitn(2, ' ').nth(1).unwrap_or("");
                serde_json::from_str(rest).unwrap_or(serde_json::Value::String(e.text.clone()))
            }
            None => serde_json::Value::Null,
        };
        let state = match (echo, &st) {
            (Some(x), _) => if x.meta_bool("isError") { "err" } else { "ok" },
            (None, Some(s)) if s.running => "run",
            (None, Some(s)) => if s.result.as_ref().map(|r| r.is_error).unwrap_or(false) { "err" } else { "ok" },
            (None, None) => "run",
        };
        let (scol, stext) = match state {
            "ok" => (k.green, "done".to_string()),
            "err" => (k.red, "error".to_string()),
            _ => (k.amber, match st.as_ref().and_then(|s| s.started_at.as_ref()) {
                Some(t) => format!("running · {}", fmt::elapsed(t)),
                None => "running…".into(),
            }),
        };
        let can_stop = state == "run" && st.as_ref().map(|s| s.running).unwrap_or(false);
        let hover = k.panel;
        let state_e = self.state.clone();
        let cid2 = cid.clone();
        let mut card = div()
            .my(px(8.))
            .border_1()
            .border_color(k.line)
            .bg(k.bg2)
            .rounded(px(10.))
            .overflow_hidden()
            .child(
                div()
                    .id(ElementId::Name(format!("th{}", e.i).into()))
                    .flex()
                    .items_center()
                    .gap(px(9.))
                    .px(px(11.))
                    .py(px(7.))
                    .text_size(px(13.))
                    .cursor_pointer()
                    .hover(move |s| s.bg(hover))
                    .on_click(cx.listener({
                        let key = key.clone();
                        move |f, _, _, cx| f.toggle(&key, cx)
                    }))
                    .child(icon("chevron-right", 12., k.faint).when(open, |s| s.with_transformation(Transformation::rotate(percentage(0.25)))))
                    .child(div().font_family("Menlo").text_size(px(12.)).text_color(k.cyan).whitespace_nowrap().child(name.clone()))
                    .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().font_family("Menlo").text_size(px(12.)).text_color(k.text2).child(arg_preview(&name, &input)))
                    .child(div().text_size(px(11.)).whitespace_nowrap().text_color(scol).child(stext))
                    .when(can_stop, |d| {
                        d.child(btn_small(ElementId::Name(format!("stop{}", e.i).into()), "stop", Tone::Danger, cx).on_click(move |_, _, cx| {
                            cx.stop_propagation();
                            let c3 = cid2.clone();
                            state_e.update(cx, |s, cx| s.command(cx, false, move |h| { h.stop_tool(&c3); Ok(String::new()) }));
                        }))
                    })
                    .child(div().font_family("Menlo").text_size(px(10.5)).text_color(k.faint).child(format!("#{}{}", e.i, echo.map(|x| format!("–{}", x.i)).unwrap_or_default()))),
            );
        if name.starts_with("task_") && name != "task_list" {
            let mut ids: Vec<i64> = vec![];
            if let Some(i) = input.get("id").and_then(|v| v.as_i64()) {
                ids.push(i);
            }
            if let Some(x) = echo {
                let mut rest = x.text.as_str();
                while let Some(p) = rest.find('#') {
                    let digits: String = rest[p + 1..].chars().take_while(|c| c.is_ascii_digit()).collect();
                    if let Ok(n) = digits.parse::<i64>() {
                        if !ids.contains(&n) {
                            ids.push(n);
                        }
                    }
                    rest = &rest[p + 1..];
                }
            }
            if !ids.is_empty() {
                let mut chips = div().flex().flex_wrap().gap(px(6.)).px(px(10.)).pb(px(8.));
                for id in ids.into_iter().take(8) {
                    chips = chips.child(
                        chip(format!("#{id} ↗"), ChipTone::Violet, cx)
                            .id(ElementId::Name(format!("tc{}.{id}", e.i).into()))
                            .cursor_pointer()
                            .on_click(cx.listener(move |_, _, _, cx| cx.emit(FeedEvent::OpenTask(id)))),
                    );
                }
                card = card.child(chips);
            }
        }
        if open {
            let lbl = |t: &str| div().text_size(px(10.5)).text_color(k.muted).px(px(12.)).pt(px(8.)).child(t.to_uppercase());
            let pre = |id: String, t: String| {
                div()
                    .id(ElementId::Name(id.into()))
                    .max_h(px(420.))
                    .overflow_y_scroll()
                    .px(px(12.))
                    .py(px(8.))
                    .font_family("Menlo")
                    .text_size(px(12.))
                    .line_height(px(18.))
                    .text_color(k.text2)
                    .child(t)
            };
            let mut detail = div().border_t_1().border_color(k.line);
            if e.kind == LogKind::Tool {
                let txt = match &input {
                    serde_json::Value::String(s) => s.clone(),
                    v => serde_json::to_string_pretty(v).unwrap_or_default(),
                };
                let copy_in = txt.clone();
                detail = detail.child(lbl("input")).child(pre(format!("tin{}", e.i), txt).context_menu(move |m, _, _| m.item(PopupMenuItem::new("Copy input").on_click({ let c2 = copy_in.clone(); move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(c2.clone())) }))));
            }
            if let Some(x) = echo {
                let (txt, n) = self.cut_text(x);
                let copy_out = x.text.clone();
                detail = detail.child(lbl("output")).child(pre(format!("tout{}", e.i), txt.to_string()).context_menu(move |m, _, _| m.item(PopupMenuItem::new("Copy output").on_click({ let c2 = copy_out.clone(); move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(c2.clone())) }))));
                if let Some(b) = self.cut_button(x, n, cx) {
                    detail = detail.child(div().px(px(10.)).child(b));
                }
            } else if let Some(r) = st.as_ref().and_then(|s| s.result.clone()) {
                detail = detail.child(lbl("output")).child(pre(format!("tres{}", e.i), optchat_engine::adapters::result_text(&r.content)));
            }
            let mut ims = div().flex().flex_wrap().gap(px(8.)).p(px(10.));
            let mut any = false;
            for x in images {
                if let Some(r) = &x.image {
                    ims = ims.child(self.blob_img(r, 240., 200., cx));
                    any = true;
                }
            }
            if any {
                detail = detail.child(ims);
            }
            card = card.child(detail);
        }
        card.into_any_element()
    }

    fn live(&mut self, turn_id: &str, w: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let k = c(cx).clone();
        let s = self.state.read(cx);
        let turn = s.cur.turns.get(turn_id).cloned();
        let lives: Vec<(String, String, String)> = s
            .cur
            .live
            .iter()
            .filter(|(id, v)| v.turn_id == turn_id && s.cur.requests.get(*id).map(|r| r.status == "running").unwrap_or(true))
            .map(|(id, v)| (id.clone(), v.text.clone(), v.thinking.clone()))
            .collect();
        let tool_running = s.cur.tools.values().any(|t| t.running && t.turn_id == turn_id);
        let unbuilt = s.cur.view.as_ref().map(|v| v.parts.iter().filter(|p| !p.built).count()).unwrap_or(0);
        let busy = s.busy.len();
        let failed = s.failed.len();
        let mut d = div().flex().flex_col().gap(px(6.));
        let waiting_line = |text: String, color: Hsla, id: &str| {
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .my(px(8.))
                .text_size(px(13.))
                .text_color(k.muted)
                .child(pulse(ElementId::Name(format!("wl{id}").into()), dot(color, 7.)))
                .child(text)
        };
        for (rid, text, thinking) in &lives {
            if !thinking.is_empty() {
                let key = format!("th{rid}");
                let open = self.open.contains(&key);
                let last = thinking.lines().filter(|l| !l.trim().is_empty()).last().unwrap_or("").to_string();
                let live_now = text.is_empty();
                d = d.child(
                    div()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .id(ElementId::Name(format!("thh{rid}").into()))
                                .flex()
                                .items_center()
                                .gap(px(7.))
                                .text_size(px(13.))
                                .text_color(k.muted)
                                .cursor_pointer()
                                .on_click(cx.listener({
                                    let key = key.clone();
                                    move |f, _, _, cx| f.toggle(&key, cx)
                                }))
                                .child(icon("chevron-right", 11., k.faint).when(open, |s| s.with_transformation(Transformation::rotate(percentage(0.25)))))
                                .when(live_now, |d| d.child(pulse(ElementId::Name(format!("thp{rid}").into()), dot(k.accent, 7.))).child("thinking…"))
                                .when(!live_now, |d| d.child("thought"))
                                .when(!open, |d| d.child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().italic().child(last))),
                        )
                        .when(open, |dd| dd.child(div().mt(px(6.)).pl(px(12.)).border_l_2().border_color(k.line2).italic().text_size(px(13.)).line_height(px(20.)).text_color(k.muted).child(thinking.clone()))),
                );
            }
            if !text.is_empty() {
                let doc = match self.live_md.get(rid) {
                    Some((n, d)) if *n == text.len() => d.clone(),
                    _ => {
                        let d = markdown::parse(text);
                        self.live_md.insert(rid.clone(), (text.len(), d.clone()));
                        d
                    }
                };
                d = d.child(
                    div()
                        .child(markdown::render(&format!("lv{rid}"), &doc, &markdown::Opts { size: 14.0, color: k.text }, w, cx))
                        .child(pulse(ElementId::Name(format!("cur{rid}").into()), div().w(px(8.)).h(px(16.)).rounded(px(2.)).bg(k.accent))),
                );
            }
            if text.is_empty() && thinking.is_empty() {
                d = d.child(waiting_line("waiting for the model…".into(), k.accent, rid));
            }
        }
        if let Some(t) = &turn {
            if t.status == "waiting" {
                let mut s = format!("waiting for summaries: {unbuilt} left");
                if busy > 0 {
                    s.push_str(&format!(" ({busy} in progress)"));
                }
                if failed > 0 {
                    s.push_str(&format!(" · {failed} retrying"));
                }
                s.push_str(&format!(" · {}", fmt::elapsed(&t.started_at)));
                d = d.child(waiting_line(s, k.amber, "w"));
            } else if t.status == "running" && lives.is_empty() && !tool_running {
                d = d.child(waiting_line("waiting for the model…".into(), k.accent, "m"));
            }
        }
        d.into_any_element()
    }

    fn foot(&mut self, turn_id: &str, cx: &mut Context<Self>) -> AnyElement {
        let k = c(cx).clone();
        let s = self.state.read(cx);
        let Some(turn) = s.cur.turns.get(turn_id).cloned() else { return div().into_any_element() };
        let running = s.cur.running.as_ref().map(|r| r.id == turn.id).unwrap_or(false);
        let selected = s.shown_request();
        let mut reqs: Vec<RequestRecord> = s.cur.requests.values().filter(|r| r.turn_id.as_deref() == Some(turn_id)).cloned().collect();
        reqs.sort_by(|a, b| a.started_at.cmp(&b.started_at));
        let key = format!("foot{turn_id}");
        let open = self.open.contains(&key);
        let last = reqs.last().cloned();
        let switched = reqs.iter().any(|r| r.model != turn.model || (r.effort.is_some() && r.effort != turn.effort));
        let lm = last.as_ref().map(|r| r.model.clone()).unwrap_or(turn.model.clone());
        let le = last.as_ref().and_then(|r| r.effort.clone()).or(turn.effort.clone());
        let label = format!(
            "{}{}{}",
            turn.model,
            if lm != turn.model { format!(" → {lm}") } else { String::new() },
            match &turn.effort {
                Some(e) => format!(" · {e}{}", match &le { Some(l) if l != e => format!(" → {l}"), _ => String::new() }),
                None => String::new(),
            }
        );
        let u = &turn.usage;
        let mut row = div().flex().flex_wrap().items_center().gap(px(6.)).mt(px(10.)).text_size(px(11.5));
        row = row.child(chip(label, if turn.backend == Backend::ClaudeCode { ChipTone::Acc } else { ChipTone::Green }, cx).child(probe(format!("foot.model.{}", turn.id))));
        row = row.child(
            chip(format!("{} step{} {}", reqs.len(), if reqs.len() == 1 { "" } else { "s" }, if open { "▴" } else { "▾" }), ChipTone::Plain, cx)
                .id(ElementId::Name(format!("steps{turn_id}").into()))
                .cursor_pointer()
                .on_click(cx.listener({
                    let key = key.clone();
                    move |f, _, _, cx| f.toggle(&key, cx)
                })),
        );
        row = row.child(chip(format!("in {} ({}% cached)", fmt::tokens(u.input), fmt::cache_pct(Some(u))), ChipTone::Plain, cx));
        row = row.child(chip(format!("out {}", fmt::tokens(u.output)), ChipTone::Plain, cx));
        row = row.child(chip(fmt::cost(Some(&turn.cost)), if turn.cost.priced { ChipTone::Plain } else { ChipTone::Amber }, cx));
        if let Some(d) = &turn.done_at {
            row = row.child(chip(fmt::dur(Some(&turn.started_at), Some(d)), ChipTone::Plain, cx));
        }
        if running {
            row = row.child(chip("live", ChipTone::Amber, cx).child(pulse(ElementId::Name(format!("lv{turn_id}").into()), dot(k.amber, 6.))));
        }
        if turn.status == "cancelled" {
            row = row.child(chip("stopped", ChipTone::Red, cx));
        }
        if let Some(l) = &last {
            let rid = l.id.clone();
            row = row.child(chip("context ↗", ChipTone::Plain, cx).id(ElementId::Name(format!("ctx{turn_id}").into())).cursor_pointer().on_click(cx.listener(move |_, _, _, cx| cx.emit(FeedEvent::SelectRequest(rid.clone())))));
        }
        let mut d = div().flex().flex_col().mb(px(18.));
        if let Some(err) = &turn.error {
            d = d.child(div().text_size(px(13.)).text_color(k.red).my(px(6.)).child(format!("⚠ {err}")));
        }
        d = d.child(row);
        if open {
            let mut t = div().mt(px(8.)).flex().flex_col().text_size(px(12.)).border_1().border_color(k.line).rounded(px(8.)).overflow_hidden();
            let cols: Vec<&str> = if switched { vec!["#", "model", "stop", "in", "cached", "write", "out", "cost", "time"] } else { vec!["#", "stop", "in", "cached", "write", "out", "cost", "time"] };
            let cell = |s: String, right: bool, w: f32| {
                let d = div().w(px(w)).flex_none().px(px(6.)).py(px(3.)).overflow_hidden().whitespace_nowrap();
                if right { d.flex().justify_end().child(s) } else { d.child(s) }
            };
            let widths = |c: &str| match c {
                "#" => 26.,
                "model" => 170.,
                "stop" => 76.,
                _ => 64.,
            };
            let mut head = div().flex().bg(k.panel).text_color(k.muted);
            for col in &cols {
                head = head.child(cell(col.to_string(), !matches!(*col, "#" | "model" | "stop"), widths(col)));
            }
            t = t.child(head);
            for (n, r) in reqs.iter().enumerate() {
                let rid = r.id.clone();
                let sel = selected.as_deref() == Some(&r.id);
                let u = r.usage.clone().unwrap_or_default();
                let mut tr = div()
                    .id(ElementId::Name(format!("req{}", r.id).into()))
                    .flex()
                    .text_color(k.text2)
                    .cursor_pointer()
                    .when(sel, |d| d.bg(k.accent_bg))
                    .hover(|s| s.bg(k.hover))
                    .on_click(cx.listener(move |_, _, _, cx| cx.emit(FeedEvent::SelectRequest(rid.clone()))));
                for col in &cols {
                    let v = match *col {
                        "#" => (n + 1).to_string(),
                        "model" => format!("{}{}", r.model, r.effort.as_ref().map(|e| format!(" · {e}")).unwrap_or_default()),
                        "stop" => if r.status == "running" { "running".into() } else { r.stop.clone().unwrap_or_default() },
                        "in" => fmt::tokens(u.input),
                        "cached" => fmt::tokens(u.cache_read),
                        "write" => fmt::tokens(u.cache_write),
                        "out" => fmt::tokens(u.output),
                        "cost" => fmt::cost(r.cost.as_ref()),
                        _ => fmt::dur(Some(&r.started_at), r.done_at.as_deref()),
                    };
                    tr = tr.child(cell(v, !matches!(*col, "#" | "model" | "stop"), widths(col)));
                }
                t = t.child(tr);
            }
            d = d.child(t);
        }
        d.into_any_element()
    }
}

fn e_i(key: &str) -> String {
    key.trim_start_matches('r').to_string()
}

pub fn parse_report(e: &LogEntry) -> (i64, String, String) {
    // "#7 talk: …"
    let t = &e.text;
    if let Some(rest) = t.strip_prefix('#') {
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        let after = &rest[digits.len()..];
        for kind in ["talk", "user", "error", "status"] {
            let pre = format!(" {kind}: ");
            if after.starts_with(&pre) {
                return (digits.parse().unwrap_or(-1), kind.into(), after[pre.len()..].to_string());
            }
        }
    }
    (e.meta_i64("from").unwrap_or(-1), e.meta_str("report").unwrap_or("report").to_string(), t.clone())
}

fn split_work(text: &str) -> Vec<(String, String)> {
    let mut out = vec![];
    for part in text.split("\n\n[") {
        let p = if out.is_empty() { part.to_string() } else { format!("[{part}") };
        if let Some(rest) = p.strip_prefix('[') {
            if let Some(end) = rest.find("] ") {
                out.push((rest[..end].to_string(), rest[end + 2..].to_string()));
                continue;
            }
        }
        if let Some(last) = out.last_mut() {
            last.1.push_str("\n\n");
            last.1.push_str(&p);
        } else {
            out.push((String::new(), p));
        }
    }
    out
}

pub fn arg_preview(name: &str, input: &serde_json::Value) -> String {
    let s = |k: &str| input.get(k).map(|v| v.as_str().map(String::from).unwrap_or_else(|| v.to_string())).unwrap_or_default();
    let has = |k: &str| input.get(k).map(|v| !v.is_null()).unwrap_or(false);
    let undef = |k: &str| if has(k) { s(k) } else { "undefined".into() };
    match name {
        "bash" => s("command"),
        "computer" => if has("title") { s("title") } else { s("code") },
        "zoom" => format!("{}+{}{}", undef("id"), undef("n"), if has("task") { format!("@{}", s("task")) } else { String::new() }),
        "date" => format!("{}{}", undef("id"), if has("task") { format!("@{}", s("task")) } else { String::new() }),
        "task_spawn" => format!("{}: {}", undef("mode"), undef("title")),
        "task_tell" | "task_resume" => format!("#{}{}", undef("id"), if has("message") && !s("message").is_empty() { format!(": {}", s("message")) } else { String::new() }),
        "task_pause" | "task_cancel" => format!("#{}{}", undef("id"), if has("reason") && !s("reason").is_empty() { format!(": {}", s("reason")) } else { String::new() }),
        "task_close" => format!("#{}: {}", undef("id"), s("result")),
        "task_list" => if has("id") { format!("#{}", s("id")) } else { "children".into() },
        _ => {
            for k in ["path", "pattern", "url"] {
                if let Some(v) = input.get(k).and_then(|v| v.as_str()) {
                    return v.to_string();
                }
            }
            match input {
                serde_json::Value::String(t) => t.clone(),
                v => v.to_string(),
            }
        }
    }
}

fn group_thousands(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

pub fn logo(size: f32, cx: &App) -> Div {
    let k = c(cx);
    div()
        .size(px(size))
        .rounded(px(size * 0.3))
        .bg(linear_gradient(200., linear_color_stop(k.accent, 0.), linear_color_stop(k.violet, 1.)))
        .shadow(vec![BoxShadow { color: k.accent.opacity(0.35), offset: point(px(0.), px(0.)), blur_radius: px(18.), spread_radius: px(0.) }])
}

#[allow(dead_code)]
fn _unused(_: &C) {}
