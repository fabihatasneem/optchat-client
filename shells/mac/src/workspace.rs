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
use optchat_proto::types::*;

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
        DictEnter,
        DictAppend,
        OpenMainChat,
        ZoomIn,
        ZoomOut,
        ZoomReset,
        NextTheme,
        ToggleFollowSystem,
        About,
        Minimize,
        ZoomWindow,
        ToggleFullScreen,
        ShowShortcuts,
        OpenDataFolder,
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
        // while dictating the focus may sit outside the (read-only) text field: Enter / ⌘Enter still send
        KeyBinding::new("enter", DictEnter, Some("Workspace && !Input")),
        KeyBinding::new("cmd-enter", DictAppend, Some("Workspace && !Input")),
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
    _ticker: Option<Task<()>>,
    renders: u64,
    focus_desc: &'static str,
    ask_limit: std::rc::Rc<std::cell::Cell<usize>>,
    bounds_seq: u64,
    title: String,
    task_menu: Option<(Entity<gpui_component::menu::PopupMenu>, Point<Pixels>, Subscription)>,
    pub tab_scroll: ScrollHandle,
    pub tab_scrolled_to: Option<i64>,
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
        // like the web's maxLength: 280 for a close result, 2000 otherwise (UTF-16 units, as in JS)
        let ask_limit = std::rc::Rc::new(std::cell::Cell::new(2000usize));
        let lim = ask_limit.clone();
        let ask_input = cx.new(|cx| InputState::new(window, cx).validate(move |t, _| t.encode_utf16().count() <= lim.get()));
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
            match ev {
                tasks_view::TasksEvent::Open(id) => this.open_task(*id, window, cx),
                tasks_view::TasksEvent::Act(id, op) => this.act(*id, op, window, cx),
                tasks_view::TasksEvent::Menu(id, status, pos) => this.open_task_menu(*id, *status, *pos, window, cx),
            }
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
        crate::ui::anim::set_window_active(window.is_window_active());
        crate::ui::set_state_extra("active", serde_json::json!(window.is_window_active()));
        subs.push(cx.observe_window_activation(window, |_, window, _| {
            crate::ui::anim::set_window_active(window.is_window_active());
            crate::ui::set_state_extra("active", serde_json::json!(window.is_window_active()));
        }));
        subs.push(cx.observe_window_appearance(window, |_, window, cx| {
            crate::theme::sync_window(window, cx);
        }));
        subs.push(cx.observe_window_bounds(window, |this: &mut Workspace, window, cx| {
            // remember the window's bounds (saved 800 ms after the last change)
            // origin of the frame, size of the content: what opening a window takes (bounds() includes the titlebar)
            let b = window.bounds();
            let v = window.viewport_size();
            let wb = [f32::from(b.origin.x), f32::from(b.origin.y), f32::from(v.width), f32::from(v.height)];
            this.state.update(cx, |s, _| s.prefs.window = Some(wb));
            this.bounds_seq += 1;
            let seq = this.bounds_seq;
            cx.spawn(async move |this, cx| {
                Timer::after(std::time::Duration::from_millis(800)).await;
                let _ = this.update(cx, |w, cx| {
                    if w.bounds_seq == seq {
                        w.state.update(cx, |s, _| s.save_prefs());
                    }
                });
            })
            .detach();
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
            if let Some((menu, _, _)) = &this.task_menu {
                // an open task menu keeps the focus (so Esc and the arrows reach it)
                let m = menu.clone();
                m.update(cx, |m, cx| m.focus_handle(cx).focus(window));
                return;
            }
            let input = match &this.overlay {
                Some(Overlay::Palette(p)) => Some(p.read(cx).input()),
                Some(Overlay::NewTask(d)) => Some(d.read(cx).input()),
                _ if this.ask.is_some() => Some(this.ask_input.clone()),
                _ => None,
            };
            match input {
                Some(i) => i.update(cx, |i, cx| i.focus(window, cx)),
                None => window.focus(&this.focus),
            }
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
            narrow: f32::from(window.bounds().size.width) < 760.0,
            sheet: false,
            m_press: None,
            _subs: subs,
            _ticker: None,
            renders: 0,
            focus_desc: "",
            ask_limit,
            bounds_seq: 0,
            title: String::new(),
            task_menu: None,
            tab_scroll: ScrollHandle::new(),
            tab_scrolled_to: None,
        }
    }

    /// The app state for UI tests (OPTCHAT_PROBES): written at every render and every 250 ms, so it stays fresh
    /// while macOS skips drawing an occluded window.
    pub fn publish_state(&self, cx: &App) {
            let s = self.state.read(cx);
            let th = cx.global::<crate::theme::Themes>();
            let last_talk = s.cur.log.iter().rev().find(|e| e.kind == LogKind::Talk).map(|e| e.text.clone());
            let j = serde_json::json!({
                "ready": s.hub.is_some(), "current": s.current, "loaded": s.cur.loaded, "status": s.info().map(|i| i.task.status.as_str()),
                "running": s.cur.running.as_ref().map(|r| r.status.clone()), "t": s.cur.t, "lastTalk": last_talk,
                "lastKinds": s.cur.log.iter().rev().take(8).map(|e| e.kind.as_str()).collect::<Vec<_>>(),
                "tabs": s.prefs.tabs, "view": s.prefs.view, "fontSize": s.prefs.font_size, "settingsGlobal": s.prefs.settings_tab_global, "followSystem": s.prefs.follow_system, "narrow": self.narrow, "pid": std::process::id(), "frame": self.renders, "conn": format!("{:?}", s.conn).to_lowercase(), "reconnects": s.hub().map(|h| h.reconnects.load(std::sync::atomic::Ordering::SeqCst)).unwrap_or(0), "instance": s.instance, "hubBuild": s.build, "focus": self.focus_desc, "taskMenu": self.task_menu.is_some(), "askText": self.ask_input.read(cx).value().to_string(), "window": s.prefs.window,  "placeholderTask": self.composer.read(cx).task,  "selectedRequest": s.selected_request, "sideOpen": self.side_open(cx), "theme": th.active, "overlay": self.overlay.as_ref().map(|o| match o { Overlay::NewTask(_) => "newtask", Overlay::Palette(_) => "palette", Overlay::Metrics(_) => "metrics", Overlay::Lightbox(_) => "lightbox" }),
                "ask": self.ask.as_ref().map(|a| a.op), "dict": s.dict.state, "approvals": s.approvals.len(), "notice": s.notice.as_ref().map(|n| n.1.clone()),
                "model": s.cur.settings.as_ref().map(|x| x.model.clone()), "effort": s.cur.settings.as_ref().map(|x| x.effort.clone()), "tasks": s.tasks.len(),
                "queue": s.cur.queue.len(), "unrun": s.info().and_then(|i| i.task.unrun), "draft": self.composer.read(cx).input.read(cx).value().to_string(),
                "firstFrameMs": s.stats.first_frame_ms, "frames": frame_summary(), "rss": crate::ui::rss_mb(), "loadedMs": s.stats.loaded_ms, "switches": s.stats.switches, "switchesFull": s.stats.switches_full, "errors": s.errors.clone(),
            });
            set_state_json(j.to_string());
            set_state_extra("ws", serde_json::json!({"sideW": s.prefs.side_w, "maxTurns": s.global.max_turns, "running2": s.tasks.values().filter(|x| x.running).count(), "waiting": s.tasks.values().filter(|x| x.waiting).map(|x| x.task.id).collect::<Vec<_>>(), "taskStatus": s.tasks.values().map(|x| (x.task.id.to_string(), x.task.status.as_str())).collect::<std::collections::BTreeMap<_, _>>(), "pinned": s.pinned_request, "unbuilt": s.cur.view.as_ref().map(|v| v.parts.iter().filter(|p| !p.built).count()), "busy": s.busy.len(), "failed": s.failed.len(), "dictDone": s.dict.done_count, "dictText": s.dict.text.clone()}));
    }

    /// Redraws ambient animations (spinners, pulsing dots) at 12.5 Hz (2.5 Hz while the window is in the
    /// background), only in the views that drew one. Every redraw re-encodes the whole window, so this is what
    /// an idle window with a "summarizing" spinner costs.
    pub fn start_ticker(&mut self, cx: &mut Context<Self>) {
        use crate::ui::anim;
        if probes_on() {
            cx.spawn(async move |this, cx| loop {
                Timer::after(std::time::Duration::from_millis(250)).await;
                if this.update(cx, |w, cx| w.publish_state(cx)).is_err() {
                    break;
                }
            })
            .detach();
        }
        self._ticker = Some(cx.spawn(async move |this, cx| loop {
            let ms = if anim::window_active() { 80 } else { 400 };
            Timer::after(std::time::Duration::from_millis(ms)).await;
            let w = anim::take();
            if w == 0 {
                if this.upgrade().is_none() {
                    break;
                }
                continue;
            }
            let alive = this.update(cx, |ws, cx| {
                if w & anim::WORKSPACE != 0 {
                    cx.notify();
                }
                if w & anim::FEED != 0 {
                    ws.feed.update(cx, |_, cx| cx.notify());
                }
                if w & anim::TASKS != 0 {
                    ws.tasks_panel.update(cx, |_, cx| cx.notify());
                }
                if w & anim::PANEL != 0 {
                    ws.memory.update(cx, |_, cx| cx.notify());
                    ws.context.update(cx, |_, cx| cx.notify());
                    ws.log.update(cx, |_, cx| cx.notify());
                    ws.models.update(cx, |_, cx| cx.notify());
                    ws.settings.update(cx, |_, cx| cx.notify());
                }
            });
            if alive.is_err() {
                break;
            }
        }));
    }

    pub fn update_title(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let s = self.state.read(cx);
        let t = match s.info() {
            Some(i) => format!("{}{} · OptChat", task_name(&i.task), if i.task.id != 0 { format!(" · #{}", i.task.id) } else { String::new() }),
            None => "OptChat".into(),
        };
        if t != self.title {
            window.set_window_title(&t);
            self.title = t;
        }
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

    /// A task's context menu at the mouse (task rows): drawn by the workspace above everything.
    pub fn open_task_menu(&mut self, id: i64, status: TaskStatus, pos: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let ws = cx.entity().downgrade();
        let act: std::rc::Rc<dyn Fn(i64, &'static str, &mut Window, &mut App)> = std::rc::Rc::new(move |id, op, window, cx| {
            let _ = ws.update(cx, |w, cx| {
                w.close_task_menu(cx);
                if op == "open" { w.open_task(id, window, cx) } else { w.act(id, op, window, cx) }
            });
        });
        let menu = gpui_component::menu::PopupMenu::build(window, cx, move |m, _, _| task_menu(m, id, status, act));
        let sub = cx.subscribe_in(&menu, window, |w: &mut Workspace, _, _: &DismissEvent, _, cx| {
            w.close_task_menu(cx);
        });
        self.tasks_panel.update(cx, |p, cx| {
            p.menu_open = true;
            cx.notify();
        });
        menu.update(cx, |m, cx| m.focus_handle(cx).focus(window));
        self.task_menu = Some((menu, pos, sub));
        cx.notify();
    }

    pub fn close_task_menu(&mut self, cx: &mut Context<Self>) {
        self.task_menu = None;
        self.tasks_panel.update(cx, |p, cx| {
            p.menu_open = false;
            cx.notify();
        });
        cx.notify();
    }
    pub fn task_menu_open(&self) -> bool {
        self.task_menu.is_some()
    }

    /// A task action from a context menu: open the task, then the same ask bar (or dialog) as its tab actions.
    pub fn act(&mut self, id: i64, op: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.open_task(id, window, cx);
        match op {
            "new" => crate::dialogs::open_new_task(self, Some(("handoff".into(), None, id)), window, cx),
            "pause" | "resume" | "close" | "cancel" => self.start_ask(op, window, cx),
            _ => {}
        }
    }

    pub fn start_ask(&mut self, op: &str, window: &mut Window, cx: &mut Context<Self>) {
        let a = ask_of(op);
        self.ask_limit.set(if op == "close" { 280 } else { 2000 });
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
            s.command(cx, true, move |h| async move { match op {
                "pause" => h.pause(task, &val).await,
                "resume" => h.resume(task, &val, vec![]).await,
                "cancel" => h.cancel(task, &val).await,
                _ => h.close(task, &val).await,
            } })
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
        crate::ui::frame_begin();
        crate::ui::anim::area(crate::ui::anim::WORKSPACE);
        if self._ticker.is_none() {
            self.start_ticker(cx);
        }
        let k = c(cx).clone();
        if self.state.read(cx).stats.first_frame_ms.is_none() {
            let launch = crate::LAUNCH.get().map(|t| t.elapsed().as_secs_f64() * 1000.0).unwrap_or(0.0);
            self.state.update(cx, |s, _| s.stats.first_frame_ms = Some(launch));
        }
        if self.state.read(cx).cur.loaded && self.state.read(cx).stats.switch_t0.is_some() {
            self.state.update(cx, |s, _| {
                if let Some(t0) = s.stats.switch_t0.take() {
                    let ms = t0.elapsed().as_secs_f64() * 1000.0;
                    if std::env::var("OPTCHAT_TIMING").is_ok() {
                        eprintln!("open #{}: first render with data at {ms:.1} ms", s.current);
                    }
                    s.stats.switches.push(ms);
                }
            });
        }
        // nothing focused (a finished task has no composer, a dialog closed): keep the workspace's keys working
        let ended_now = self.state.read(cx).info().map(|i| i.task.status.terminal()).unwrap_or(false);
        if self.overlay.is_none() && self.ask.is_none() && (window.focused(cx).is_none() || (ended_now && !self.focus.contains_focused(window, cx))) {
            window.focus(&self.focus);
        }
        self.renders = crate::ui::frame_count();
        self.update_title(window, cx); // a rename shows at once
        if probes_on() {
            let ci = self.composer.read(cx).input.read(cx).focus_handle(cx);
            self.focus_desc = if window.focused(cx).is_none() {
                "none"
            } else if ci.is_focused(window) {
                "composer"
            } else if self.focus.is_focused(window) {
                "workspace"
            } else if self.focus.contains_focused(window, cx) {
                "inside"
            } else {
                "outside"
            };
        }
        if probes_on() {
            self.publish_state(cx);
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
                w.state.update(cx, |s, cx| s.command(cx, false, move |h| async move { { h.stop(t); Ok(String::new()) } }));
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
            .on_action(cx.listener(|_, _: &Minimize, window, _| window.minimize_window()))
            .on_action(cx.listener(|_, _: &ZoomWindow, window, _| window.zoom_window()))
            .on_action(cx.listener(|_, _: &ToggleFullScreen, window, _| window.toggle_fullscreen()))
            .on_action(cx.listener(|w, _: &ShowShortcuts, window, cx| dialogs::open_palette(w, false, window, cx)))
            .on_action(cx.listener(|w, _: &About, window, cx| {
                let s = w.state.read(cx);
                let detail = format!(
                    "App {}\nConnected to the OptChat hub {} (version {}, build {}).\nA native client for OptChat's endless, task-tree chat, in Rust and GPUI.\nThemes from Zed (see THEMES.md).",
                    env!("CARGO_PKG_VERSION"),
                    s.hub().map(|h| h.cfg.label()).unwrap_or_default(),
                    s.version,
                    s.build
                );
                let _ = window.prompt(PromptLevel::Info, "OptChat for Mac", Some(&detail), &["OK"], cx);
            }))
            .on_action(cx.listener(|w, _: &OpenDataFolder, _, cx| {
                // the folder that holds this chat's database (and the app's prefs and logs)
                let dir = std::path::Path::new(&w.state.read(cx).prefs_path).parent().map(|p| p.to_path_buf());
                if let Some(d) = dir {
                    cx.reveal_path(&d);
                }
            }))
            .on_action(cx.listener(|w, _: &Dismiss, window, cx| {
                if w.task_menu.is_some() {
                    w.close_task_menu(cx);
                    w.composer.update(cx, |c, cx| c.focus(window, cx));
                    return;
                }
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
            .on_action(cx.listener(|w, _: &DictEnter, window, cx| {
                if w.state.read(cx).dict.state == "idle" {
                    return cx.propagate();
                }
                w.composer.update(cx, |c, cx| c.submit(true, window, cx));
            }))
            .on_action(cx.listener(|w, _: &DictAppend, window, cx| {
                if w.state.read(cx).dict.state == "idle" {
                    return cx.propagate();
                }
                w.composer.update(cx, |c, cx| {
                    let run = c.running(cx);
                    c.submit(run, window, cx)
                });
            }))
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
        if let Some((menu, pos, _)) = &self.task_menu {
            root = root.child(deferred(anchored().position(*pos).snap_to_window_with_margin(px(8.)).child(menu.clone())).with_priority(2));
        }
        root = root.child(frame_probe());

        // ---- overlays
        if let Some(o) = self.overlay.clone() {
            // dialogs and popovers fade in (120 ms); the state goes with the overlay, so each opening plays it
            let kind = match &o { Overlay::NewTask(_) => "fade-new", Overlay::Palette(_) => "fade-pal", Overlay::Metrics(_) => "fade-met", Overlay::Lightbox(_) => "fade-light" };
            let el = dialogs::render_overlay(self, o, window, cx);
            root = root.child(
                div()
                    .absolute()
                    .inset_0()
                    .child(el)
                    .with_animation(kind, Animation::new(std::time::Duration::from_millis(120)).with_easing(ease_out_quint()), |d, t| d.opacity(t)),
            );
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
        set_state_extra("rail", serde_json::json!({ "badge": active, "memDot": compacting }));
        if active > 0 {
            crate::ui::seen("railSeen", "badge");
        }
        if compacting {
            crate::ui::seen("railSeen", "memDot");
        }
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

    fn sidebar(&mut self, view: View, w: f32, narrow: bool, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
            head = head.child(btn_small("follow", "follow latest", Tone::Ghost, cx).child(probe("ctx.follow")).on_click(cx.listener(|w, _, _, cx| {
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
            .child(probe("side.resizer"))
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

/// The task context menu (task rows and tabs): the web app's task actions for that task's status, then Copy #id.
pub fn task_menu(m: gpui_component::menu::PopupMenu, id: i64, status: TaskStatus, act: std::rc::Rc<dyn Fn(i64, &'static str, &mut Window, &mut App)>) -> gpui_component::menu::PopupMenu {
    use gpui_component::menu::PopupMenuItem;
    let term = status.terminal();
    let mut m = m;
    let item = |m: gpui_component::menu::PopupMenu, label: &str, op: &'static str| {
        let act = act.clone();
        m.item(PopupMenuItem::new(label.to_string()).on_click(move |_, window, cx| act(id, op, window, cx)))
    };
    m = item(m, "Open", "open");
    if !term && status != TaskStatus::Paused && status != TaskStatus::Pending {
        m = item(m, "Pause…", "pause");
    }
    if status == TaskStatus::Paused {
        m = item(m, "Resume…", "resume");
    }
    if id != 0 && !term {
        m = item(m, "Close as done…", "close");
        m = item(m, "Cancel…", "cancel");
    }
    if !term {
        m = m.separator();
        m = item(m, "New subtask…", "new");
    }
    m.separator().item(PopupMenuItem::new(format!("Copy #{id}")).on_click(move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(format!("#{id}")))))
}
