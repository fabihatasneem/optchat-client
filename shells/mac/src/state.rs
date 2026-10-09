//! The app's model (app/ui/state.ts): a mirror of the hub's state for the task on screen plus the global bits,
//! fed by `HubEvent`s, and the commands the views send (run on the hub's runtime, never on the UI thread).
use gpui::{App, AppContext, Context, Entity, EventEmitter, Task};
use optchat_engine::engine::{EngineEvent, TaskInfo, ViewInfo};
use optchat_engine::hub::*;
use optchat_engine::tools::ToolResult;
use optchat_engine::types::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum View {
    Tasks,
    Memory,
    Context,
    Log,
    Models,
    Settings,
}
impl View {
    pub const ALL: [View; 6] = [View::Tasks, View::Memory, View::Context, View::Log, View::Models, View::Settings];
    pub fn label(&self) -> &'static str {
        match self {
            View::Tasks => "Tasks",
            View::Memory => "Memory",
            View::Context => "Context",
            View::Log => "Log",
            View::Models => "Models",
            View::Settings => "Settings",
        }
    }
    pub fn icon(&self) -> &'static str {
        match self {
            View::Tasks => "list-tree",
            View::Memory => "brain",
            View::Context => "scan-text",
            View::Log => "scroll-text",
            View::Models => "cpu",
            View::Settings => "settings",
        }
    }
}

/// per-viewer layout preferences (a JSON file next to the database); missing fields take their defaults
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    /// window bounds [x, y, w, h] in points, restored at launch
    pub window: Option<[f32; 4]>,
    pub view: View,
    pub side_open: bool,
    pub side_w: f32,
    pub task_scope_all: bool,
    pub show_ended: bool,
    pub settings_tab_global: bool,
    pub theme: Option<String>,
    pub follow_system: bool,
    pub tabs: Vec<i64>,
    pub current: i64,
    pub font_size: f32,
}
impl Default for Prefs {
    fn default() -> Self {
        Prefs { window: None, view: View::Tasks, side_open: true, side_w: 320.0, task_scope_all: false, show_ended: true, settings_tab_global: false, theme: None, follow_system: true, tabs: vec![0], current: 0, font_size: 14.0 }
    }
}

#[derive(Clone, Debug)]
pub struct ToolState {
    pub call_id: String,
    pub turn_id: String,
    pub name: String,
    pub input: serde_json::Value,
    pub running: bool,
    pub started_at: Option<String>,
    pub result: Option<ToolResult>,
}

#[derive(Clone, Debug, Default)]
pub struct Live {
    pub turn_id: String,
    pub text: String,
    pub thinking: String,
}

/// Everything about the task on screen (replaced on every snapshot).
#[derive(Clone, Default)]
pub struct Cur {
    pub loaded: bool,
    pub log: Vec<Arc<LogEntry>>,
    pub t: i64,
    pub g: i64,
    pub view: Option<ViewInfo>,
    pub nodes: usize,
    pub running: Option<TurnRecord>,
    pub queue: Vec<String>,
    pub turns: HashMap<String, TurnRecord>,
    pub requests: HashMap<String, RequestRecord>,
    pub live: BTreeMap<String, Live>,
    pub tools: HashMap<String, ToolState>,
    pub settings: Option<Settings>,
    pub tool_list: Vec<ToolSummary>,
}

#[derive(Clone, Debug, Default)]
pub struct Dict {
    pub task: i64,
    pub state: String, // idle | connecting | listening | finishing
    pub level: f32,
    pub text: Option<String>,
    pub done: Option<String>,
    pub seq: u64,
    /// a dictation error shows in the composer for 6 s (engine errors stay, like the web app)
    pub error: Option<(String, std::time::Instant)>,
}

pub struct AppState {
    pub hub: Option<Arc<Hub>>,
    pub rt: tokio::runtime::Handle,
    pub error: Option<String>,
    pub tasks: BTreeMap<i64, TaskInfo>,
    pub current: i64,
    pub cur: Cur,
    pub models: Vec<ModelInfo>,
    pub global: GlobalSettings,
    pub busy: Vec<String>,
    pub failed: BTreeMap<String, String>,
    pub node_tick: u64,
    pub approvals: Vec<Approval>,
    pub server_log: VecDeque<String>,
    pub errors: Vec<String>,
    pub notice: Option<(bool, String, std::time::Instant)>,
    pub dict: Dict,
    pub prefs: Prefs,
    pub prefs_path: String,
    pub feed_rev: u64,  // bumps whenever feed rows may change
    pub open_seq: u64,  // task switches
    pub follow: bool,   // autoscroll
    pub selected_request: Option<String>,
    pub pinned_request: bool,
    pub stats: Stats,
    _pump: Option<Task<()>>,
}

#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub launch_ms: f64,
    pub first_frame_ms: Option<f64>,
    pub loaded_ms: Option<f64>,
    /// task switches: from load_task to the first render with the task's data, ms
    pub switch_t0: Option<std::time::Instant>,
    pub switches: Vec<f64>,
    pub events: u64,
}

pub enum StateEvent {
    Opened(i64),
    SpawnedOpen(i64),
    DictText,
    Appended,
}
impl EventEmitter<StateEvent> for AppState {}

impl AppState {
    pub fn new(rt: tokio::runtime::Handle, prefs_path: String) -> AppState {
        let prefs: Prefs = std::fs::read_to_string(&prefs_path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        AppState {
            hub: None,
            rt,
            error: None,
            tasks: BTreeMap::new(),
            current: prefs.current,
            cur: Cur::default(),
            models: vec![],
            global: GlobalSettings::default(),
            busy: vec![],
            failed: BTreeMap::new(),
            node_tick: 0,
            approvals: vec![],
            server_log: VecDeque::new(),
            errors: vec![],
            notice: None,
            dict: Dict { state: "idle".into(), ..Default::default() },
            prefs,
            prefs_path,
            feed_rev: 0,
            open_seq: 0,
            follow: true,
            selected_request: None,
            pinned_request: false,
            stats: Stats::default(),
            _pump: None,
        }
    }
    pub fn save_prefs(&self) {
        if let Ok(s) = serde_json::to_string_pretty(&self.prefs) {
            let _ = std::fs::write(&self.prefs_path, s);
        }
    }

    /// Wire the hub in: snapshot the current task, then apply events as they come (batched per frame).
    pub fn attach(&mut self, hub: Arc<Hub>, mut rx: tokio::sync::mpsc::UnboundedReceiver<HubEvent>, cx: &mut Context<Self>) {
        self.models = hub.models();
        self.global = hub.global.read().clone();
        self.approvals = hub.pending_approvals();
        let (b, f) = hub.compactor_state();
        self.busy = b;
        self.failed = f;
        self.tasks = hub.tasks().into_iter().map(|i| (i.task.id, i)).collect();
        if !self.tasks.contains_key(&self.current) {
            self.current = 0;
        }
        self.prefs.tabs.retain(|t| self.tasks.contains_key(t));
        if !self.prefs.tabs.contains(&self.current) {
            self.prefs.tabs.push(self.current);
        }
        self.hub = Some(hub);
        let cur = self.current;
        self.load_task(cur, cx);
        self._pump = Some(cx.spawn(async move |this, cx| {
            while let Some(first) = rx.recv().await {
                let mut batch = vec![first];
                while let Ok(e) = rx.try_recv() {
                    batch.push(e);
                    if batch.len() > 2000 {
                        break;
                    }
                }
                let ok = this.update(cx, |s, cx| {
                    for e in batch {
                        s.apply(e, cx);
                    }
                    cx.notify();
                });
                if ok.is_err() {
                    break;
                }
            }
        }));
    }

    pub fn hub(&self) -> Option<Arc<Hub>> {
        self.hub.clone()
    }

    /// Run a hub call on its runtime; `done` gets the result back on the UI thread.
    pub fn run<R: Send + 'static>(&self, cx: &mut Context<Self>, f: impl FnOnce(Arc<Hub>) -> R + Send + 'static, done: impl FnOnce(&mut AppState, R, &mut Context<Self>) + 'static) {
        let Some(hub) = self.hub.clone() else { return };
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.rt.spawn(async move {
            let r = tokio::task::spawn_blocking(move || f(hub)).await;
            if let Ok(r) = r {
                let _ = tx.send(r);
            }
        });
        cx.spawn(async move |this, cx| {
            if let Ok(r) = rx.await {
                let _ = this.update(cx, |s, cx| {
                    done(s, r, cx);
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// A command whose outcome is a notice (toast) on error, and optionally on success.
    pub fn command(&self, cx: &mut Context<Self>, toast_ok: bool, f: impl FnOnce(Arc<Hub>) -> Result<String, String> + Send + 'static) {
        self.run(cx, f, move |s, r, _| match r {
            Ok(m) => {
                if toast_ok {
                    s.notice = Some((true, m, std::time::Instant::now()));
                }
            }
            Err(e) => s.notice = Some((false, e, std::time::Instant::now())),
        });
    }

    pub fn load_task(&mut self, id: i64, cx: &mut Context<Self>) {
        self.current = id;
        self.cur = Cur::default();
        self.open_seq += 1;
        self.feed_rev += 1;
        self.follow = true;
        self.pinned_request = false;
        self.selected_request = None;
        if !self.prefs.tabs.contains(&id) {
            self.prefs.tabs.push(id);
        }
        self.prefs.current = id;
        self.save_prefs();
        let seq = self.open_seq;
        let t0 = std::time::Instant::now();
        self.stats.switch_t0 = Some(t0);
        self.run(cx, move |h| h.snapshot(id), move |s, snap, cx| {
            if s.open_seq != seq {
                return;
            }
            if let Some(snap) = snap {
                let ta = std::time::Instant::now();
                s.apply_snapshot(snap);
                if std::env::var("OPTCHAT_TIMING").is_ok() {
                    eprintln!("open #{id}: snapshot arrived {:?}, applied in {:?}", ta.duration_since(t0), ta.elapsed());
                }
                if s.stats.loaded_ms.is_none() {
                    s.stats.loaded_ms = Some(s.stats.launch_ms + t0.elapsed().as_secs_f64() * 1000.0);
                }
                cx.emit(StateEvent::Opened(id));
            }
        });
        cx.notify();
    }

    pub fn open_task(&mut self, id: i64, cx: &mut Context<Self>) {
        if id < 0 {
            return;
        }
        if id == self.current && self.cur.loaded {
            return;
        }
        self.load_task(id, cx);
    }
    pub fn close_tab(&mut self, id: i64, cx: &mut Context<Self>) {
        let k = self.prefs.tabs.iter().position(|t| *t == id).unwrap_or(0);
        self.prefs.tabs.retain(|t| *t != id);
        if self.prefs.tabs.is_empty() {
            self.prefs.tabs.push(0);
        }
        self.save_prefs();
        if id == self.current {
            let next = self.prefs.tabs[k.min(self.prefs.tabs.len() - 1)];
            self.load_task(next, cx);
        }
        cx.notify();
    }

    fn apply_snapshot(&mut self, s: Snapshot) {
        self.tasks = s.tasks.into_iter().map(|i| (i.task.id, i)).collect();
        let mut turns: HashMap<String, TurnRecord> = s.turns.into_iter().map(|t| (t.id.clone(), t)).collect();
        if let Some(r) = &s.running {
            turns.insert(r.id.clone(), r.clone());
        }
        let mut tools: HashMap<String, ToolState> = HashMap::new();
        for t in s.running_tools {
            tools.insert(t.call_id.clone(), ToolState { call_id: t.call_id, turn_id: t.turn_id, name: t.name, input: t.input, running: true, started_at: Some(t.started_at), result: None });
        }
        self.cur = Cur {
            loaded: true,
            log: s.log,
            t: s.t,
            g: s.g,
            view: Some(s.view),
            nodes: s.nodes_count,
            running: s.running,
            queue: s.queue,
            turns,
            requests: s.requests.into_iter().map(|r| (r.id.clone(), r)).collect(),
            live: s.live.into_iter().map(|l| (l.request_id, Live { turn_id: l.turn_id, text: l.text, thinking: l.thinking })).collect(),
            tools,
            settings: Some(s.settings),
            tool_list: s.tools,
        };
        self.feed_rev += 1;
    }

    fn apply(&mut self, e: HubEvent, cx: &mut Context<Self>) {
        self.stats.events += 1;
        let cur = self.current;
        match e {
            HubEvent::Engine(ev) => match ev {
                EngineEvent::Log { task, entry } => {
                    if task == cur && self.cur.loaded && !self.cur.log.iter().rev().take(4).any(|e| e.i == entry.i) {
                        self.cur.t = self.cur.t.max(entry.i + 1);
                        self.cur.log.push(entry);
                        self.feed_rev += 1;
                        cx.emit(StateEvent::Appended);
                    }
                }
                EngineEvent::Node { task, .. } => {
                    if task == cur {
                        self.cur.nodes += 1;
                    }
                    self.node_tick += 1;
                }
                EngineEvent::View { task, view } => {
                    if task == cur {
                        self.cur.g = view.g;
                        self.cur.view = Some(view);
                    }
                }
                EngineEvent::Compactor { busy, failed } => {
                    self.busy = busy;
                    self.failed = failed;
                }
                EngineEvent::Turn { task, turn } => {
                    if task != cur {
                        return;
                    }
                    let active = turn.status == "waiting" || turn.status == "running";
                    if active {
                        self.cur.running = Some(turn.clone());
                    } else if self.cur.running.as_ref().map(|r| r.id == turn.id).unwrap_or(false) {
                        self.cur.running = None;
                        self.cur.live.retain(|_, v| v.turn_id != turn.id);
                    }
                    self.cur.turns.insert(turn.id.clone(), turn);
                    self.feed_rev += 1;
                }
                EngineEvent::Request { task, request } => {
                    if task != cur {
                        return;
                    }
                    if request.status != "running" {
                        self.cur.live.remove(&request.id);
                    }
                    self.cur.requests.insert(request.id.clone(), request);
                    self.feed_rev += 1;
                }
                EngineEvent::Delta { task, turn_id, request_id, thinking, text } => {
                    if task != cur {
                        return;
                    }
                    let l = self.cur.live.entry(request_id).or_insert_with(|| Live { turn_id, ..Default::default() });
                    if thinking {
                        l.thinking.push_str(&text);
                    } else {
                        l.text.push_str(&text);
                    }
                    self.feed_rev += 1;
                }
                EngineEvent::Tool { task, turn_id, call_id, name, input, running, result, started_at } => {
                    if task != cur {
                        return;
                    }
                    let prev = self.cur.tools.get(&call_id).and_then(|t| t.started_at.clone());
                    self.cur.tools.insert(call_id.clone(), ToolState { call_id, turn_id, name, input, running, started_at: started_at.or(prev), result });
                    self.feed_rev += 1;
                }
                EngineEvent::Task { info } => {
                    let id = info.task.id;
                    if id == cur {
                        self.cur.settings = Some(info.task.settings.clone());
                        self.feed_rev += 1;
                    }
                    self.tasks.insert(id, info);
                }
                EngineEvent::Inbox { task, queue } => {
                    if task == cur {
                        self.cur.queue = queue;
                    }
                }
                EngineEvent::Status { .. } => {}
                EngineEvent::Error { task, message } => {
                    let m = match task {
                        Some(t) => format!("#{t}: {message}"),
                        None => message,
                    };
                    self.errors.push(m);
                    if self.errors.len() > 5 {
                        self.errors.remove(0);
                    }
                }
            },
            HubEvent::ServerLog(l) => {
                self.server_log.push_back(l);
                while self.server_log.len() > 300 {
                    self.server_log.pop_front();
                }
            }
            HubEvent::Approval(a) => {
                self.approvals.retain(|x| x.id != a.id);
                self.approvals.push(a);
            }
            HubEvent::ApprovalDone(id) => self.approvals.retain(|x| x.id != id),
            HubEvent::Models(m) => self.models = m,
            HubEvent::Global(g) => self.global = g,
            HubEvent::Dictation { task, ev } => {
                self.dict.task = task;
                match ev {
                    DictEvent::State(s) => self.dict.state = s,
                    DictEvent::Level(l) => self.dict.level = l,
                    DictEvent::Text(t) => {
                        self.dict.text = Some(t);
                        self.dict.seq += 1;
                        cx.emit(StateEvent::DictText);
                    }
                    DictEvent::Error(m) => {
                        self.dict.error = Some((m, std::time::Instant::now()));
                        cx.spawn(async move |this, cx| {
                            gpui::Timer::after(std::time::Duration::from_millis(6050)).await;
                            let _ = this.update(cx, |_, cx| cx.notify());
                        })
                        .detach();
                    }
                    DictEvent::Done { text, .. } => {
                        self.dict.state = "idle".into();
                        self.dict.level = 0.0;
                        self.dict.done = Some(text.clone());
                        self.dict.text = Some(text);
                        self.dict.seq += 1;
                        cx.emit(StateEvent::DictText);
                    }
                }
            }
            HubEvent::Notice { ok, message, spawned } => {
                self.notice = Some((ok, message, std::time::Instant::now()));
                if let Some(id) = spawned {
                    cx.emit(StateEvent::SpawnedOpen(id));
                }
            }
            HubEvent::Ready => {}
        }
    }

    // ---- derived ---------------------------------------------------------------------------------------------
    pub fn info(&self) -> Option<&TaskInfo> {
        self.tasks.get(&self.current)
    }
    pub fn children_of(&self, id: i64) -> Vec<&TaskInfo> {
        self.tasks.values().filter(|x| x.task.parent == Some(id)).collect()
    }
    pub fn ancestors(&self, id: i64) -> Vec<&TaskInfo> {
        let mut out = vec![];
        let mut x = self.tasks.get(&id);
        while let Some(t) = x {
            match t.task.parent.and_then(|p| self.tasks.get(&p)) {
                Some(p) => {
                    out.insert(0, p);
                    x = Some(p);
                }
                None => break,
            }
        }
        out
    }
    pub fn model(&self, id: &str) -> Option<&ModelInfo> {
        self.models.iter().find(|m| m.id == id)
    }
    pub fn last_turn(&self) -> Option<&TurnRecord> {
        self.cur.turns.values().max_by(|a, b| a.started_at.cmp(&b.started_at))
    }
    pub fn latest_request(&self) -> Option<String> {
        self.cur.requests.values().filter(|r| r.purpose == "chat").max_by(|a, b| a.started_at.cmp(&b.started_at)).map(|r| r.id.clone())
    }
    pub fn shown_request(&self) -> Option<String> {
        if self.pinned_request {
            self.selected_request.clone()
        } else {
            self.latest_request()
        }
    }
}

pub fn sub(x: Option<&TaskInfo>, metric: &str) -> i64 {
    x.and_then(|i| i.metrics.get(metric)).and_then(|m| m.get("*")).map(|v| v.subtree).unwrap_or(0)
}
pub fn sub_k(x: Option<&TaskInfo>, metric: &str, kind: &str) -> i64 {
    x.and_then(|i| i.metrics.get(metric)).and_then(|m| m.get(kind)).map(|v| v.subtree).unwrap_or(0)
}
pub fn own(x: Option<&TaskInfo>, metric: &str) -> i64 {
    x.and_then(|i| i.metrics.get(metric)).and_then(|m| m.get("*")).map(|v| v.own).unwrap_or(0)
}
pub fn own_k(x: Option<&TaskInfo>, metric: &str, kind: &str) -> i64 {
    x.and_then(|i| i.metrics.get(metric)).and_then(|m| m.get(kind)).map(|v| v.own).unwrap_or(0)
}
pub fn task_name(t: &TaskRecord) -> String {
    if t.parent.is_none() { "main chat".into() } else { t.title.clone() }
}

pub fn new_state(cx: &mut App, rt: tokio::runtime::Handle, prefs_path: String) -> Entity<AppState> {
    cx.new(|_| AppState::new(rt, prefs_path))
}
