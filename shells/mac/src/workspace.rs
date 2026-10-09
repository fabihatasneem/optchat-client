//! The window (app/ui/app.tsx), VS Code style: an activity rail, a resizable sidebar with the rail's view, editor
//! tabs of open tasks with the task's actions, a thin task bar, the feed and composer, and a status bar. Overlays:
//! dialogs, the command palette, approvals, the lightbox, toasts.
use crate::composer::Composer;
use crate::dialogs::{self, Overlay};
use crate::feed::{logo, Feed, FeedEvent};
use crate::panels::{ContextPanel, LogPanel, MemoryPanel, ModelsPanel, SettingsPanel};
use crate::state::*;
use crate::statusbar;
use crate::tasks_view;
use crate::theme::c;
use crate::ui::*;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui::Context;
use gpui_component::input::{InputEvent, InputState};
use optchat_engine::types::*;

actions!(
    optchat,
    [
        ToggleSidebar,
        ShowTasks,
        ShowMemory,
        ShowContext,
        ShowLog,
        ShowModels,
        ShowSettings,
        NewTask,
        CloseTab,
        NextTab,
        PrevTab,
        StopTurn,
        FocusComposer,
        CommandPalette,
        TaskSwitcher,
        PauseTask,
        ResumeTask,
        CloseTask,
        CancelTask,
        Dismiss,
        DictateKey,
        OpenMainChat,
        ZoomIn,
        ZoomOut,
        ZoomReset,
        NextTheme,
        ToggleFollowSystem,
    ]
);

pub fn bind(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-b", ToggleSidebar, None),
        KeyBinding::new("cmd-1", ShowTasks, None),
        KeyBinding::new("cmd-2", ShowMemory, None),
        KeyBinding::new("cmd-3", ShowContext, None),
        KeyBinding::new("cmd-4", ShowLog, None),
        KeyBinding::new("cmd-5", ShowModels, None),
        KeyBinding::new("cmd-,", ShowSettings, None),
        KeyBinding::new("cmd-6", ShowSettings, None),
        KeyBinding::new("cmd-n", NewTask, None),
        KeyBinding::new("cmd-w", CloseTab, None),
        KeyBinding::new("cmd-shift-]", NextTab, None),
        KeyBinding::new("cmd-shift-[", PrevTab, None),
        KeyBinding::new("ctrl-tab", NextTab, None),
        KeyBinding::new("ctrl-shift-tab", PrevTab, None),
        KeyBinding::new("cmd-.", StopTurn, None),
        KeyBinding::new("cmd-l", FocusComposer, None),
        KeyBinding::new("cmd-k", CommandPalette, None),
        KeyBinding::new("cmd-shift-p", CommandPalette, None),
        KeyBinding::new("cmd-t", TaskSwitcher, None),
        KeyBinding::new("cmd-p", TaskSwitcher, None),
        KeyBinding::new("cmd-0", OpenMainChat, None),
        KeyBinding::new("cmd-=", ZoomIn, None),
        KeyBinding::new("cmd-+", ZoomIn, None),
        KeyBinding::new("cmd--", ZoomOut, None),
        KeyBinding::new("cmd-shift-0", ZoomReset, None),
        KeyBinding::new("escape", Dismiss, Some("Workspace")),
        KeyBinding::new("m", DictateKey, Some("Workspace && !Input")),
    ]);
}

#[derive(Clone)]
pub struct Ask {
    pub op: &'static str,
    pub label: &'static str,
    pub placeholder: &'static str,
    pub required: bool,
}
pub fn ask_of(op: &str) -> Ask {
    match op {
        "pause" => Ask { op: "pause", label: "Pause", placeholder: "why (optional)", required: false },
        "resume" => Ask { op: "resume", label: "Resume", placeholder: "a message to it (optional)", required: false },
        "cancel" => Ask { op: "cancel", label: "Cancel task", placeholder: "why (optional)", required: false },
        _ => Ask { op: "close", label: "Close as done", placeholder: "result: what it did, past tense (≤ 280 chars)", required: true },
    }
}

pub struct Workspace {
    pub state: Entity<AppState>,
    pub focus: FocusHandle,
    pub feed: Entity<Feed>,
    pub composer: Entity<Composer>,
    pub memory: Entity<MemoryPanel>,
    pub context: Entity<ContextPanel>,
    pub log: Entity<LogPanel>,
    pub models: Entity<ModelsPanel>,
    pub settings: Entity<SettingsPanel>,
    pub ask: Option<Ask>,
    pub ask_input: Entity<InputState>,
    pub overlay: Option<Overlay>,
    pub tasks_panel: Entity<tasks_view::TasksPanel>,
    resizing: Option<(f32, f32)>,
    pub metrics_open: bool,
    pub narrow: bool,
    pub sheet: bool,
    m_press: Option<std::time::Instant>,
    _subs: Vec<Subscription>,
}

impl Workspace {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Workspace {
        let feed = cx.new(|cx| Feed::new(state.clone(), cx));
        let composer = cx.new(|cx| Composer::new(state.clone(), window, cx));
        let memory = cx.new(|cx| MemoryPanel::new(state.clone(), window, cx));
        let context = cx.new(|cx| ContextPanel::new(state.clone(), cx));
        let log = cx.new(|cx| LogPanel::new(state.clone(), cx));
        let models = cx.new(|cx| ModelsPanel::new(state.clone(), window, cx));
        let settings = cx.new(|cx| SettingsPanel::new(state.clone(), window, cx));
        let ask_input = cx.new(|cx| InputState::new(window, cx));
        let tasks_panel = cx.new(|cx| tasks_view::TasksPanel::new(state.clone(), cx));
        let mut subs = vec![];
        subs.push(cx.observe(&state, |_, _, cx| cx.notify()));
        subs.push(cx.subscribe_in(&feed, window, |this: &mut Workspace, _, ev: &FeedEvent, window, cx| match ev {
            FeedEvent::OpenTask(id) => this.open_task(*id, window, cx),
            FeedEvent::SelectRequest(id) => this.select_request(id.clone(), cx),
            FeedEvent::Lightbox(blob) => {
                this.overlay = Some(Overlay::Lightbox(blob.clone()));
                cx.notify();
            }
        }));
        subs.push(cx.subscribe_in(&tasks_panel, window, |this: &mut Workspace, _, ev: &tasks_view::TasksEvent, window, cx| {
            let tasks_view::TasksEvent::Open(id) = ev;
            this.open_task(*id, window, cx);
        }));
        subs.push(cx.subscribe_in(&ask_input, window, |this: &mut Workspace, _, ev: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                this.confirm_ask(window, cx);
            }
        }));
        subs.push(cx.subscribe_in(&state, window, |this: &mut Workspace, _, ev: &StateEvent, window, cx| {
            if let StateEvent::SpawnedOpen(id) = ev {
                this.open_task(*id, window, cx);
            }
            if let StateEvent::Opened(_) = ev {
                this.update_title(window, cx);
            }
        }));
        subs.push(cx.observe_window_appearance(window, |_, window, cx| {
            crate::theme::sync_window(window, cx);
        }));
        subs.push(cx.observe_window_bounds(window, |this: &mut Workspace, window, cx| {
            let narrow = f32::from(window.bounds().size.width) < 760.0;
            if narrow != this.narrow {
                this.narrow = narrow;
                this.sheet = false;
                cx.notify();
            }
        }));
        // a closed menu or a removed element leaves nothing focused, and then no workspace key works:
        // give focus back to the workspace (its keys, M to dictate)
        subs.push(cx.on_focus_lost(window, |this: &mut Workspace, window, cx| {
            window.focus(&this.focus);
            cx.notify();
        }));
        let focus = cx.focus_handle();
        Workspace {
            state,
            focus,
            feed,
            composer,
            memory,
            context,
            log,
            models,
            settings,
            ask: None,
            ask_input,
            overlay: None,
            tasks_panel,
            resizing: None,
            metrics_open: false,
            narrow: false,
            sheet: false,
            m_press: None,
            _subs: subs,
        }
    }

    pub fn update_title(&self, window: &mut Window, cx: &mut Context<Self>) {
        let s = self.state.read(cx);
        let t = match s.info() {
            Some(i) => format!("{}{} · OptChat", task_name(&i.task), if i.task.id != 0 { format!(" · #{}", i.task.id) } else { String::new() }),
            None => "OptChat".into(),
        };
        window.set_window_title(&t);
    }

    pub fn open_task(&mut self, id: i64, window: &mut Window, cx: &mut Context<Self>) {
        self.ask = None;
        self.metrics_open = false;
        if self.narrow {
            self.sheet = false;
        }
        self.state.update(cx, |s, cx| s.open_task(id, cx));
        self.update_title(window, cx);
        self.composer.update(cx, |c, cx| c.focus(window, cx));
        cx.notify();
    }
    pub fn close_overlay(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.overlay = None;
        self.composer.update(cx, |c, cx| c.focus(window, cx));
        cx.notify();
    }
    pub fn select_request(&mut self, id: String, cx: &mut Context<Self>) {
        self.state.update(cx, |s, cx| {
            s.selected_request = Some(id);
            s.pinned_request = true;
            s.prefs.view = View::Context;
            s.prefs.side_open = true;
            s.save_prefs();
            cx.notify();
        });
    }
    pub fn side_open(&self, cx: &App) -> bool {
        if self.narrow { self.sheet } else { self.state.read(cx).prefs.side_open }
    }
    pub fn set_side_open(&mut self, v: bool, cx: &mut Context<Self>) {
        if self.narrow {
            self.sheet = v;
        } else {
            self.state.update(cx, |s, _| {
                s.prefs.side_open = v;
                s.save_prefs();
            });
        }
        cx.notify();
    }
    /// the rail: a click shows that view; a click on the view already shown hides the sidebar (as in VS Code)
    pub fn pick(&mut self, v: View, cx: &mut Context<Self>) {
        let open = self.side_open(cx);
        if open && self.state.read(cx).prefs.view == v {
            self.set_side_open(false, cx);
            return;
        }
        self.state.update(cx, |s, _| {
            s.prefs.view = v;
            s.save_prefs();
        });
        self.set_side_open(true, cx);
    }

    pub fn start_ask(&mut self, op: &str, window: &mut Window, cx: &mut Context<Self>) {
        let a = ask_of(op);
        self.ask_input.update(cx, |i, cx| {
            i.set_value("", window, cx);
            i.set_placeholder(a.placeholder, window, cx);
            i.focus(window, cx);
        });
        self.ask = Some(a);
        cx.notify();
    }
    pub fn confirm_ask(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(a) = self.ask.clone() else { return };
        let val = self.ask_input.read(cx).value().trim().to_string();
        if a.required && val.is_empty() {
            return;
        }
        let task = self.state.read(cx).current;
        let op = a.op;
        self.state.update(cx, |s, cx| {
            s.command(cx, true, move |h| match op {
                "pause" => h.pause(task, &val),
                "resume" => h.resume(task, &val, vec![]),
                "cancel" => h.cancel(task, &val),
                _ => h.close(task, &val),
            })
        });
        self.ask = None;
        self.composer.update(cx, |c, cx| c.focus(window, cx));
        cx.notify();
    }

    fn on_dictate_key(&mut self, _: &DictateKey, window: &mut Window, cx: &mut Context<Self>) {
        // M outside any text field: tap toggles, hold dictates while held (key-up handled in on_key_up)
        if self.m_press.is_some() {
            return;
        }
        self.m_press = Some(std::time::Instant::now());
        self.composer.update(cx, |c, cx| c.mic_down(window, cx));
    }
}

impl Focusable for Workspace {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let k = c(cx).clone();
        if self.state.read(cx).stats.first_frame_ms.is_none() {
            let launch = crate::LAUNCH.get().map(|t| t.elapsed().as_secs_f64() * 1000.0).unwrap_or(0.0);
            self.state.update(cx, |s, _| s.stats.first_frame_ms = Some(launch));
        }
        // nothing focused (a finished task has no composer, a dialog closed): keep the workspace's keys working
        let ended_now = self.state.read(cx).info().map(|i| i.task.status.terminal()).unwrap_or(false);
        if self.overlay.is_none() && self.ask.is_none() && (window.focused(cx).is_none() || (ended_now && !self.focus.contains_focused(window, cx))) {
            window.focus(&self.focus);
        }
        if probes_on() {
            let s = self.state.read(cx);
            let th = cx.global::<crate::theme::Themes>();
            let last_talk = s.cur.log.iter().rev().find(|e| e.kind == LogKind::Talk).map(|e| e.text.clone());
            let j = serde_json::json!({
                "ready": s.hub.is_some(), "current": s.current, "loaded": s.cur.loaded, "status": s.info().map(|i| i.task.status.as_str()),
                "running": s.cur.running.as_ref().map(|r| r.status.clone()), "t": s.cur.t, "lastTalk": last_talk,
                "lastKinds": s.cur.log.iter().rev().take(8).map(|e| e.kind.as_str()).collect::<Vec<_>>(),
                "tabs": s.prefs.tabs, "view": s.prefs.view, "fontSize": s.prefs.font_size, "settingsGlobal": s.prefs.settings_tab_global, "followSystem": s.prefs.follow_system, "narrow": self.narrow,  "selectedRequest": s.selected_request, "sideOpen": self.side_open(cx), "theme": th.active, "overlay": self.overlay.as_ref().map(|o| match o { Overlay::NewTask(_) => "newtask", Overlay::Palette(_) => "palette", Overlay::Metrics(_) => "metrics", Overlay::Lightbox(_) => "lightbox" }),
                "ask": self.ask.as_ref().map(|a| a.op), "dict": s.dict.state, "approvals": s.approvals.len(), "notice": s.notice.as_ref().map(|n| n.1.clone()),
                "model": s.cur.settings.as_ref().map(|x| x.model.clone()), "effort": s.cur.settings.as_ref().map(|x| x.effort.clone()), "tasks": s.tasks.len(),
                "queue": s.cur.queue.len(), "unrun": s.info().and_then(|i| i.task.unrun), "draft": self.composer.read(cx).input.read(cx).value().to_string(),
                "firstFrameMs": s.stats.first_frame_ms, "frames": frame_summary(), "rss": crate::ui::rss_mb(), "loadedMs": s.stats.loaded_ms, "errors": s.errors.clone(),
            });
            set_state_json(j.to_string());
        }
        let s = self.state.read(cx);
        let open = self.side_open(cx);
        let view = s.prefs.view;
        let side_w = s.prefs.side_w;
        let ended = s.info().map(|i| i.task.status.terminal()).unwrap_or(false);
        let narrow = self.narrow;

        let mut root = div()
            .id("workspace")
            .key_context("Workspace")
            .track_focus(&self.focus)
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .bg(k.bg)
            .text_color(k.text)
            .text_size(px(13.5))
            .font_family(".SystemUIFont")
            .on_action(cx.listener(|w, _: &ToggleSidebar, _, cx| {
                let o = w.side_open(cx);
                w.set_side_open(!o, cx)
            }))
            .on_action(cx.listener(|w, _: &ShowTasks, _, cx| w.pick(View::Tasks, cx)))
            .on_action(cx.listener(|w, _: &ShowMemory, _, cx| w.pick(View::Memory, cx)))
            .on_action(cx.listener(|w, _: &ShowContext, _, cx| w.pick(View::Context, cx)))
            .on_action(cx.listener(|w, _: &ShowLog, _, cx| w.pick(View::Log, cx)))
            .on_action(cx.listener(|w, _: &ShowModels, _, cx| w.pick(View::Models, cx)))
            .on_action(cx.listener(|w, _: &ShowSettings, _, cx| w.pick(View::Settings, cx)))
            .on_action(cx.listener(|w, _: &NewTask, window, cx| dialogs::open_new_task(w, None, window, cx)))
            .on_action(cx.listener(|w, _: &CloseTab, window, cx| {
                let cur = w.state.read(cx).current;
                w.state.update(cx, |s, cx| s.close_tab(cur, cx));
                w.update_title(window, cx);
            }))
            .on_action(cx.listener(|w, _: &NextTab, window, cx| dialogs::cycle_tab(w, 1, window, cx)))
            .on_action(cx.listener(|w, _: &PrevTab, window, cx| dialogs::cycle_tab(w, -1, window, cx)))
            .on_action(cx.listener(|w, _: &StopTurn, _, cx| {
                let t = w.state.read(cx).current;
                w.state.update(cx, |s, cx| s.command(cx, false, move |h| { h.stop(t); Ok(String::new()) }));
            }))
            .on_action(cx.listener(|w, _: &FocusComposer, window, cx| w.composer.update(cx, |c, cx| c.focus(window, cx))))
            .on_action(cx.listener(|w, _: &CommandPalette, window, cx| dialogs::open_palette(w, false, window, cx)))
            .on_action(cx.listener(|w, _: &TaskSwitcher, window, cx| dialogs::open_palette(w, true, window, cx)))
            .on_action(cx.listener(|w, _: &OpenMainChat, window, cx| w.open_task(0, window, cx)))
            .on_action(cx.listener(|w, _: &PauseTask, window, cx| w.start_ask("pause", window, cx)))
            .on_action(cx.listener(|w, _: &ResumeTask, window, cx| w.start_ask("resume", window, cx)))
            .on_action(cx.listener(|w, _: &CloseTask, window, cx| w.start_ask("close", window, cx)))
            .on_action(cx.listener(|w, _: &CancelTask, window, cx| w.start_ask("cancel", window, cx)))
            .on_action(cx.listener(|w, _: &ZoomIn, _, cx| dialogs::zoom(w, 1.0, cx)))
            .on_action(cx.listener(|w, _: &ZoomOut, _, cx| dialogs::zoom(w, -1.0, cx)))
            .on_action(cx.listener(|w, _: &ZoomReset, _, cx| dialogs::zoom(w, 0.0, cx)))
            .on_action(cx.listener(|w, _: &NextTheme, _, cx| dialogs::next_theme(w, cx)))
            .on_action(cx.listener(|w, _: &Dismiss, window, cx| {
                if w.state.read(cx).dict.state != "idle" {
                    // Esc stops dictation and keeps the text
                    w.composer.update(cx, |c, cx| c.stop_dictation(cx));
                } else if w.overlay.is_some() {
                    w.overlay = None;
                    w.composer.update(cx, |c, cx| c.focus(window, cx));
                } else if w.ask.is_some() {
                    w.ask = None;
                    w.composer.update(cx, |c, cx| c.focus(window, cx));
                } else if w.metrics_open {
                    w.metrics_open = false;
                } else if w.narrow && w.sheet {
                    w.sheet = false;
                } else {
                    w.composer.update(cx, |c, cx| c.focus(window, cx));
                }
                cx.notify();
            }))
            .on_action(cx.listener(Self::on_dictate_key))
            .on_key_up(cx.listener(|w, e: &KeyUpEvent, _, cx| {
                if e.keystroke.key == "m" {
                    if w.m_press.take().is_some() {
                        w.composer.update(cx, |c, cx| c.mic_up(cx));
                    }
                }
            }))
            .on_mouse_move(cx.listener(|w, e: &MouseMoveEvent, _, cx| {
                if let Some((x0, w0)) = w.resizing {
                    if e.pressed_button == Some(MouseButton::Left) {
                        let nw = (w0 + f32::from(e.position.x) - x0).clamp(220., 760.);
                        w.state.update(cx, |s, _| s.prefs.side_w = nw);
                        cx.notify();
                    } else {
                        w.resizing = None;
                        w.state.update(cx, |s, _| s.save_prefs());
                    }
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|w, _, _, cx| {
                if w.resizing.take().is_some() {
                    w.state.update(cx, |s, _| s.save_prefs());
                }
            }));

        // ---- body: rail · sidebar · editor
        let mut body = div().flex_1().min_h_0().flex().relative();
        body = body.child(self.rail(open, view, cx));
        // the docked sidebar sits in the row; in a narrow window it floats over the editor (painted after it)
        if open && !narrow {
            let side = self.sidebar(view, side_w, narrow, window, cx);
            body = body.child(side);
        }
        body = body.child(self.editor(ended, window, cx));
        if open && narrow {
            let side = self.sidebar(view, side_w, narrow, window, cx);
            body = body.child(div().id("scrim").absolute().inset_0().left(px(48.)).bg(hsla(0., 0., 0., 0.35)).on_click(cx.listener(|w, _, _, cx| w.set_side_open(false, cx))));
            body = body.child(div().id("sheet").occlude().absolute().top_0().bottom_0().left(px(48.)).w(px(side_w.min(f32::from(window.bounds().size.width) - 60.))).child(side));
        }
        root = root.child(body);
        root = root.child(statusbar::render(self, window, cx));
        root = root.child(frame_probe());

        // ---- overlays
        if let Some(o) = self.overlay.clone() {
            root = root.child(dialogs::render_overlay(self, o, window, cx));
        }
        if let Some(a) = self.state.read(cx).approvals.first().cloned() {
            root = root.child(dialogs::approval(self, a, cx));
        }
        root = root.children(dialogs::toast(self, cx));
        root
    }
}

impl Workspace {
    fn rail(&self, open: bool, view: View, cx: &mut Context<Self>) -> impl IntoElement {
        let k = c(cx).clone();
        let s = self.state.read(cx);
        let active = s.tasks.values().filter(|x| x.task.status == TaskStatus::Active).count();
        let compacting = !s.busy.is_empty();
        let mut rail = div()
            .w(px(48.))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(2.))
            .pt(px(10.))
            .pb(px(6.))
            .bg(k.rail)
            .border_r_1()
            .border_color(k.line)
            .child(div().id("logo").mb(px(12.)).mt(px(2.)).cursor_pointer().child(logo(24., cx)).tooltip(tooltip("OptChat · main chat (⌘0)")).on_click(cx.listener(|w, _, window, cx| w.open_task(0, window, cx))));
        for v in [View::Tasks, View::Memory, View::Context, View::Log, View::Models] {
            rail = rail.child(self.rail_btn(v, open && view == v, if v == View::Tasks { active } else { 0 }, v == View::Memory && compacting, cx));
        }
        rail = rail.child(div().flex_1());
        rail = rail.child(self.rail_btn(View::Settings, open && view == View::Settings, 0, false, cx));
        let k2 = k.clone();
        rail = rail.child(
            div()
                .id("rail-toggle")
                .size(px(40.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(8.))
                .cursor_pointer()
                .hover(move |s| s.bg(k2.hover))
                .tooltip(tooltip(if open { "Hide the sidebar (⌘B)" } else { "Show the sidebar (⌘B)" }))
                .child(icon(if open { "panel-left-close" } else { "panel-left-open" }, 19., k.muted))
                .child(probe("rail.toggle"))
                .on_click(cx.listener(move |w, _, _, cx| w.set_side_open(!open, cx))),
        );
        rail
    }

    fn rail_btn(&self, v: View, on: bool, badge: usize, dot_on: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let k = c(cx).clone();
        let col = if on { k.text } else { k.muted };
        let hover = k.text2;
        div()
            .id(SharedString::from(format!("rail-{}", v.label())))
            .relative()
            .size(px(40.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(8.))
            .cursor_pointer()
            .tooltip(tooltip(format!("{}{}", v.label(), if on { " (click to hide the sidebar)" } else { "" })))
            .child(icon(v.icon(), 21., col).hover(move |s| s.text_color(hover)))
            .child(probe(format!("rail.{}", v.label().to_lowercase())))
            .when(on, |d| d.child(div().absolute().left(px(-4.)).top(px(9.)).bottom(px(9.)).w(px(2.)).rounded(px(2.)).bg(k.accent)))
            .when(badge > 0, |d| {
                d.child(
                    div()
                        .absolute()
                        .top(px(4.))
                        .right(px(3.))
                        .min_w(px(15.))
                        .h(px(15.))
                        .px(px(3.))
                        .rounded_full()
                        .bg(k.accent2)
                        .text_color(k.on_accent)
                        .text_size(px(9.5))
                        .font_weight(FontWeight::BOLD)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(badge.to_string()),
                )
            })
            .when(dot_on, |d| d.child(div().absolute().top(px(8.)).right(px(8.)).child(pulse("memdot", dot(k.amber, 7.)))))
            .on_click(cx.listener(move |w, _, _, cx| w.pick(v, cx)))
    }

    fn sidebar(&mut self, view: View, w: f32, narrow: bool, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let k = c(cx).clone();
        let s = self.state.read(cx);
        let ended = s.info().map(|i| i.task.status.terminal()).unwrap_or(false);
        let scope_all = s.prefs.task_scope_all;
        let show_ended = s.prefs.show_ended;
        let pinned = s.pinned_request;
        let mut head = div()
            .flex()
            .items_center()
            .gap(px(2.))
            .h(px(38.))
            .px(px(12.))
            .flex_none()
            .child(div().text_size(px(11.)).font_weight(FontWeight::SEMIBOLD).text_color(k.muted).child(view.label().to_uppercase()))
            .child(div().flex_1());
        if view == View::Tasks {
            head = head
                .child(icon_btn("scope", "layers", 15., cx).when(scope_all, |d| d.bg(k.hover)).tooltip(tooltip(if scope_all { "show the current task's subtree" } else { "show every task" })).child(probe("side.scope")).on_click(cx.listener(|w, _, _, cx| {
                    w.state.update(cx, |s, cx| {
                        s.prefs.task_scope_all = !s.prefs.task_scope_all;
                        s.save_prefs();
                        cx.notify();
                    })
                })))
                .child(icon_btn("ended", if show_ended { "eye" } else { "eye-off" }, 15., cx).tooltip(tooltip(if show_ended { "hide finished tasks" } else { "show finished tasks" })).on_click(cx.listener(|w, _, _, cx| {
                    w.state.update(cx, |s, cx| {
                        s.prefs.show_ended = !s.prefs.show_ended;
                        s.save_prefs();
                        cx.notify();
                    })
                })))
                .when(!ended, |d| d.child(icon_btn("side-new", "plus", 16., cx).tooltip(tooltip("new task under the current one (⌘N)")).child(probe("side.new")).on_click(cx.listener(|w, _, window, cx| dialogs::open_new_task(w, None, window, cx)))));
        }
        if view == View::Context && pinned {
            head = head.child(btn_small("follow", "follow latest", Tone::Ghost, cx).on_click(cx.listener(|w, _, _, cx| {
                w.state.update(cx, |s, cx| {
                    s.pinned_request = false;
                    cx.notify();
                })
            })));
        }
        head = head.child(icon_btn("side-hide", "panel-left-close", 15., cx).tooltip(tooltip("hide the sidebar (⌘B)")).on_click(cx.listener(|w, _, _, cx| w.set_side_open(false, cx))));
        let loaded = s.cur.loaded;
        let cached = |v: AnyView| v.cached(StyleRefinement::default().size_full()).into_any_element();
        let body: AnyElement = match view {
            View::Tasks => cached(self.tasks_panel.clone().into()),
            _ if !loaded => div().p(px(12.)).child(skeleton_line(None, cx)).child(skeleton_line(Some(160.), cx)).into_any_element(),
            View::Memory => cached(self.memory.clone().into()),
            View::Context => cached(self.context.clone().into()),
            View::Log => cached(self.log.clone().into()),
            View::Models => cached(self.models.clone().into()),
            View::Settings => cached(self.settings.clone().into()),
        };
        let resizer = div()
            .id("resizer")
            .absolute()
            .top_0()
            .bottom_0()
            .right(px(-3.))
            .w(px(6.))
            .cursor_col_resize()
            .hover(|s| s.bg(k.accent.opacity(0.25)))
            .on_mouse_down(MouseButton::Left, cx.listener(move |ws, e: &MouseDownEvent, _, cx| {
                ws.resizing = Some((f32::from(e.position.x), w));
                cx.notify();
            }));
        div()
            .id("sidebar")
            .relative()
            .w(px(w))
            .when(narrow, |d| d.w_full().shadow_lg())
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .bg(k.bg2)
            .border_r_1()
            .border_color(k.line)
            .child(probe("sidebar"))
            .child(head)
            .child(div().id("sidebody").flex_1().min_h_0().overflow_hidden().child(body))
            .when(!narrow, |d| d.child(resizer))
    }

    fn editor(&mut self, ended: bool, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let k = c(cx).clone();
        let mut ed = div().flex_1().min_w_0().h_full().flex().flex_col().bg(k.bg);
        ed = ed.child(crate::tabs::tab_strip(self, window, cx));
        ed = ed.child(crate::tabs::task_bar(self, window, cx));
        // a click in the feed leaves the composer (so M dictates, as outside a text field in the web app)
        ed = ed.child(div().flex_1().min_h_0().child(AnyView::from(self.feed.clone()).cached(StyleRefinement::default().size_full())).child(probe("feed")).on_mouse_down(MouseButton::Left, cx.listener(|w, _, window, _| window.focus(&w.focus))));
        if ended {
            ed = ed.child(crate::tabs::endbar(self, cx));
        } else {
            // not cached: a cached view is laid out as a leaf, and the composer sizes to its content
            ed = ed.child(self.composer.clone());
        }
        ed
    }
}
