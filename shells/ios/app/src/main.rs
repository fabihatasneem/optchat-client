//! OptChat for iPhone and iPad: the app shell. GPUI (latest upstream + gpui_ios, pinned), the engine client
//! (engine.rs), Keychain-stored connection, keyboard avoidance, and the window-shape layout rule:
//! wide (landscape, ≥ 700 pt) → the iPad layout (tasks beside the chat); narrow or portrait → the phone layout.
//! The UI is a placeholder that proves the pipe end to end; the real UI comes from the shared crates.
#[cfg(target_os = "ios")]
mod engine;
#[cfg(target_os = "ios")]
mod text_input;

#[cfg(target_os = "ios")]
mod app {
    use crate::engine::{self, Command, Config, Event};
    use crate::text_input::{self, TextInput};
    use gpui::{
        div, prelude::*, px, rgb, App, Context, Entity, FontWeight, KeyBinding, SharedString, Window, WindowOptions,
    };
    use serde_json::{json, Value};
    use std::sync::mpsc::{self, Receiver, Sender};
    use std::time::Duration;

    const CRED_URL: &str = "https://engine.optchat.reframe";

    #[derive(PartialEq, Clone, Copy)]
    enum Screen { Connect, Tasks, Chat }

    struct TaskRow { id: i64, title: String, status: String, parent: Option<i64> }
    #[derive(Clone)]
    struct Entry { kind: String, text: String }

    gpui::actions!(optchat, [SendMessage]);

    pub struct OptChat {
        cfg: Option<Config>,
        screen: Screen,
        conn: String,
        tasks: Vec<TaskRow>,
        current: i64,
        log: Vec<Entry>,
        live: String,
        running: bool,
        error: Option<String>,
        cmd: Option<Sender<Command>>,
        events: Option<Receiver<Event>>,
        endpoint: Entity<TextInput>,
        workspace: Entity<TextInput>,
        token: Entity<TextInput>,
        tunnel_key: Entity<TextInput>,
        composer: Entity<TextInput>,
        kb_from: f32, kb_to: f32, kb_now: f32, kb_t0: std::time::Instant,
        version: String,
        /// debug builds only: OPTCHAT_AUTOTEST="<task id>|<message>" opens the task and sends once (simulator e2e)
        autotest: Option<(i64, String, u8)>,
        /// the chat feed: a bottom-aligned list (stays pinned to the newest entry as messages arrive)
        feed: gpui::ListState,
        feed_items: std::rc::Rc<std::cell::RefCell<Vec<Entry>>>,
    }

    impl OptChat {
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            let mk = |cx: &mut Context<Self>, p: &str| cx.new(|cx| TextInput::new(cx, p));
            let mk_secret = |cx: &mut Context<Self>, p: &str| cx.new(|cx| { let mut i = TextInput::new(cx, p); i.masked = true; i });
            let s = OptChat {
                cfg: None, screen: Screen::Connect, conn: "not connected".into(), tasks: vec![], current: 0, log: vec![],
                live: String::new(), running: false, error: None, cmd: None, events: None,
                endpoint: mk(cx, "https://chat.tunnel.reframe.sh"), workspace: mk(cx, "workspace"), token: mk_secret(cx, "client token"),
                tunnel_key: mk_secret(cx, "tunnel access key (tunnel only)"), composer: mk(cx, "Message"),
                kb_from: 0., kb_to: 0., kb_now: 0., kb_t0: std::time::Instant::now(), version: bundle_version(),
                feed: gpui::ListState::new(0, gpui::ListAlignment::Bottom, px(600.)),
                feed_items: Default::default(),
                autotest: if cfg!(debug_assertions) { std::env::var("OPTCHAT_AUTOTEST").ok().and_then(|v| { let (t, m) = v.split_once('|')?; Some((t.parse().ok()?, m.to_string(), 0)) }) } else { None },
            };
            // the saved connection (Keychain), or a dev seed passed at launch (written to the Keychain, never logged)
            let seed = std::env::var("OPTCHAT_SEED").ok().and_then(|b| {
                use base64::Engine;
                String::from_utf8(base64::engine::general_purpose::STANDARD.decode(b.trim()).ok()?).ok()
            }).and_then(|s| Config::from_json(&s));
            let read = cx.read_credentials(CRED_URL);
            cx.spawn_in(window, async move |this, cx| {
                let saved = match seed {
                    Some(c) => {
                        // detach: dropping a GPUI Task cancels it
                        let w = this.update(cx, |_, cx| cx.write_credentials(CRED_URL, "engine", c.to_json().as_bytes()));
                        if let Ok(w) = w { let r = w.await; diag(&format!("keychain write: {}", if r.is_ok() { "ok".to_string() } else { format!("{:?}", r.err()) })); }
                        Some(c)
                    }
                    None => {
                        let r = read.await;
                        diag(&format!("keychain read: {}", match &r { Ok(Some(_)) => "found".to_string(), Ok(None) => "none".to_string(), Err(e) => format!("error {e}") }));
                        r.ok().flatten().and_then(|(_, b)| String::from_utf8(b).ok()).and_then(|s| Config::from_json(&s))
                    }
                };
                if let Some(c) = saved { let _ = this.update(cx, |me, cx| me.connect(c, cx)); }
            }).detach();
            // pump engine events into the UI
            cx.spawn_in(window, async move |this, cx| loop {
                cx.background_executor().timer(Duration::from_millis(33)).await;
                if this.update(cx, |me, cx| me.drain(cx)).is_err() { break }
            }).detach();
            s
        }

        fn connect(&mut self, c: Config, cx: &mut Context<Self>) {
            if let Some(tx) = self.cmd.take() { let _ = tx.send(Command::Stop); }
            let (etx, erx) = mpsc::channel();
            let (ctx, crx) = mpsc::channel();
            engine::start(c.clone(), etx, crx);
            self.cfg = Some(c);
            self.cmd = Some(ctx);
            self.events = Some(erx);
            self.tasks.clear(); self.log.clear(); self.error = None;
            self.screen = Screen::Tasks;
            cx.notify();
        }

        fn save_and_connect(&mut self, cx: &mut Context<Self>) {
            let v = |e: &Entity<TextInput>, cx: &Context<Self>| e.read(cx).content.trim().to_string();
            let c = Config { endpoint: v(&self.endpoint, cx), workspace: v(&self.workspace, cx), token: v(&self.token, cx), tunnel_key: v(&self.tunnel_key, cx) };
            if c.endpoint.is_empty() || c.workspace.is_empty() || c.token.is_empty() { self.error = Some("endpoint, workspace and token are required".into()); cx.notify(); return }
            cx.write_credentials(CRED_URL, "engine", c.to_json().as_bytes()).detach();
            self.connect(c, cx);
        }

        fn send(&self, v: Value) { if let Some(tx) = &self.cmd { let _ = tx.send(Command::Send(v)); } }

        fn open_task(&mut self, id: i64, cx: &mut Context<Self>) {
            self.current = id; self.log.clear(); self.live.clear();
            self.feed_items.borrow_mut().clear(); self.feed.reset(0);
            self.send(json!({"type": "open", "task": id}));
            self.screen = Screen::Chat;
            cx.notify();
        }

        fn send_message(&mut self, window: &mut Window, cx: &mut Context<Self>) {
            let text = self.composer.read(cx).content.trim().to_string();
            if text.is_empty() { return }
            self.send(json!({"type": "send", "task": self.current, "text": text, "run": true}));
            self.composer.update(cx, |i, cx| i.reset(window, cx));
        }

        fn drain(&mut self, cx: &mut Context<Self>) {
            let Some(rx) = &self.events else { return };
            let mut changed = false;
            let evs: Vec<Event> = rx.try_iter().collect();
            for e in evs {
                changed = true;
                match e {
                    Event::Status(s) => self.conn = s,
                    Event::Message(m) => self.apply(m),
                }
            }
            if changed { self.autotest_step(cx); cx.notify() }
        }

        fn autotest_step(&mut self, cx: &mut Context<Self>) {
            let Some((task, msg, stage)) = self.autotest.clone() else { return };
            if stage == 0 && !self.tasks.is_empty() {
                self.autotest = Some((task, msg, 1));
                self.open_task(task, cx);
            } else if stage == 1 && self.current == task && !self.log.is_empty() {
                self.autotest = Some((task, msg.clone(), 2));
                if !msg.is_empty() { self.send(json!({"type": "send", "task": task, "text": msg, "run": true})); }
            }
        }

        fn apply(&mut self, m: Value) {
            let row = |t: &Value| TaskRow { id: t["id"].as_i64().unwrap_or(0), title: t["title"].as_str().unwrap_or("").into(),
                status: t["status"].as_str().unwrap_or("").into(), parent: t["parent"].as_i64() };
            match m["type"].as_str().unwrap_or("") {
                "snapshot" => {
                    self.tasks = m["tasks"].as_array().map(|a| a.iter().map(|x| row(&x["task"])).collect()).unwrap_or_default();
                    self.tasks.sort_by_key(|t| (t.parent.is_some(), -t.id));
                    if m["task"].as_i64() == Some(self.current) {
                        self.log = m["log"].as_array().map(|a| a.iter().map(|e| Entry { kind: e["kind"].as_str().unwrap_or("").into(), text: e["text"].as_str().unwrap_or("").into() }).collect()).unwrap_or_default();
                        self.running = !m["running"].is_null();
                    }
                }
                "task" => {
                    let t = row(&m["task"]);
                    match self.tasks.iter_mut().find(|x| x.id == t.id) { Some(x) => *x = t, None => self.tasks.push(t) }
                }
                "log" if m["task"].as_i64() == Some(self.current) => {
                    let e = &m["entry"];
                    self.log.push(Entry { kind: e["kind"].as_str().unwrap_or("").into(), text: e["text"].as_str().unwrap_or("").into() });
                    if e["kind"] == "talk" { self.live.clear() }
                }
                "delta" if m["task"].as_i64() == Some(self.current) && m["kind"] == "text" => self.live.push_str(m["text"].as_str().unwrap_or("")),
                "turn" if m["task"].as_i64() == Some(self.current) => {
                    let st = m["turn"]["status"].as_str().unwrap_or("");
                    self.running = st == "waiting" || st == "running";
                    if !self.running { self.live.clear() }
                }
                "error" => self.error = m["message"].as_str().map(String::from),
                "disabled" => self.error = Some(format!("workspace disabled: {}", m["reason"].as_str().unwrap_or(""))),
                _ => {}
            }
        }

        // ---- views ------------------------------------------------------------------------------------------
        fn field(label: &str, input: &Entity<TextInput>) -> impl IntoElement {
            div().flex().flex_col().gap(px(4.)).child(div().text_size(px(13.)).text_color(rgb(0x9a9a9a)).child(label.to_string())).child(input.clone())
        }

        fn connect_view(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
            div().id("connect").flex().flex_col().gap(px(14.)).p(px(20.)).max_w(px(560.)).w_full()
                .child(div().text_size(px(24.)).font_weight(FontWeight::BOLD).child("Connect to an engine"))
                .child(Self::field("Endpoint", &self.endpoint))
                .child(Self::field("Workspace", &self.workspace))
                .child(Self::field("Client token (kept in the Keychain)", &self.token))
                .child(Self::field("Tunnel access key (only for *.tunnel.reframe.sh)", &self.tunnel_key))
                .child(button("connect-btn", "Connect").on_click(cx.listener(|me, _, _, cx| me.save_and_connect(cx))))
                .children(self.error.clone().map(|e| div().text_color(rgb(0xff6b6b)).child(e)))
                .child(div().text_size(px(12.)).text_color(rgb(0x707070)).child(format!("OptChat {}", self.version)))
                .into_any_element()
        }

        fn tasks_view(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
            let ws = self.cfg.as_ref().map(|c| c.workspace.clone()).unwrap_or_default();
            div().id("tasks").flex().flex_col().size_full().overflow_y_scroll()
                .child(div().flex().items_center().justify_between().px(px(16.)).py(px(10.))
                    .child(div().flex().flex_col()
                        .child(div().text_size(px(20.)).font_weight(FontWeight::BOLD).child("Tasks"))
                        .child(div().text_size(px(12.)).text_color(rgb(0x8a8a8a)).child(format!("{ws} · {}", self.conn))))
                    .child(button("settings", "Engine").on_click(cx.listener(|me, _, _, cx| { me.screen = Screen::Connect; cx.notify() }))))
                .children(self.tasks.iter().map(|t| {
                    let id = t.id;
                    let title = if t.parent.is_none() { "main chat".to_string() } else { format!("#{} {}", t.id, t.title) };
                    let sel = id == self.current && self.screen == Screen::Chat;
                    div().id(("task", id as u64)).px(px(16.)).py(px(10.)).border_b_1().border_color(rgb(0x1f1f1f))
                        .bg(rgb(if sel { 0x1a2a40 } else { 0x000000 }))
                        .child(div().text_size(px(16.)).child(title))
                        .child(div().text_size(px(12.)).text_color(rgb(0x8a8a8a)).child(t.status.clone()))
                        .on_click(cx.listener(move |me, _, _, cx| me.open_task(id, cx)))
                }))
                .into_any_element()
        }

        fn chat_view(&self, back: bool, cx: &mut Context<Self>) -> gpui::AnyElement {
            let title = self.tasks.iter().find(|t| t.id == self.current).map(|t| if t.parent.is_none() { "main chat".to_string() } else { format!("#{} {}", t.id, t.title) }).unwrap_or_else(|| format!("#{}", self.current));
            // feed items = the log + the streaming reply; the list is re-measured when they change
            let mut items: Vec<Entry> = self.log.clone();
            if !self.live.is_empty() { items.push(Entry { kind: "talk".into(), text: self.live.clone() }); }
            let changed = { let cur = self.feed_items.borrow(); cur.len() != items.len() || cur.last().map(|e| &e.text) != items.last().map(|e| &e.text) };
            if changed {
                let n = items.len();
                let old = self.feed_items.borrow().len();
                *self.feed_items.borrow_mut() = items;
                if n >= old && old > 0 { self.feed.splice(old.saturating_sub(1)..old, n - old + 1) } else { self.feed.reset(n) }
            }
            let shared = self.feed_items.clone();
            let feed = gpui::list(self.feed.clone(), move |ix, _, _| {
                let items = shared.borrow();
                div().px(px(16.)).py(px(4.)).child(entry_view(&items[ix])).into_any_element()
            });
            div().flex().flex_col().size_full()
                .child(div().flex().items_center().gap(px(10.)).px(px(16.)).py(px(10.)).border_b_1().border_color(rgb(0x1f1f1f))
                    .children(back.then(|| button("back", "‹ Tasks").on_click(cx.listener(|me, _, _, cx| { me.screen = Screen::Tasks; cx.notify() }))))
                    .child(div().text_size(px(17.)).font_weight(FontWeight::BOLD).child(title))
                    .children(self.running.then(|| div().text_size(px(12.)).text_color(rgb(0x8a8a8a)).child("working…"))))
                .child(feed.flex_grow(1.).w_full())
                .children(self.error.clone().map(|e| div().px(px(16.)).text_color(rgb(0xff6b6b)).child(e)))
                .child(div().flex().items_center().gap(px(8.)).px(px(12.)).py(px(8.)).bg(rgb(0x0f0f0f))
                    .child(div().flex_1().min_w(px(0.)).child(self.composer.clone()))
                    .child(button("send", "Send").on_click(cx.listener(|me, _, window, cx| me.send_message(window, cx)))))
                .into_any_element()
        }
    }

    fn button(id: &'static str, label: &'static str) -> gpui::Stateful<gpui::Div> {
        div().id(id).px(px(12.)).py(px(8.)).rounded(px(8.)).bg(rgb(0x1f3b63)).text_color(rgb(0xffffff)).child(label)
    }

    fn entry_view(e: &Entry) -> gpui::AnyElement {
        let one_line = |s: &str| { let t: String = s.replace('\n', " "); if t.chars().count() > 160 { t.chars().take(160).collect::<String>() + "…" } else { t } };
        match e.kind.as_str() {
            "user" | "note" => div().flex().justify_end().child(div().max_w(gpui::relative(0.8)).px(px(12.)).py(px(8.)).rounded(px(12.)).bg(rgb(0x1a406f)).child(e.text.clone())).into_any_element(),
            "talk" => div().text_size(px(15.)).child(e.text.clone()).into_any_element(),
            "tool" | "echo" => div().text_size(px(12.)).font_family("Lilex").text_color(rgb(0x8a8a8a)).child(one_line(&e.text)).into_any_element(),
            _ => div().text_size(px(13.)).text_color(rgb(0x9a9a9a)).child(one_line(&e.text)).into_any_element(),
        }
    }

    impl Render for OptChat {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            // keyboard avoidance: follow the IME inset with a 250 ms ease-out (UIKit's keyboard curve is similar)
            let insets = window.insets();
            let target = insets.effective().bottom.as_f32();
            if (target - self.kb_to).abs() > 0.5 { self.kb_from = self.kb_now; self.kb_to = target; self.kb_t0 = std::time::Instant::now(); }
            let p = (self.kb_t0.elapsed().as_secs_f32() / 0.25).clamp(0., 1.);
            self.kb_now = self.kb_from + (self.kb_to - self.kb_from) * (1. - (1. - p).powi(3));
            if p < 1. { window.request_animation_frame(); }

            // the window-shape rule
            let vp = window.viewport_size();
            let wide = vp.width > vp.height && vp.width >= px(700.);
            let body = if self.cfg.is_none() || self.screen == Screen::Connect {
                div().flex().justify_center().size_full().child(self.connect_view(cx)).into_any_element()
            } else if wide {
                div().flex().size_full()
                    .child(div().w(px(320.)).h_full().border_r_1().border_color(rgb(0x1f1f1f)).child(self.tasks_view(cx)))
                    .child(div().flex_1().min_w(px(0.)).h_full().child(self.chat_view(false, cx)))
                    .into_any_element()
            } else if self.screen == Screen::Chat {
                self.chat_view(true, cx)
            } else {
                self.tasks_view(cx)
            };
            div().key_context("OptChat").on_action(cx.listener(|me, _: &SendMessage, window, cx| me.send_message(window, cx)))
                .size_full().flex().flex_col().bg(rgb(0x000000)).text_color(rgb(0xebebeb))
                .pt(insets.safe_area.top).pb(px(self.kb_now)).pl(insets.safe_area.left).pr(insets.safe_area.right)
                .child(body)
        }
    }

    /// a line in Documents/creds.log (outcomes only, never values)
    fn diag(line: &str) {
        if let Ok(home) = std::env::var("HOME") {
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(std::path::Path::new(&home).join("Documents/creds.log")) { let _ = writeln!(f, "{line}"); }
        }
    }

    fn footprint_mb() -> f64 {
        unsafe {
            let mut info: mach2::task_info::task_vm_info = std::mem::zeroed();
            let mut count = (std::mem::size_of::<mach2::task_info::task_vm_info>() / 4) as u32;
            let kr = mach2::task::task_info(mach2::traps::mach_task_self(), mach2::task_info::TASK_VM_INFO, &mut info as *mut _ as *mut i32, &mut count);
            if kr == 0 { info.phys_footprint as f64 / 1_048_576. } else { -1. }
        }
    }

    fn bundle_version() -> String {
        use objc2::runtime::{AnyClass, AnyObject};
        unsafe {
            let Some(cls) = AnyClass::get(c"NSBundle") else { return "?".into() };
            let b: *mut AnyObject = objc2::msg_send![cls, mainBundle];
            let get = |k: &str| -> String {
                let key: *mut AnyObject = objc2::msg_send![AnyClass::get(c"NSString").unwrap(), stringWithUTF8String: std::ffi::CString::new(k).unwrap().as_ptr()];
                let v: *mut AnyObject = objc2::msg_send![b, objectForInfoDictionaryKey: key];
                if v.is_null() { return "?".into() }
                let c: *const std::ffi::c_char = objc2::msg_send![v, UTF8String];
                std::ffi::CStr::from_ptr(c).to_string_lossy().into_owned()
            };
            format!("{} ({})", get("CFBundleShortVersionString"), get("CFBundleVersion"))
        }
    }

    pub fn main() {
        gpui_platform::application().run(|cx: &mut App| {
            if let Ok(bytes) = std::fs::read(std::env::current_exe().unwrap().parent().unwrap().join("Lilex-Regular.ttf")) {
                cx.text_system().add_fonts(vec![std::borrow::Cow::Owned(bytes)]).ok();
            }
            cx.bind_keys([
                KeyBinding::new("backspace", text_input::Backspace, None),
                KeyBinding::new("delete", text_input::Delete, None),
                KeyBinding::new("left", text_input::Left, None),
                KeyBinding::new("right", text_input::Right, None),
                KeyBinding::new("enter", SendMessage, None),
            ]);
            gpui_ios::ios::set_status_bar_style(gpui_ios::StatusBarContentStyle::Light);
            // footprint at each lifecycle change (Documents/lifecycle.json): proves the drawables go when backgrounded
            cx.on_app_lifecycle(|phase, _| {
                let line = format!("{:?} {:.1} MB\n", phase, footprint_mb());
                if let Ok(home) = std::env::var("HOME") {
                    use std::io::Write;
                    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(std::path::Path::new(&home).join("Documents/lifecycle.json")) { let _ = f.write_all(line.as_bytes()); }
                }
            }).detach();
            cx.open_window(WindowOptions::default(), |window, cx| cx.new(|cx| OptChat::new(window, cx))).unwrap();
            cx.activate(true);
        });
    }

    #[allow(dead_code)]
    fn _types(_: SharedString) {}
}

#[cfg(target_os = "ios")]
fn main() {
    app::main();
}

#[cfg(not(target_os = "ios"))]
fn main() {
    eprintln!("OptChat iOS: build for an iOS target (app/build.sh).");
}
