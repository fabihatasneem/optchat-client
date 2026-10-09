//! Small building blocks used by every view: icons, buttons, chips, dots, spinners, probes (test hooks).
use crate::theme::{c, C};
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::tooltip::Tooltip;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Duration;

pub fn icon(name: &str, size: f32, color: Hsla) -> Svg {
    svg().path(SharedString::from(format!("icons/{name}.svg"))).size(px(size)).flex_none().text_color(color)
}

// ---- ambient animations (spinners, pulsing dots) ----
// A 60 fps animation redraws the whole window every frame (the platform draw dominates, ~25% CPU while a
// "summarizing" spinner shows). These are driven by the clock instead and redrawn by a 20 Hz ticker
// (Workspace::start_ticker) that notifies only the views that drew one in their last render.
pub mod anim {
    use std::cell::Cell;
    use std::sync::atomic::{AtomicU32, Ordering};
    pub const WORKSPACE: u32 = 1;
    pub const FEED: u32 = 2;
    pub const TASKS: u32 = 4;
    pub const PANEL: u32 = 8;
    static WANTED: AtomicU32 = AtomicU32::new(0);
    thread_local!(static AREA: Cell<u32> = const { Cell::new(WORKSPACE) });
    /// called at the start of a view's render: animations drawn from here on belong to that view
    pub fn area(a: u32) {
        AREA.with(|x| x.set(a));
    }
    pub(crate) fn want() {
        WANTED.fetch_or(AREA.with(|x| x.get()), Ordering::Relaxed);
    }
    pub fn take() -> u32 {
        WANTED.swap(0, Ordering::Relaxed)
    }
    static ACTIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
    pub fn set_window_active(v: bool) {
        ACTIVE.store(v, Ordering::Relaxed);
    }
    pub fn window_active() -> bool {
        ACTIVE.load(Ordering::Relaxed)
    }
    pub fn clock() -> f32 {
        static T0: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
        T0.get_or_init(std::time::Instant::now).elapsed().as_secs_f32()
    }
}

pub fn spinner(_id: impl Into<ElementId>, size: f32, color: Hsla) -> impl IntoElement {
    anim::want();
    // 12 steps of 30° per second, in step with the 12.5 Hz ticker
    let turn = ((anim::clock() * 12.0).floor() / 12.0).fract();
    icon("loader-circle", size, color).with_transformation(Transformation::rotate(percentage(turn)))
}

/// A soft pulse (opacity) for "live" dots.
pub fn pulse<E: IntoElement + Styled + 'static>(_id: impl Into<ElementId>, el: E) -> impl IntoElement {
    anim::want();
    let d = (anim::clock() / 1.2 * std::f32::consts::TAU).cos() * 0.5 + 0.5;
    el.opacity(0.35 + 0.65 * d)
}

pub fn dot(color: Hsla, size: f32) -> Div {
    div().size(px(size)).rounded_full().bg(color).flex_none()
}

/// The task status dot: idle grey, active pulsing accent, paused amber, canceled red, done green, pending hollow ring.
pub fn status_dot(id: impl Into<SharedString>, status: &str, size: f32, cx: &App) -> AnyElement {
    let k = c(cx);
    let id: SharedString = id.into();
    match status {
        "active" => pulse(ElementId::Name(format!("sd-{id}").into()), dot(k.accent, size).shadow(vec![BoxShadow { color: k.accent.opacity(0.6), offset: point(px(0.), px(0.)), blur_radius: px(6.), spread_radius: px(0.) }])).into_any_element(),
        "paused" => dot(k.amber, size).into_any_element(),
        "canceled" => dot(k.red, size).opacity(0.85).into_any_element(),
        "done" => dot(k.green, size).into_any_element(),
        "pending" => div().size(px(size)).rounded_full().border_1().border_color(k.muted).flex_none().into_any_element(),
        _ => dot(k.faint, size).into_any_element(),
    }
}

pub fn tooltip(text: impl Into<SharedString>) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let t: SharedString = text.into();
    move |w, cx| Tooltip::new(t.clone()).build(w, cx)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Plain,
    Primary,
    Danger,
    Ghost,
    On,
    Accent,
}

/// A text button (web `.btn`).
pub fn btn(id: impl Into<ElementId>, label: impl Into<SharedString>, tone: Tone, cx: &App) -> Stateful<Div> {
    let k = c(cx);
    let (bg, fg, border, hover) = match tone {
        Tone::Primary => (k.accent2, k.on_accent, k.accent2, k.accent),
        Tone::Danger => (k.panel, k.red, k.red.opacity(0.35), k.red_bg),
        Tone::Ghost => (transparent_black(), k.text2, transparent_black(), k.hover),
        Tone::On => (k.hover, k.text, k.line2, k.hover),
        Tone::Accent => (k.accent_bg, k.accent, k.accent.opacity(0.3), k.accent_bg),
        Tone::Plain => (k.panel, k.text2, k.line2, k.hover),
    };
    div()
        .id(id.into())
        .flex()
        .items_center()
        .gap(px(6.))
        .px(px(11.))
        .h(px(28.))
        .rounded(px(7.))
        .border_1()
        .border_color(border)
        .bg(bg)
        .text_color(fg)
        .text_size(px(13.))
        .whitespace_nowrap()
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .child(label.into())
}
pub fn btn_small(id: impl Into<ElementId>, label: impl Into<SharedString>, tone: Tone, cx: &App) -> Stateful<Div> {
    btn(id, label, tone, cx).h(px(24.)).px(px(8.)).text_size(px(12.))
}
pub fn disabled<E: Styled + InteractiveElement>(el: E, off: bool) -> E {
    if off { el.opacity(0.45).cursor_default() } else { el }
}

/// An icon-only button (web `.iconbtn`).
pub fn icon_btn(id: impl Into<ElementId>, name: &str, size: f32, cx: &App) -> Stateful<Div> {
    let k = c(cx);
    let hover = k.hover;
    let text = k.text;
    div()
        .id(id.into())
        .size(px(size + 14.))
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .rounded(px(7.))
        .text_color(k.muted)
        .cursor_pointer()
        .hover(move |s| s.bg(hover).text_color(text))
        .child(svg().path(SharedString::from(format!("icons/{name}.svg"))).size(px(size)).text_color(k.muted))
}
/// icon button whose icon follows the given color
pub fn icon_btn_c(id: impl Into<ElementId>, name: &str, size: f32, color: Hsla, cx: &App) -> Stateful<Div> {
    let k = c(cx);
    let hover = k.hover;
    div()
        .id(id.into())
        .size(px(size + 14.))
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .rounded(px(7.))
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .child(icon(name, size, color))
}

/// A chip (turn foot, tags).
pub fn chip(label: impl Into<SharedString>, tone: ChipTone, cx: &App) -> Div {
    let k = c(cx);
    let (fg, bg, border) = match tone {
        ChipTone::Plain => (k.muted, k.panel, k.line),
        ChipTone::Acc => (k.accent, k.accent_bg, k.accent.opacity(0.3)),
        ChipTone::Green => (k.green, k.green_bg, k.green.opacity(0.3)),
        ChipTone::Amber => (k.amber, k.amber_bg, k.amber.opacity(0.3)),
        ChipTone::Red => (k.red, k.red_bg, k.red.opacity(0.3)),
        ChipTone::Violet => (k.violet, k.violet.opacity(0.1), k.violet.opacity(0.3)),
    };
    div()
        .flex()
        .items_center()
        .gap(px(5.))
        .px(px(8.))
        .py(px(2.))
        .rounded(px(6.))
        .bg(bg)
        .border_1()
        .border_color(border)
        .text_color(fg)
        .text_size(px(11.5))
        .whitespace_nowrap()
        .child(label.into())
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ChipTone {
    Plain,
    Acc,
    Green,
    Amber,
    Red,
    Violet,
}

pub fn section_title(t: impl Into<SharedString>, cx: &App) -> Div {
    let k = c(cx);
    div().text_size(px(10.5)).text_color(k.muted).font_weight(FontWeight::SEMIBOLD).mb(px(8.)).child(t.into().to_uppercase())
}

pub fn card(cx: &App) -> Div {
    let k = c(cx);
    div().rounded(px(10.)).border_1().border_color(k.line).bg(k.panel).p(px(12.))
}

pub fn kv_row(k_: impl Into<SharedString>, v: impl Into<SharedString>, cx: &App) -> Div {
    let k = c(cx);
    div()
        .flex()
        .gap(px(10.))
        .text_size(px(12.5))
        .py(px(2.))
        .child(div().w(px(118.)).flex_none().text_color(k.muted).child(k_.into()))
        .child(div().flex_1().min_w_0().text_color(k.text2).child(v.into()))
}

pub fn mono(cx: &App) -> SharedString {
    let _ = cx;
    "Menlo".into()
}

pub fn skeleton_line(w: Option<f32>, cx: &App) -> Div {
    let k = c(cx);
    let d = div().h(px(12.)).rounded(px(6.)).bg(k.panel2).mb(px(10.));
    match w {
        Some(w) => d.w(px(w)),
        None => d.w_full(),
    }
}

pub fn colors(cx: &App) -> C {
    c(cx).clone()
}

// ---- probes: element bounds for UI tests (OPTCHAT_PROBES=<file>), written as JSON by a background thread ----
pub static PROBES: OnceLock<Mutex<HashMap<String, [f32; 4]>>> = OnceLock::new();
pub fn probes_on() -> bool {
    std::env::var("OPTCHAT_PROBES").is_ok()
}
/// An invisible absolute child that records its parent's bounds under `name`.
pub fn probe(name: impl Into<String>) -> impl IntoElement {
    let name = name.into();
    canvas(
        move |b, _w, _cx| {
            if probes_on() {
                PROBES.get_or_init(|| Mutex::new(HashMap::new())).lock().insert(name, [f32::from(b.origin.x), f32::from(b.origin.y), f32::from(b.size.width), f32::from(b.size.height)]);
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .size_full()
    .top_0()
    .left_0()
}
/// A JSON summary of the app state for UI tests (written next to the probes as `<file>.state.json`).
pub static STATE_JSON: OnceLock<Mutex<String>> = OnceLock::new();
pub fn set_state_json(s: String) {
    if probes_on() {
        *STATE_JSON.get_or_init(|| Mutex::new(String::new())).lock() = s;
    }
}
/// state published by views that render on their own (merged into the state file under its key)
pub static STATE_EXTRA: OnceLock<Mutex<serde_json::Map<String, serde_json::Value>>> = OnceLock::new();
pub fn set_state_extra(key: &str, v: serde_json::Value) {
    if probes_on() {
        STATE_EXTRA.get_or_init(Default::default).lock().insert(key.to_string(), v);
    }
}
pub fn start_probe_writer() {
    let Ok(path) = std::env::var("OPTCHAT_PROBES") else { return };
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(250));
        let m = PROBES.get_or_init(|| Mutex::new(HashMap::new())).lock().clone();
        if let Ok(s) = serde_json::to_string(&m) {
            let tmp = format!("{path}.tmp");
            if std::fs::write(&tmp, s).is_ok() {
                let _ = std::fs::rename(&tmp, &path);
            }
            let mut st = STATE_JSON.get_or_init(|| Mutex::new(String::new())).lock().clone();
            if let Ok(serde_json::Value::Object(mut o)) = serde_json::from_str::<serde_json::Value>(&st) {
                for (k, v) in STATE_EXTRA.get_or_init(Default::default).lock().iter() {
                    o.insert(k.clone(), v.clone());
                }
                st = serde_json::Value::Object(o).to_string();
            }
            let sp = format!("{path}.state.json");
            if std::fs::write(format!("{sp}.tmp"), st).is_ok() {
                let _ = std::fs::rename(format!("{sp}.tmp"), &sp);
            }
        }
    });
}
pub fn clear_probes(prefix: &str) {
    if let Some(m) = PROBES.get() {
        m.lock().retain(|k, _| !k.starts_with(prefix));
    }
}

pub fn when_some<T, E: FluentBuilder>(e: E, v: Option<T>, f: impl FnOnce(E, T) -> E) -> E {
    match v {
        Some(v) => f(e, v),
        None => e,
    }
}

// ---- frame timing (perf measurements): intervals between window paints while the feed scrolls ----------------------
pub struct FrameStats {
    pub last_paint: Option<std::time::Instant>,
    pub last_scroll: Option<std::time::Instant>,
    pub intervals_ms: Vec<f32>,
    pub render_ms: Vec<f32>,
    /// main-thread work per frame while scrolling: from the root render to the end of paint
    pub work_ms: Vec<f32>,
    pub frame_start: Option<std::time::Instant>,
}
/// called first thing in the root view's render
pub fn frame_begin() {
    frames().lock().frame_start = Some(std::time::Instant::now());
}
pub static FRAMES: OnceLock<Mutex<FrameStats>> = OnceLock::new();
pub fn frames() -> &'static Mutex<FrameStats> {
    FRAMES.get_or_init(|| Mutex::new(FrameStats { last_paint: None, last_scroll: None, intervals_ms: vec![], render_ms: vec![], work_ms: vec![], frame_start: None }))
}
/// A zero-size element whose paint records the time since the previous paint (only while scrolling).
pub fn frame_probe() -> impl IntoElement {
    canvas(
        |_, _, _| {},
        |_, _, _, _| {
            let now = std::time::Instant::now();
            let mut f = frames().lock();
            let scrolling = f.last_scroll.map(|t| now.duration_since(t).as_millis() < 120).unwrap_or(false);
            if let (Some(t0), true) = (f.frame_start.take(), scrolling) {
                let w = now.duration_since(t0).as_secs_f32() * 1000.0;
                f.work_ms.push(w);
            }
            if let (Some(prev), true) = (f.last_paint, scrolling) {
                let d = now.duration_since(prev).as_secs_f32() * 1000.0;
                if d < 1000.0 {
                    f.intervals_ms.push(d);
                }
            }
            f.last_paint = Some(now);
        },
    )
    .absolute()
    .size_0()
}
pub fn frame_summary() -> serde_json::Value {
    let f = frames().lock();
    let mut v = f.intervals_ms.clone();
    if v.is_empty() {
        return serde_json::json!(null);
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p = |q: f32| v[((v.len() as f32 - 1.0) * q) as usize];
    let mut r = f.render_ms.clone();
    r.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let rp = |q: f32| if r.is_empty() { 0.0 } else { r[((r.len() as f32 - 1.0) * q) as usize] };
    let mut w = f.work_ms.clone();
    w.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let wp = |q: f32| if w.is_empty() { 0.0 } else { w[((w.len() as f32 - 1.0) * q) as usize] };
    serde_json::json!({"workFrames": w.len(), "workP50": wp(0.5), "workP95": wp(0.95), "workP99": wp(0.99), "workMax": w.last().copied().unwrap_or(0.0), "workOver8": w.iter().filter(|x| **x > 8.0).count(), "workOver16": w.iter().filter(|x| **x > 16.7).count(), "frames": v.len(), "p50": p(0.5), "p95": p(0.95), "p99": p(0.99), "max": v[v.len()-1], "over16": v.iter().filter(|x| **x > 16.7).count(), "over33": v.iter().filter(|x| **x > 33.4).count(), "rowRenderP50": rp(0.5), "rowRenderP99": rp(0.99)})
}
pub fn reset_frames() {
    let mut f = frames().lock();
    f.intervals_ms.clear();
    f.render_ms.clear();
    f.work_ms.clear();
}

/// Resident memory of this process in MB (ps).
pub fn rss_mb() -> f64 {
    // proc_pidinfo(PROC_PIDTASKINFO): a syscall, no subprocess on the UI thread
    let mut info: libc::proc_taskinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<libc::proc_taskinfo>() as i32;
    let n = unsafe { libc::proc_pidinfo(std::process::id() as i32, libc::PROC_PIDTASKINFO, 0, &mut info as *mut _ as *mut libc::c_void, size) };
    if n == size { info.pti_resident_size as f64 / (1024.0 * 1024.0) } else { 0.0 }
}

/// The scroll container of a sidebar panel (inside its cached view).
pub fn scroll_body(id: &'static str, body: AnyElement) -> impl IntoElement {
    div().id(SharedString::from(format!("scroll-{id}"))).size_full().overflow_y_scroll().child(body)
}

// ---- chat zoom (⌘= / ⌘- / ⌘⇧0): the feed's sizes are multiplied by font_size / 14, like a browser's zoom ----
static ZOOM: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0x3f800000); // 1.0
pub fn set_zoom(font_size: f32) {
    ZOOM.store((font_size / 14.0).to_bits(), std::sync::atomic::Ordering::Relaxed);
}
pub fn zoom() -> f32 {
    f32::from_bits(ZOOM.load(std::sync::atomic::Ordering::Relaxed))
}
/// a feed size in points at the current zoom
pub fn zx(v: f32) -> Pixels {
    px(v * zoom())
}
/// renders of the workspace so far (tests check the state file is fresh)
pub fn frame_count() -> u64 {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    N.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1
}
