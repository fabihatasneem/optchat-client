//! Overlays: the new-task dialog, the command palette / task switcher, the metrics popover with budgets, the computer
//! use approval dialog, the image lightbox and toasts.
use crate::fmt;
use crate::state::*;
use crate::theme::{c, Themes};
use crate::ui::*;
use crate::workspace::*;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui::Context;
use gpui_component::button::Button;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::menu::{DropdownMenu, PopupMenuItem};
use gpui_component::Sizable;
use optchat_engine::hub::{prepare_image, Approval, ImageIn};
use optchat_engine::lifecycle::BudgetArgs;
use optchat_engine::metrics::metrics;
use optchat_engine::types::*;
use std::sync::Arc;

#[derive(Clone)]
pub enum Overlay {
    NewTask(Entity<NewTaskDialog>),
    Palette(Entity<Palette>),
    Metrics(Entity<MetricsPop>),
    Lightbox(String),
}

pub fn noop() -> Div {
    div()
}

pub fn cycle_tab(w: &mut Workspace, d: i64, window: &mut Window, cx: &mut Context<Workspace>) {
    let s = w.state.read(cx);
    let tabs = s.prefs.tabs.clone();
    if tabs.is_empty() {
        return;
    }
    let k = tabs.iter().position(|t| *t == s.current).unwrap_or(0) as i64;
    let n = tabs.len() as i64;
    let next = tabs[(((k + d) % n + n) % n) as usize];
    w.open_task(next, window, cx);
}

pub fn zoom(w: &mut Workspace, d: f32, cx: &mut Context<Workspace>) {
    w.state.update(cx, |s, _| {
        s.prefs.font_size = if d == 0.0 { 14.0 } else { (s.prefs.font_size + d).clamp(11.0, 20.0) };
        s.save_prefs();
    });
    let size = w.state.read(cx).prefs.font_size;
    gpui_component::Theme::global_mut(cx).font_size = px(size - 0.5);
    crate::ui::set_zoom(size);
    w.feed.update(cx, |f, cx| f.relayout(cx));
    cx.refresh_windows();
}

pub fn next_theme(w: &mut Workspace, cx: &mut Context<Workspace>) {
    let th = cx.global::<Themes>();
    let k = th.all.iter().position(|t| t.id == th.active).unwrap_or(0);
    let next = th.all[(k + 1) % th.all.len()].id.clone();
    set_theme(w, &next, false, cx);
}
pub fn set_theme(w: &mut Workspace, id: &str, follow: bool, cx: &mut Context<Workspace>) {
    crate::theme::set(cx, id, follow);
    let id = id.to_string();
    w.state.update(cx, |s, _| {
        s.prefs.theme = Some(id);
        s.prefs.follow_system = follow;
        s.save_prefs();
    });
    cx.notify();
}

fn overlay_bg(id: &str, cx: &App) -> Stateful<Div> {
    let k = c(cx);
    div().id(SharedString::from(id.to_string())).absolute().inset_0().bg(hsla(0., 0., 0., if k.dark { 0.5 } else { 0.25 })).flex().items_center().justify_center().occlude()
}
fn dialog_box(cx: &App, w: f32) -> Div {
    let k = c(cx);
    div()
        .w(px(w))
        .max_w(relative(0.94))
        .max_h(relative(0.92))
        .bg(k.panel)
        .border_1()
        .border_color(k.line2)
        .rounded(px(14.))
        .p(px(20.))
        .shadow(vec![BoxShadow { color: k.shadow, offset: point(px(0.), px(20.)), blur_radius: px(60.), spread_radius: px(0.) }])
}

pub fn render_overlay(w: &mut Workspace, o: Overlay, _window: &mut Window, cx: &mut Context<Workspace>) -> AnyElement {
    match o {
        Overlay::NewTask(d) => overlay_bg("ov-new", cx).on_click(cx.listener(|w, _, window, cx| w.close_overlay(window, cx))).child(div().id("ov-new-inner").on_click(|_, _, cx| cx.stop_propagation()).child(d)).into_any_element(),
        Overlay::Palette(p) => div()
            .id("ov-pal")
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .justify_center()
            .pt(px(80.))
            .bg(c(cx).shadow.opacity(0.25))
            .on_click(cx.listener(|w, _, window, cx| w.close_overlay(window, cx)))
            .child(div().id("ov-pal-inner").on_click(|_, _, cx| cx.stop_propagation()).child(p))
            .into_any_element(),
        Overlay::Metrics(m) => div()
            .id("ov-met")
            .absolute()
            .inset_0()
            .occlude()
            .on_click(cx.listener(|w, _, window, cx| w.close_overlay(window, cx)))
            .child(div().id("ov-met-inner").absolute().right(px(10.)).bottom(px(32.)).on_click(|_, _, cx| cx.stop_propagation()).child(m))
            .into_any_element(),
        Overlay::Lightbox(blob) => {
            let im = w.feed.read(cx).images.get(&blob).cloned().flatten();
            overlay_bg("ov-light", cx)
                .bg(hsla(0., 0., 0.02, 0.85))
                .cursor_pointer()
                .on_click(cx.listener(|w, _, window, cx| w.close_overlay(window, cx)))
                .when_some(im, |d, i| d.child(img(i).max_w(relative(0.94)).max_h(relative(0.92)).object_fit(ObjectFit::Contain).rounded(px(8.))))
                .child(probe("lightbox"))
                .into_any_element()
        }
    }
}

// ---- approval ---------------------------------------------------------------------------------------------------
pub fn approval(w: &mut Workspace, a: Approval, cx: &mut Context<Workspace>) -> AnyElement {
    let k = c(cx).clone();
    let params = a.meta.get("tool_params").map(|p| p.to_string()).unwrap_or_default();
    let mk = |id: &'static str, label: &'static str, tone: Tone, accept: bool, persist: Option<&'static str>, cx: &mut Context<Workspace>| {
        let aid = a.id.clone();
        let st = w.state.clone();
        btn(id, label, tone, cx).child(probe(format!("approve.{id}"))).on_click(move |_, _, cx| {
            let aid = aid.clone();
            st.update(cx, |s, cx| s.command(cx, false, move |h| { h.approve(&aid, accept, persist.map(String::from)); Ok(String::new()) }))
        })
    };
    let b1 = mk("deny", "Deny", Tone::Plain, false, None, cx);
    let b2 = mk("once", "Allow once", Tone::Plain, true, None, cx);
    let b3 = mk("session", "Allow this session", Tone::Plain, true, Some("session"), cx);
    let b4 = mk("always", "Always allow", Tone::Primary, true, Some("always"), cx);
    overlay_bg("ov-approve", cx)
        .child(
            dialog_box(cx, 520.)
                .child(probe("approval"))
                .child(div().text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).mb(px(8.)).child(format!("Computer use{}", a.task.map(|t| format!(" · task #{t}")).unwrap_or_default())))
                .child(div().text_color(k.text2).line_height(px(21.)).mb(px(14.)).child(a.message.clone()))
                .when(!params.is_empty(), |d| d.child(div().mb(px(14.)).p(px(10.)).rounded(px(8.)).bg(k.code_bg).font_family("Menlo").text_size(px(12.)).text_color(k.text2).child(params)))
                .child(div().flex().justify_end().gap(px(8.)).child(b1).child(b2).child(b3).child(b4)),
        )
        .into_any_element()
}

// ---- toast ------------------------------------------------------------------------------------------------------------
pub fn toast(w: &mut Workspace, cx: &mut Context<Workspace>) -> Option<AnyElement> {
    let k = c(cx).clone();
    let (ok, msg, at) = w.state.read(cx).notice.clone()?;
    let ttl = if ok { 2600 } else { 6000 };
    let age = at.elapsed().as_millis() as u64;
    if age > ttl {
        return None;
    }
    // re-render when it should disappear
    cx.spawn(async move |this, cx| {
        Timer::after(std::time::Duration::from_millis(ttl - age + 20)).await;
        let _ = this.update(cx, |_, cx| cx.notify());
    })
    .detach();
    Some(
        div()
            .absolute()
            .bottom(px(42.))
            .left_0()
            .right_0()
            .flex()
            .justify_center()
            .child(
                div()
                    .px(px(14.))
                    .py(px(8.))
                    .rounded(px(10.))
                    .bg(k.panel2)
                    .border_1()
                    .border_color(if ok { k.line2 } else { k.red.opacity(0.4) })
                    .text_color(if ok { k.text } else { k.red })
                    .text_size(px(13.))
                    .shadow_lg()
                    .child(msg)
                    .child(probe("toast"))
                    .with_animation("toast-in", Animation::new(std::time::Duration::from_millis(160)), |d, t| d.opacity(t).mt(px(8. * (1. - t)))),
            )
            .into_any_element(),
    )
}

// ---- metrics popover ------------------------------------------------------------------------------------------------
pub struct MetricsPop {
    state: Entity<AppState>,
    usd: Entity<InputState>,
    min: Entity<InputState>,
    tok: Entity<InputState>,
    dl: Entity<InputState>,
    task: i64,
    _subs: Vec<Subscription>,
}
pub fn open_metrics(w: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    let state = w.state.clone();
    let info = state.read(cx).info().cloned();
    let Some(info) = info else { return };
    let b = info.task.budget.clone();
    let mk = |v: Option<String>, ph: &str, window: &mut Window, cx: &mut Context<MetricsPop>| {
        let ph = ph.to_string();
        cx.new(|cx| {
            let mut s = InputState::new(window, cx).placeholder(ph);
            if let Some(v) = v {
                s = s.default_value(v);
            }
            s
        })
    };
    let dl_ph = b.deadline.as_ref().map(|d| fmt::time(d)).unwrap_or("min from now".into());
    let pop = cx.new(|cx| {
        let usd = mk(b.usd.map(|v| v.to_string()), "dollars", window, cx);
        let min = mk(b.minutes.map(|v| ((v * 100.0).round() / 100.0).to_string()), "minutes", window, cx);
        let tok = mk(b.tokens.map(|v| (v as i64).to_string()), "tokens", window, cx);
        let dl = mk(None, &dl_ph, window, cx);
        let mut subs = vec![];
        for i in [&usd, &min, &tok, &dl] {
            subs.push(cx.subscribe_in(i, window, |p: &mut MetricsPop, _, e: &InputEvent, _, cx| {
                if let InputEvent::PressEnter { .. } = e {
                    p.save(false, cx);
                }
            }));
        }
        usd.update(cx, |i, cx| i.focus(window, cx));
        MetricsPop { state: state.clone(), usd, min, tok, dl, task: info.task.id, _subs: subs }
    });
    cx.subscribe_in(&pop, window, |w: &mut Workspace, _, _: &DismissEvent, window, cx| w.close_overlay(window, cx)).detach();
    w.overlay = Some(Overlay::Metrics(pop));
    cx.notify();
}
impl MetricsPop {
    fn save(&mut self, clear: bool, cx: &mut Context<Self>) {
        let num = |e: &Entity<InputState>, cx: &App| e.read(cx).value().trim().parse::<f64>().ok();
        let task = self.task;
        let cur = self.state.read(cx).tasks.get(&task).map(|t| t.task.budget.clone()).unwrap_or_default();
        let b = if clear {
            BudgetArgs::default()
        } else {
            let dm = num(&self.dl, cx);
            BudgetArgs { usd: num(&self.usd, cx), minutes: num(&self.min, cx), tokens: num(&self.tok, cx), deadline_minutes: dm, deadline: if dm.is_none() { cur.deadline.clone() } else { None } }
        };
        self.state.update(cx, |s, cx| s.command(cx, true, move |h| h.set_budget(task, b)));
        cx.emit(DismissEvent);
    }
}
impl EventEmitter<DismissEvent> for MetricsPop {}
impl Render for MetricsPop {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let k = c(cx).clone();
        let s = self.state.read(cx);
        let Some(info) = s.tasks.get(&self.task).cloned() else { return div() };
        let t = info.task.clone();
        let fmt_m = |m: &str, v: i64| if m == "usd" { fmt::usd(v as f64 / 1e6) } else if m.starts_with("time.") || m == "audio.ms" { fmt::ms(v) } else { fmt::tokens(v) };
        let mut table = div().flex().flex_col().text_size(px(12.));
        let mut any = false;
        for m in metrics() {
            let (o, sb) = (own(Some(&info), &m.id), sub(Some(&info), &m.id));
            if o == 0 && sb == 0 {
                continue;
            }
            any = true;
            table = table.child(
                div()
                    .flex()
                    .py(px(3.))
                    .child(div().flex_1().text_color(k.text2).child(m.id.clone()))
                    .child(div().w(px(130.)).flex().justify_end().text_color(k.muted).child(fmt_m(&m.id, o)))
                    .child(div().w(px(130.)).flex().justify_end().font_weight(FontWeight::SEMIBOLD).child(fmt_m(&m.id, sb))),
            );
            let (sc, sm) = (sub_k(Some(&info), &m.id, "chat"), sub_k(Some(&info), &m.id, "compact"));
            if sc > 0 && sm > 0 {
                table = table.child(
                    div()
                        .flex()
                        .pb(px(3.))
                        .text_size(px(11.))
                        .text_color(k.muted)
                        .child(div().flex_1().pl(px(10.)).child("chat · memory"))
                        .child(div().w(px(130.)).flex().justify_end().child(format!("{} · {}", fmt_m(&m.id, own_k(Some(&info), &m.id, "chat")), fmt_m(&m.id, own_k(Some(&info), &m.id, "compact")))))
                        .child(div().w(px(130.)).flex().justify_end().child(format!("{} · {}", fmt_m(&m.id, sc), fmt_m(&m.id, sm)))),
                );
            }
        }
        if !any {
            table = table.child(div().text_color(k.muted).child("nothing yet"));
        }
        let field = |label: &str, i: &Entity<InputState>| div().flex_1().flex().flex_col().gap(px(3.)).text_size(px(11.)).text_color(k.muted).child(label.to_string()).child(Input::new(i).small());
        let has_budget = t.budget.usd.is_some() || t.budget.minutes.is_some() || t.budget.tokens.is_some() || t.budget.deadline.is_some();
        div()
            .w(px(520.))
            .bg(k.panel)
            .border_1()
            .border_color(k.line2)
            .rounded(px(12.))
            .p(px(14.))
            .shadow_lg()
            .child(probe("metrics"))
            .child(div().flex().items_center().mb(px(8.)).child(div().font_weight(FontWeight::SEMIBOLD).child(format!("{} · metrics", if t.parent.is_none() { "Main chat".to_string() } else { format!("#{}", t.id) }))).child(div().ml(px(6.)).text_size(px(11.)).text_color(k.muted).child("(own · with subtree)")))
            .child(table)
            .when(!t.status.terminal(), |d| {
                d.child(div().mt(px(12.)).mb(px(6.)).font_weight(FontWeight::SEMIBOLD).child("Budget").child(div().text_size(px(11.)).text_color(k.muted).font_weight(FontWeight::NORMAL).child("limits on it and its subtree")))
                    .child(div().flex().gap(px(8.)).child(field("$", &self.usd)).child(field("agent-min", &self.min)).child(field("tokens", &self.tok)).child(field("deadline", &self.dl)))
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap(px(8.))
                            .mt(px(10.))
                            .when(has_budget, |d| d.child(btn_small("b-clear", "Clear", Tone::Ghost, cx).on_click(cx.listener(|p, _, _, cx| p.save(true, cx)))))
                            .child(btn_small("b-set", "Set budget", Tone::Primary, cx).child(probe("metrics.set")).on_click(cx.listener(|p, _, _, cx| p.save(false, cx)))),
                    )
                    .child(div().mt(px(8.)).text_size(px(11.)).line_height(px(16.)).text_color(k.muted).child("Past any limit a task pauses with its subtree, and its parent hears which limit. Agent-time adds up across children working in parallel."))
            })
    }
}

// ---- new task dialog ------------------------------------------------------------------------------------------------
pub struct NewTaskDialog {
    state: Entity<AppState>,
    parent: i64,
    mode: String,
    prev: Option<i64>,
    title: Entity<InputState>,
    instruction: Entity<InputState>,
    usd: Entity<InputState>,
    minutes: Entity<InputState>,
    model: String,
    images: Vec<crate::composer::Attachment>,
    focus: FocusHandle,
}
impl EventEmitter<DismissEvent> for NewTaskDialog {}
actions!(newtask, [SubmitStart]);

pub fn open_new_task(w: &mut Workspace, preset: Option<(String, Option<i64>, i64)>, window: &mut Window, cx: &mut Context<Workspace>) {
    let s = w.state.read(cx);
    let parent = preset.as_ref().map(|p| p.2).unwrap_or(s.current);
    let Some(p) = s.tasks.get(&parent).cloned() else { return };
    if p.task.status.terminal() {
        return;
    }
    let state = w.state.clone();
    let d = cx.new(|cx| {
        let title = cx.new(|cx| InputState::new(window, cx).placeholder("e.g. Add image upload to the composer"));
        let instruction = cx.new(|cx| InputState::new(window, cx).multi_line(true).rows(5).placeholder("What to do. For handoff and blind: everything it needs to know."));
        let usd = cx.new(|cx| InputState::new(window, cx).placeholder("$ budget"));
        let minutes = cx.new(|cx| InputState::new(window, cx).placeholder("minutes"));
        title.update(cx, |i, cx| i.focus(window, cx));
        NewTaskDialog {
            state: state.clone(),
            parent,
            mode: preset.as_ref().map(|p| p.0.clone()).unwrap_or("handoff".into()),
            prev: preset.as_ref().and_then(|p| p.1),
            title,
            instruction,
            usd,
            minutes,
            model: p.task.settings.model.clone(),
            images: vec![],
            focus: cx.focus_handle(),
        }
    });
    let sub = cx.subscribe_in(&d, window, |w: &mut Workspace, _, _: &DismissEvent, window, cx| w.close_overlay(window, cx));
    sub.detach();
    w.overlay = Some(Overlay::NewTask(d));
    cx.notify();
}

impl NewTaskDialog {
    fn ok(&self, cx: &App) -> bool {
        !self.title.read(cx).value().trim().is_empty() && (!self.instruction.read(cx).value().trim().is_empty() || !self.images.is_empty()) && (self.mode != "continue" || self.prev.is_some())
    }
    fn create(&mut self, start: bool, cx: &mut Context<Self>) {
        if !self.ok(cx) {
            return;
        }
        let parent = self.parent;
        let mode = self.mode.clone();
        let title = self.title.read(cx).value().trim().to_string();
        let instruction = self.instruction.read(cx).value().trim().to_string();
        let usd = self.usd.read(cx).value().trim().parse::<f64>().ok();
        let minutes = self.minutes.read(cx).value().trim().parse::<f64>().ok();
        let budget = if usd.is_some() || minutes.is_some() { Some(BudgetArgs { usd, minutes, ..Default::default() }) } else { None };
        let pmodel = self.state.read(cx).tasks.get(&parent).map(|t| t.task.settings.model.clone()).unwrap_or_default();
        let settings = if self.model != pmodel { Some(SettingsPatch { model: Some(self.model.clone()), ..Default::default() }) } else { None };
        let prev = if mode == "continue" { self.prev } else { None };
        let imgs: Vec<ImageIn> = self.images.drain(..).map(|a| ImageIn { media_type: a.media_type, bytes: (*a.bytes).clone(), name: a.name, w: a.w, h: a.h }).collect();
        self.state.update(cx, |s, cx| {
            s.run(cx, move |h| h.spawn(parent, &mode, &title, &instruction, imgs, prev, budget, settings, start), move |s, r, cx| match r {
                Ok(id) => {
                    let st = s.tasks.get(&id).map(|t| t.task.status);
                    s.notice = Some((true, format!("#{id} {}", if st == Some(TaskStatus::Pending) { "added" } else { "spawned" }), std::time::Instant::now()));
                    cx.emit(StateEvent::SpawnedOpen(id));
                }
                Err(e) => s.notice = Some((false, e, std::time::Instant::now())),
            })
        });
        cx.emit(DismissEvent);
    }
    fn attach(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: true, prompt: Some("Attach".into()) });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await {
                let _ = this.update(cx, |d, cx| {
                    for p in paths {
                        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                        let mt = match ext.as_str() {
                            "png" => "image/png",
                            "jpg" | "jpeg" => "image/jpeg",
                            "gif" => "image/gif",
                            "webp" => "image/webp",
                            _ => continue,
                        };
                        if let Ok(b) = std::fs::read(&p) {
                            if let Ok((mt, b, w, h)) = prepare_image(&b, mt) {
                                let fmt = if mt == "image/png" { ImageFormat::Png } else { ImageFormat::Jpeg };
                                let img = Arc::new(Image::from_bytes(fmt, b.clone()));
                                d.images.push(crate::composer::Attachment { name: p.file_name().and_then(|n| n.to_str()).unwrap_or("image").into(), media_type: mt, bytes: Arc::new(b), w, h, img });
                            }
                        }
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }
}

impl Render for NewTaskDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let k = c(cx).clone();
        let s = self.state.read(cx);
        let parent = self.parent;
        let p = s.tasks.get(&parent).cloned();
        let ptitle = if parent == 0 { "the main chat".to_string() } else { format!("#{parent} {}", p.as_ref().map(|p| p.task.title.clone()).unwrap_or_default()) };
        let pname = if parent == 0 { "the main chat".to_string() } else { format!("#{parent}") };
        let siblings: Vec<(i64, String, String)> = s.children_of(parent).into_iter().filter(|c| c.task.status.terminal()).map(|c| (c.task.id, c.task.status.as_str().to_string(), c.task.title.clone())).collect();
        let modes: [(&str, String); 4] = [
            ("handoff", "starts from your instruction alone (reads its lineage's memory)".into()),
            ("fork", format!("starts with a copy of {pname}'s view")),
            ("continue", "picks up a finished or canceled sibling's view".into()),
            ("blind", "starts from nothing and reads no memory but its own".into()),
        ];
        let mut cards = div().flex().flex_col().gap(px(6.)).mb(px(12.));
        for (m, d) in modes.iter() {
            let on = self.mode == *m;
            let off = *m == "continue" && siblings.is_empty();
            let mm = m.to_string();
            cards = cards.child(
                div()
                    .id(SharedString::from(format!("mode-{m}")))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(10.))
                    .py(px(7.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(if on { k.accent } else { k.line2 })
                    .when(on, |d| d.bg(k.accent_bg))
                    .when(off, |d| d.opacity(0.4))
                    .when(!off, |d| d.cursor_pointer().on_click(cx.listener(move |dlg, _, _, cx| {
                        dlg.mode = mm.clone();
                        cx.notify();
                    })))
                    .child(probe(format!("newtask.mode.{m}")))
                    .child(div().size(px(14.)).rounded_full().border_1().border_color(if on { k.accent } else { k.line2 }).flex().items_center().justify_center().when(on, |d| d.child(dot(k.accent, 7.))))
                    .child(div().w(px(70.)).font_weight(FontWeight::SEMIBOLD).child(m.to_string()))
                    .child(div().flex_1().text_size(px(12.)).text_color(k.muted).child(d.clone())),
            );
        }
        let ok = self.ok(cx);
        let models = s.models.clone();
        let pmodel = p.as_ref().map(|p| p.task.settings.model.clone()).unwrap_or_default();
        let model_label = s.model(&self.model).map(|m| format!("{}{}", m.label, if m.id == pmodel { " (inherited)" } else { "" })).unwrap_or(self.model.clone());
        let me = cx.entity().downgrade();
        let prev_label = self.prev.and_then(|id| siblings.iter().find(|x| x.0 == id)).map(|x| format!("#{} {} · {}", x.0, x.1, x.2)).unwrap_or("choose…".into());
        let me2 = cx.entity().downgrade();
        let sib2 = siblings.clone();
        let mut thumbs = div().flex().gap(px(8.)).mb(px(10.));
        for (n, a) in self.images.iter().enumerate() {
            thumbs = thumbs.child(div().id(("ntthumb", n)).size(px(56.)).rounded(px(8.)).overflow_hidden().border_1().border_color(k.line2).child(img(a.img.clone()).size_full().object_fit(ObjectFit::Cover)).cursor_pointer().on_click(cx.listener(move |d, _, _, cx| {
                if n < d.images.len() {
                    d.images.remove(n);
                }
                cx.notify();
            })));
        }
        dialog_box(cx, 620.)
            .id("newtask")
            .key_context("NewTask")
            .track_focus(&self.focus)
            .on_action(cx.listener(|d, _: &SubmitStart, _, cx| d.create(true, cx)))
            .overflow_y_scroll()
            .child(probe("newtask"))
            .child(div().text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).mb(px(6.)).child(format!("New task under {ptitle}")))
            .child(div().text_size(px(12.)).text_color(k.muted).mb(px(12.)).child(format!("You write as its parent. Its replies come back to {pname} as reports without waking it.")))
            .child(cards)
            .when(self.mode == "continue", |d| {
                d.child(div().mb(px(10.)).flex().flex_col().gap(px(4.)).child(div().text_size(px(11.5)).text_color(k.muted).child("continue which task")).child(Button::new("prev").outline().small().label(prev_label).dropdown_caret(true).dropdown_menu(move |mut m, _, _| {
                    for (id, st, title) in &sib2 {
                        let me = me2.clone();
                        let id = *id;
                        m = m.item(PopupMenuItem::new(format!("#{id} {st} · {title}")).on_click(move |_, _, cx| {
                            let _ = me.update(cx, |d, cx| {
                                d.prev = Some(id);
                                cx.notify();
                            });
                        }));
                    }
                    m
                })))
            })
            .child(div().mb(px(10.)).flex().flex_col().gap(px(4.)).child(div().text_size(px(11.5)).text_color(k.muted).child("title (imperative, ≤ 80 chars)")).child(div().child(Input::new(&self.title)).child(probe("newtask.title"))))
            .child(div().mb(px(10.)).flex().flex_col().gap(px(4.)).child(div().text_size(px(11.5)).text_color(k.muted).child("instruction")).child(div().child(Input::new(&self.instruction)).child(probe("newtask.instruction"))))
            .when(!self.images.is_empty(), |d| d.child(thumbs))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(10.))
                    .mb(px(14.))
                    .child(btn_small("nt-attach", "Attach images", Tone::Plain, cx).on_click(cx.listener(|d, _, _, cx| d.attach(cx))))
                    .child(Button::new("nt-model").outline().small().label(model_label).dropdown_caret(true).dropdown_menu(move |mut m, _, _| {
                        for mi in &models {
                            let me = me.clone();
                            let id = mi.id.clone();
                            m = m.item(PopupMenuItem::new(format!("{}{}", mi.label, if mi.id == pmodel { " (inherited)" } else { "" })).on_click(move |_, _, cx| {
                                let id = id.clone();
                                let _ = me.update(cx, |d, cx| {
                                    d.model = id;
                                    cx.notify();
                                });
                            }));
                        }
                        m.max_h(px(360.)).scrollable(true)
                    }))
                    .child(div().w(px(96.)).child(Input::new(&self.usd).small()))
                    .child(div().w(px(96.)).child(Input::new(&self.minutes).small())),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(btn("nt-cancel", "Cancel", Tone::Plain, cx).on_click(cx.listener(|_, _, _, cx| cx.emit(DismissEvent))))
                    .child(div().flex_1())
                    .child(disabled(btn("nt-add", "Add", Tone::Plain, cx), !ok).tooltip(tooltip("add it as pending: it waits until you start it")).child(probe("newtask.add")).when(ok, |d| d.on_click(cx.listener(|d, _, _, cx| d.create(false, cx)))))
                    .child(disabled(btn("nt-start", "Start", Tone::Primary, cx), !ok).tooltip(tooltip("⌘Enter")).child(probe("newtask.start")).when(ok, |d| d.on_click(cx.listener(|d, _, _, cx| d.create(true, cx))))),
            )
    }
}

// ---- command palette / task switcher ----------------------------------------------------------------------------------
#[derive(Clone)]
pub enum Cmd {
    Open(i64),
    Act(&'static str),
    Theme(String),
    Model(String),
    Effort(String),
}
pub struct Palette {
    state: Entity<AppState>,
    query: Entity<InputState>,
    tasks_only: bool,
    sel: usize,
    items: Vec<(String, String, Cmd)>,
    _subs: Vec<Subscription>,
}
impl Palette {
    /// for UI tests: the query and the label of the selected item
    pub fn probe_state(&self, cx: &App) -> serde_json::Value {
        serde_json::json!({ "query": self.query.read(cx).value().to_string(), "selected": self.items.get(self.sel).map(|x| x.0.clone()) })
    }
}
pub enum PaletteEvent {
    Run(Cmd),
}
impl EventEmitter<PaletteEvent> for Palette {}
impl EventEmitter<DismissEvent> for Palette {}
actions!(palette, [Up, Down, Confirm]);

pub fn bind(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("up", Up, Some("Palette")),
        KeyBinding::new("down", Down, Some("Palette")),
        KeyBinding::new("enter", Confirm, Some("Palette")),
        KeyBinding::new("cmd-enter", SubmitStart, Some("NewTask")),
    ]);
}

const COMMANDS: &[(&str, &str, &str)] = &[
    ("New task…", "⌘N", "new"),
    ("Toggle sidebar", "⌘B", "sidebar"),
    ("Show tasks", "⌘1", "tasks"),
    ("Show memory", "⌘2", "memory"),
    ("Show context", "⌘3", "context"),
    ("Show log", "⌘4", "log"),
    ("Show models", "⌘5", "models"),
    ("Show settings", "⌘,", "settings"),
    ("Pause this task", "", "pause"),
    ("Resume this task", "", "resume"),
    ("Close this task as done…", "", "close"),
    ("Cancel this task…", "", "cancel"),
    ("Stop the running turn", "⌘.", "stop"),
    ("Focus the composer", "⌘L", "focus"),
    ("Open the main chat", "⌘0", "main"),
    ("Close tab", "⌘W", "closetab"),
    ("Next tab", "⌃⇥", "nexttab"),
    ("Dictate", "M", "dictate"),
    ("Follow the system appearance", "", "follow"),
    ("Bigger text", "⌘=", "zoomin"),
    ("Smaller text", "⌘-", "zoomout"),
    ("Metrics and budget…", "", "metrics"),
    ("Quit OptChat", "⌘Q", "quit"),
];

pub fn open_palette(w: &mut Workspace, tasks_only: bool, window: &mut Window, cx: &mut Context<Workspace>) {
    let state = w.state.clone();
    let p = cx.new(|cx| {
        let query = cx.new(|cx| InputState::new(window, cx).placeholder(if tasks_only { "Open a task…" } else { "Type a command or a task…" }));
        let s1 = cx.subscribe(&query, |p: &mut Palette, _, e: &InputEvent, cx| {
            if let InputEvent::Change = e {
                p.sel = 0;
                p.refresh(cx);
            }
        });
        query.update(cx, |i, cx| i.focus(window, cx));
        let mut p = Palette { state, query, tasks_only, sel: 0, items: vec![], _subs: vec![s1] };
        p.refresh(cx);
        p
    });
    let s1 = cx.subscribe_in(&p, window, |w: &mut Workspace, _, e: &PaletteEvent, window, cx| {
        let PaletteEvent::Run(cmd) = e;
        w.close_overlay(window, cx);
        run_cmd(w, cmd.clone(), window, cx);
    });
    s1.detach();
    w.overlay = Some(Overlay::Palette(p));
    cx.notify();
}

fn fuzzy(q: &str, s: &str) -> Option<i64> {
    if q.is_empty() {
        return Some(0);
    }
    let s = s.to_lowercase();
    let q = q.to_lowercase();
    if let Some(p) = s.find(&q) {
        return Some(1000 - p as i64);
    }
    let mut it = s.chars();
    let mut score = 0;
    for qc in q.chars() {
        loop {
            match it.next() {
                Some(c) if c == qc => {
                    score += 1;
                    break;
                }
                Some(_) => continue,
                None => return None,
            }
        }
    }
    Some(score)
}

impl Palette {
    fn refresh(&mut self, cx: &mut Context<Self>) {
        let q = self.query.read(cx).value().to_string();
        let s = self.state.read(cx);
        let mut items: Vec<(i64, String, String, Cmd)> = vec![];
        if !self.tasks_only {
            for (label, key, act) in COMMANDS {
                if let Some(sc) = fuzzy(&q, label) {
                    items.push((sc + 50, label.to_string(), key.to_string(), Cmd::Act(act)));
                }
            }
            for m in &s.models {
                let l = format!("Model: {}", m.label);
                if let Some(sc) = fuzzy(&q, &l) {
                    items.push((sc + 10, l, m.id.clone(), Cmd::Model(m.id.clone())));
                }
            }
            if let Some(set) = &s.cur.settings {
                for e in s.model(&set.model).map(|m| m.efforts.clone()).unwrap_or_default() {
                    let l = format!("Effort: {e}");
                    if let Some(sc) = fuzzy(&q, &l) {
                        items.push((sc + 10, l, if e == set.effort { "current".into() } else { String::new() }, Cmd::Effort(e)));
                    }
                }
            }
            for t in cx.global::<Themes>().all.iter() {
                let l = format!("Theme: {}", t.name);
                if let Some(sc) = fuzzy(&q, &l) {
                    items.push((sc, l, String::new(), Cmd::Theme(t.id.clone())));
                }
            }
        }
        if std::env::var("OPTCHAT_TEST_HOOKS").is_ok() && !self.tasks_only {
            for (l, a) in [("Test: approval dialog", "t-approval"), ("Test: attach image", "t-image"), ("Test: reset frame stats", "t-frames"), ("Test: narrow window", "t-narrow"), ("Test: wide window", "t-wide")] {
                if let Some(sc) = fuzzy(&q, l) {
                    items.push((sc + 60, l.to_string(), String::new(), Cmd::Act(a)));
                }
            }
        }
        for x in s.tasks.values().rev() {
            let l = format!("#{} {}", x.task.id, task_name(&x.task));
            if let Some(sc) = fuzzy(&q, &l) {
                items.push((sc + if self.tasks_only { 100 } else { 0 }, l, x.task.status.as_str().to_string(), Cmd::Open(x.task.id)));
            }
        }
        items.sort_by(|a, b| b.0.cmp(&a.0));
        self.items = items.into_iter().take(60).map(|(_, a, b, c)| (a, b, c)).collect();
        cx.notify();
    }
}
impl Render for Palette {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::ui::set_state_extra("palette", self.probe_state(cx));
        let k = c(cx).clone();
        let mut list = div().id("pal-list").flex().flex_col().max_h(px(380.)).overflow_y_scroll().py(px(4.));
        for (n, (label, hint, cmd)) in self.items.iter().enumerate() {
            let on = n == self.sel;
            let cmd = cmd.clone();
            list = list.child(
                div()
                    .id(("pal", n))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(12.))
                    .py(px(6.))
                    .mx(px(4.))
                    .rounded(px(6.))
                    .cursor_pointer()
                    .when(on, |d| d.bg(k.accent_bg))
                    .hover(|s| s.bg(k.hover))
                    .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().child(label.clone()))
                    .child(div().text_size(px(11.)).text_color(k.muted).child(hint.clone()))
                    .on_click(cx.listener(move |_, _, _, cx| cx.emit(PaletteEvent::Run(cmd.clone())))),
            );
        }
        div()
            .id("palette")
            .key_context("Palette")
            .w(px(560.))
            .bg(k.panel)
            .border_1()
            .border_color(k.line2)
            .rounded(px(12.))
            .shadow_lg()
            .overflow_hidden()
            .on_action(cx.listener(|p, _: &Up, _, cx| {
                p.sel = p.sel.saturating_sub(1);
                cx.notify();
            }))
            .on_action(cx.listener(|p, _: &Down, _, cx| {
                p.sel = (p.sel + 1).min(p.items.len().saturating_sub(1));
                cx.notify();
            }))
            .on_action(cx.listener(|p, _: &Confirm, _, cx| {
                if let Some((_, _, cmd)) = p.items.get(p.sel).cloned() {
                    cx.emit(PaletteEvent::Run(cmd));
                }
            }))
            .child(probe("palette"))
            .child(div().p(px(8.)).border_b_1().border_color(k.line).child(Input::new(&self.query).appearance(false)))
            .child(list)
    }
}

pub fn run_cmd(w: &mut Workspace, cmd: Cmd, window: &mut Window, cx: &mut Context<Workspace>) {
    match cmd {
        Cmd::Open(id) => w.open_task(id, window, cx),
        Cmd::Theme(id) => set_theme(w, &id, false, cx),
        Cmd::Model(id) => {
            let task = w.state.read(cx).current;
            w.state.update(cx, |s, cx| s.command(cx, true, move |h| h.update_settings(task, SettingsPatch { model: Some(id), ..Default::default() }).map(|r| format!("model: {r}"))));
        }
        Cmd::Effort(e) => {
            let task = w.state.read(cx).current;
            w.state.update(cx, |s, cx| s.command(cx, true, move |h| h.update_settings(task, SettingsPatch { effort: Some(e), ..Default::default() }).map(|r| format!("effort: {r}"))));
        }
        Cmd::Act(a) => match a {
            "new" => open_new_task(w, None, window, cx),
            "sidebar" => window.dispatch_action(Box::new(ToggleSidebar), cx),
            "tasks" => w.pick(View::Tasks, cx),
            "memory" => w.pick(View::Memory, cx),
            "context" => w.pick(View::Context, cx),
            "log" => w.pick(View::Log, cx),
            "models" => w.pick(View::Models, cx),
            "settings" => w.pick(View::Settings, cx),
            "pause" | "resume" | "close" | "cancel" => w.start_ask(a, window, cx),
            "stop" => window.dispatch_action(Box::new(StopTurn), cx),
            "focus" => w.composer.update(cx, |c, cx| c.focus(window, cx)),
            "main" => w.open_task(0, window, cx),
            "closetab" => window.dispatch_action(Box::new(CloseTab), cx),
            "nexttab" => cycle_tab(w, 1, window, cx),
            "dictate" => {
                w.composer.update(cx, |c, cx| {
                    c.mic_down(window, cx);
                    c.mic_up(cx);
                });
            }
            "follow" => {
                let a = window.appearance();
                w.state.update(cx, |s, _| {
                    s.prefs.follow_system = true;
                    s.save_prefs();
                });
                cx.global_mut::<Themes>().follow_system = true;
                crate::theme::on_appearance(cx, a);
            }
            "zoomin" => zoom(w, 1.0, cx),
            "zoomout" => zoom(w, -1.0, cx),
            "metrics" => open_metrics(w, window, cx),
            "quit" => cx.quit(),
            "t-approval" => w.state.update(cx, |s, _| {
                if let Some(h) = s.hub() {
                    let _g = s.rt.enter();
                    h.test_approval();
                }
            }),
            "t-image" => {
                let p = std::path::PathBuf::from(std::env::var("OPTCHAT_TEST_IMAGE").unwrap_or_default());
                w.composer.update(cx, |c, cx| c.add_paths(vec![p], cx));
            }
            "t-frames" => crate::ui::reset_frames(),
            "t-narrow" => window.resize(size(px(700.), px(820.))),
            "t-wide" => window.resize(size(px(1360.), px(880.))),
            _ => {}
        },
    }
    cx.notify();
}
