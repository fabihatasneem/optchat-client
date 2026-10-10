//! The app's connection to the OptChat hub (app/ws/serve.ts, lib/hub.ts): ONE websocket, like the browser's page.
//! A snapshot of the task on screen arrives on connect and after every `open`; then the hub's events; commands go up
//! (send / cancel / stopTool / settings / global / control / spawn / approve) and rpc calls come back by id.
//! Reconnects with backoff (0.4 s doubling to 5 s) and resnapshots. Images come from GET /blob/<sha>; each dictation
//! is a second, short-lived socket to /stt. Every URL carries `db=t<id>` on a test instance.
//! TLS: the macOS trust store (native-tls / Security.framework), verification always on.
use futures_util::{SinkExt, StreamExt};
use optchat_proto::api::*;
use optchat_proto::rates::Per1M;
use optchat_proto::types::*;
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message as WsMsg;

/// Where the hub is: OPTCHAT_HUB (default https://chat.reframe.dev) and OPTCHAT_HUB_DB (a test instance, t<id>).
#[derive(Clone, Debug)]
pub struct HubCfg {
    pub base: String,
    pub db: Option<String>,
}
impl HubCfg {
    pub fn from_env() -> Result<HubCfg, String> {
        let base = std::env::var("OPTCHAT_HUB").unwrap_or_else(|_| "https://chat.reframe.dev".into()).trim_end_matches('/').to_string();
        if !base.starts_with("https://") {
            return Err(format!("OPTCHAT_HUB must be an https URL (got {base})"));
        }
        let db = std::env::var("OPTCHAT_HUB_DB").ok().filter(|d| !d.is_empty());
        if let Some(d) = &db {
            let ok = d.len() >= 2 && d.len() <= 25 && d.starts_with('t') && d[1..].chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
            if !ok {
                return Err(format!("OPTCHAT_HUB_DB must be a test instance like t<id> (got {d})"));
            }
        }
        Ok(HubCfg { base, db })
    }
    fn host(&self) -> &str {
        self.base.trim_start_matches("https://")
    }
    pub fn origin(&self) -> String {
        format!("https://{}", self.host())
    }
    fn query(&self, task: Option<i64>) -> String {
        let mut q = vec![];
        if let Some(t) = task {
            q.push(format!("task={t}"));
        }
        if let Some(d) = &self.db {
            q.push(format!("db={d}"));
        }
        if q.is_empty() {
            String::new()
        } else {
            format!("?{}", q.join("&"))
        }
    }
    pub fn ws_url(&self, path: &str, task: i64) -> String {
        format!("wss://{}{path}{}", self.host(), self.query(Some(task)))
    }
    pub fn blob_url(&self, sha: &str) -> String {
        format!("{}/blob/{sha}{}", self.base, self.query(None))
    }
    /// what the status bar and settings show: the hub and the instance
    pub fn label(&self) -> String {
        match &self.db {
            Some(d) => format!("{} · {d}", self.host()),
            None => self.host().to_string(),
        }
    }
}

pub struct Blob {
    pub media_type: String,
    pub bytes: Vec<u8>,
}

type RpcReply = oneshot::Sender<Result<Value, String>>;
type ControlReply = oneshot::Sender<(bool, String, Option<i64>)>;

pub struct Client {
    pub cfg: HubCfg,
    events: mpsc::UnboundedSender<HubEvent>,
    out: Mutex<Option<mpsc::UnboundedSender<WsMsg>>>,
    rpcs: Mutex<HashMap<u64, RpcReply>>,
    controls: Mutex<HashMap<u64, ControlReply>>,
    next: AtomicU64,
    task: AtomicI64,
    stopped: AtomicBool,
    http: reqwest::Client,
    dict: Mutex<Option<Arc<DictSession>>>,
    /// reconnects so far (status bar, tests)
    pub reconnects: AtomicU64,
}

impl Client {
    /// Connect (and keep reconnecting) on the current tokio runtime; events go to `events`.
    pub fn start(cfg: HubCfg, task: i64, events: mpsc::UnboundedSender<HubEvent>) -> Arc<Client> {
        let c = Arc::new(Client {
            cfg,
            events,
            out: Mutex::new(None),
            rpcs: Mutex::new(HashMap::new()),
            controls: Mutex::new(HashMap::new()),
            next: AtomicU64::new(1),
            task: AtomicI64::new(task),
            stopped: AtomicBool::new(false),
            http: reqwest::Client::builder().timeout(Duration::from_secs(30)).build().expect("http client"),
            dict: Mutex::new(None),
            reconnects: AtomicU64::new(0),
        });
        let me = c.clone();
        tokio::spawn(async move { me.run().await });
        c
    }

    fn emit(&self, e: HubEvent) {
        let _ = self.events.send(e);
    }

    async fn run(self: Arc<Self>) {
        let mut retry: u32 = 0;
        let mut first = true;
        while !self.stopped.load(Ordering::SeqCst) {
            self.emit(HubEvent::Conn(Conn::Connecting));
            let url = self.cfg.ws_url("/ws", self.task.load(Ordering::SeqCst));
            match self.connect(&url).await {
                Ok(ws) => {
                    retry = 0;
                    if !first {
                        self.reconnects.fetch_add(1, Ordering::SeqCst);
                    }
                    first = false;
                    self.emit(HubEvent::Conn(Conn::Open));
                    let (mut w, mut r) = ws.split();
                    let (tx, mut rx) = mpsc::unbounded_channel::<WsMsg>();
                    *self.out.lock() = Some(tx);
                    let writer = tokio::spawn(async move {
                        while let Some(m) = rx.recv().await {
                            if w.send(m).await.is_err() {
                                break;
                            }
                        }
                        let _ = w.close().await;
                    });
                    while let Some(msg) = r.next().await {
                        match msg {
                            Ok(WsMsg::Text(t)) => self.on_message(t.as_str()),
                            Ok(WsMsg::Close(_)) | Err(_) => break,
                            _ => {}
                        }
                    }
                    *self.out.lock() = None;
                    writer.abort();
                    for (_, tx) in self.rpcs.lock().drain() {
                        let _ = tx.send(Err("disconnected from the hub".into()));
                    }
                    for (_, tx) in self.controls.lock().drain() {
                        let _ = tx.send((false, "disconnected from the hub".into(), None));
                    }
                    self.emit(HubEvent::Conn(Conn::Closed));
                }
                Err(e) => {
                    self.emit(HubEvent::Conn(Conn::Closed));
                    self.emit(HubEvent::ServerLog(format!("{} hub connection failed: {e}", chrono::Local::now().format("%H:%M:%S"))));
                }
            }
            if self.stopped.load(Ordering::SeqCst) {
                break;
            }
            // like the page: 0.4 s, doubling, at most 5 s
            let ms = (400u64 << retry.min(4)).min(5000);
            retry += 1;
            tokio::time::sleep(Duration::from_millis(ms)).await;
        }
    }

    async fn connect(&self, url: &str) -> Result<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, String> {
        let mut req = url.into_client_request().map_err(|e| e.to_string())?;
        // the endpoint is same-origin only: Origin must be https://<host>
        req.headers_mut().insert("Origin", self.cfg.origin().parse().map_err(|_| "bad origin".to_string())?);
        let (ws, _) = tokio::time::timeout(Duration::from_secs(15), tokio_tungstenite::connect_async(req)).await.map_err(|_| "timed out".to_string())?.map_err(|e| e.to_string())?;
        Ok(ws)
    }

    fn on_message(&self, text: &str) {
        let Ok(m) = serde_json::from_str::<Value>(text) else { return };
        match m["type"].as_str() {
            Some("rpc") => {
                if let Some(id) = m["id"].as_u64() {
                    if let Some(tx) = self.rpcs.lock().remove(&id) {
                        let r = match m.get("error").and_then(|e| e.as_str()) {
                            Some(e) => Err(e.to_string()),
                            None => Ok(m["result"].clone()),
                        };
                        let _ = tx.send(r);
                    }
                }
            }
            Some("controlDone") => {
                let ok = m["ok"].as_bool().unwrap_or(false);
                let message = m["message"].as_str().unwrap_or("").to_string();
                let spawned = m["spawned"].as_i64();
                // a command of ours waits for it; other clients' outcomes are not broadcast (the hub replies to one socket)
                let waiting = m["id"].as_u64().and_then(|id| self.controls.lock().remove(&id));
                match waiting {
                    Some(tx) => {
                        let _ = tx.send((ok, message, spawned));
                    }
                    None => self.emit(HubEvent::Notice { ok, message, spawned }),
                }
            }
            Some("snapshot") => {
                if let Some(s) = Snapshot::parse(&m) {
                    self.emit(HubEvent::Snapshot(Box::new(s)));
                }
            }
            _ => {
                if let Some(e) = parse_event(&m) {
                    self.emit(e);
                }
            }
        }
    }

    fn send_json(&self, v: Value) -> Result<(), String> {
        match self.out.lock().as_ref() {
            Some(tx) => tx.send(WsMsg::text(v.to_string())).map_err(|_| "not connected to the hub".to_string()),
            None => Err("not connected to the hub".into()),
        }
    }

    /// tests: an event through the app's own path (AppState::apply), as if the hub had sent it
    pub fn inject(&self, e: HubEvent) {
        self.emit(e);
    }
    /// an image from the hub's database, or None
    pub async fn get_blob(&self, sha: &str) -> Option<Blob> {
        self.blob(sha).await.ok().map(|(media_type, bytes)| Blob { media_type, bytes })
    }

    /// tests: close the socket as a network drop would; the client reconnects and resnapshots
    pub fn drop_connection(&self) {
        if let Some(tx) = self.out.lock().as_ref() {
            let _ = tx.send(WsMsg::Close(None));
        }
    }

    pub fn current_task(&self) -> i64 {
        self.task.load(Ordering::SeqCst)
    }
    pub fn shutdown(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        if let Some(tx) = self.out.lock().take() {
            let _ = tx.send(WsMsg::Close(None));
        }
        if let Some(d) = self.dict.lock().take() {
            d.stop();
        }
    }

    // ---- calls ------------------------------------------------------------------------------------------------------
    pub async fn rpc(&self, method: &str, mut params: Value) -> Result<Value, String> {
        let id = self.next.fetch_add(1, Ordering::SeqCst);
        if params.get("task").is_none() {
            params["task"] = json!(self.current_task());
        }
        let (tx, rx) = oneshot::channel();
        self.rpcs.lock().insert(id, tx);
        if let Err(e) = self.send_json(json!({ "type": "rpc", "id": id, "method": method, "params": params })) {
            self.rpcs.lock().remove(&id);
            return Err(e);
        }
        match tokio::time::timeout(Duration::from_secs(30), rx).await {
            Ok(Ok(r)) => r,
            Ok(Err(_)) => Err("disconnected from the hub".into()),
            Err(_) => {
                self.rpcs.lock().remove(&id);
                Err(format!("{method}: timed out"))
            }
        }
    }

    async fn control_msg(&self, mut msg: Value) -> Result<(String, Option<i64>), String> {
        let id = self.next.fetch_add(1, Ordering::SeqCst);
        msg["id"] = json!(id);
        let (tx, rx) = oneshot::channel();
        self.controls.lock().insert(id, tx);
        if let Err(e) = self.send_json(msg) {
            self.controls.lock().remove(&id);
            return Err(e);
        }
        match tokio::time::timeout(Duration::from_secs(60), rx).await {
            Ok(Ok((true, m, spawned))) => Ok((m, spawned)),
            Ok(Ok((false, m, _))) => Err(m),
            Ok(Err(_)) => Err("disconnected from the hub".into()),
            Err(_) => {
                self.controls.lock().remove(&id);
                Err("the hub did not answer".into())
            }
        }
    }
    /// a task control (lib/hub.ts control): pause / resume / cancel / close / tell / budget / rate / hideModel / title
    pub async fn control(&self, task: i64, op: &str, fields: Value) -> Result<String, String> {
        let mut m = json!({ "type": "control", "task": task, "op": op });
        if let Value::Object(o) = fields {
            for (k, v) in o {
                m[k] = v;
            }
        }
        self.control_msg(m).await.map(|r| r.0)
    }

    /// show this task: the hub answers with its snapshot (a HubEvent::Snapshot)
    pub fn open(&self, task: i64) {
        self.task.store(task, Ordering::SeqCst);
        let _ = self.send_json(json!({ "type": "open", "task": task }));
    }

    pub fn images_json(images: Vec<ImageIn>) -> Value {
        use base64::Engine;
        Value::Array(images.into_iter().map(|i| json!({ "mediaType": i.media_type, "data": base64::engine::general_purpose::STANDARD.encode(&i.bytes), "name": i.name, "w": i.w, "h": i.h })).collect())
    }

    // ---- the engine's operations, as the hub exposes them ------------------------------------------------------------
    pub fn send_message(&self, task: i64, text: &str, images: Vec<ImageIn>, run: bool) -> Result<String, String> {
        self.send_json(json!({ "type": "send", "task": task, "text": text, "images": Self::images_json(images), "run": run }))?;
        Ok(String::new())
    }
    pub fn continue_turn(&self, task: i64) -> Result<(), String> {
        self.send_json(json!({ "type": "continue", "task": task }))
    }
    pub fn stop(&self, task: i64) {
        let _ = self.send_json(json!({ "type": "cancel", "task": task }));
    }
    pub fn stop_tool(&self, call_id: &str) {
        let _ = self.send_json(json!({ "type": "stopTool", "callId": call_id }));
    }
    pub fn update_settings(&self, task: i64, patch: SettingsPatch) -> Result<&'static str, String> {
        self.send_json(json!({ "type": "settings", "task": task, "patch": patch }))?;
        Ok("saved")
    }
    pub fn update_global(&self, max_turns: Option<u32>, dictation: Option<DictationService>) {
        let mut patch = json!({});
        if let Some(m) = max_turns {
            patch["maxTurns"] = json!(m);
        }
        if let Some(d) = dictation {
            patch["dictation"] = json!(d);
        }
        let _ = self.send_json(json!({ "type": "global", "patch": patch }));
    }
    pub fn approve(&self, id: &str, accept: bool, persist: Option<String>) {
        let mut m = json!({ "type": "approve", "id": id, "accept": accept });
        if let Some(p) = persist {
            m["persist"] = json!(p);
        }
        let _ = self.send_json(m);
        // answered here: the dialog goes at once (the hub's approvalDone, to every client, removes it elsewhere)
        self.emit(HubEvent::ApprovalDone(id.to_string()));
    }
    #[allow(clippy::too_many_arguments)]
    pub async fn spawn(&self, parent: i64, mode: &str, title: &str, instruction: &str, images: Vec<ImageIn>, prev: Option<i64>, budget: Option<BudgetArgs>, settings: Option<SettingsPatch>, start: bool) -> Result<i64, String> {
        let mut m = json!({ "type": "spawn", "task": parent, "mode": mode, "title": title, "instruction": instruction, "images": Self::images_json(images), "start": start });
        if let Some(p) = prev {
            m["prev"] = json!(p);
        }
        if let Some(b) = budget {
            m["budget"] = serde_json::to_value(b).unwrap_or(Value::Null);
        }
        if let Some(s) = settings {
            m["settings"] = serde_json::to_value(s).unwrap_or(Value::Null);
        }
        let (_, spawned) = self.control_msg(m).await?;
        spawned.ok_or_else(|| "the hub did not say which task it spawned".to_string())
    }
    pub async fn pause(&self, task: i64, reason: &str) -> Result<String, String> {
        self.control(task, "pause", json!({ "reason": reason })).await
    }
    pub async fn resume(&self, task: i64, message: &str, images: Vec<ImageIn>) -> Result<String, String> {
        self.control(task, "resume", json!({ "message": message, "images": Self::images_json(images) })).await
    }
    pub async fn cancel(&self, task: i64, reason: &str) -> Result<String, String> {
        self.control(task, "cancel", json!({ "reason": reason })).await
    }
    pub async fn close(&self, task: i64, result: &str) -> Result<String, String> {
        self.control(task, "close", json!({ "result": result })).await
    }
    pub async fn set_budget(&self, task: i64, b: BudgetArgs) -> Result<String, String> {
        self.control(task, "budget", json!({ "budget": b })).await
    }
    pub async fn rename(&self, task: i64, title: &str) -> Result<String, String> {
        self.control(task, "title", json!({ "title": title })).await
    }
    pub async fn set_rate(&self, model: &str, per1m: Per1M, note: &str) -> Result<String, String> {
        let task = self.current_task();
        self.control(task, "rate", json!({ "model": model, "per1m": per1m, "note": note })).await
    }
    pub async fn hide_model(&self, model: &str, hidden: bool) -> Result<String, String> {
        let task = self.current_task();
        self.control(task, "hideModel", json!({ "model": model, "hidden": hidden })).await
    }

    // ---- rpc --------------------------------------------------------------------------------------------------------
    pub async fn turns_recent(&self, task: i64, limit: usize) -> Vec<TurnRecord> {
        self.rpc("turns.recent", json!({ "task": task, "limit": limit })).await.ok().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default()
    }
    pub async fn requests_recent(&self, task: i64, subtree: bool, limit: usize) -> Vec<RequestRecord> {
        let mut p = json!({ "task": task, "limit": limit });
        if subtree {
            p["scope"] = json!("subtree");
        }
        self.rpc("requests.recent", p).await.ok().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default()
    }
    pub async fn turn_requests(&self, turn_id: &str) -> Vec<RequestRecord> {
        self.rpc("turn.requests", json!({ "turnId": turn_id })).await.ok().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default()
    }
    /// log entries [from, to) of a task (slim unless `full`)
    pub async fn log_range(&self, task: i64, from: i64, to: i64, full: bool) -> Result<Vec<LogEntry>, String> {
        let v = self.rpc("log.range", json!({ "task": task, "from": from, "to": to, "full": full })).await?;
        serde_json::from_value(v).map_err(|e| e.to_string())
    }
    pub async fn view_parts(&self, task: i64) -> Vec<ViewPart> {
        self.rpc("view.parts", json!({ "task": task })).await.ok().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default()
    }
    pub async fn cview_parts(&self, task: i64) -> Vec<ViewPart> {
        self.rpc("cview.parts", json!({ "task": task })).await.ok().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default()
    }
    pub async fn request_detail(&self, id: &str) -> Option<RequestDetail> {
        self.rpc("request.get", json!({ "id": id })).await.ok().and_then(|v| RequestDetail::parse(&v))
    }
    pub async fn node_children(&self, t: i64, l: u32, i: i64) -> (Option<Arc<LogEntry>>, Vec<NodeKid>) {
        match self.rpc("node.children", json!({ "t": t, "l": l, "i": i })).await {
            Ok(v) => parse_node_children(&v),
            Err(_) => (None, vec![]),
        }
    }
    pub async fn search(&self, task: i64, q: &str) -> Vec<SearchHit> {
        self.rpc("search", json!({ "task": task, "q": q })).await.ok().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default()
    }
    pub async fn render(&self, task: i64) -> Option<Rendered> {
        self.rpc("render", json!({ "task": task })).await.ok().and_then(|v| Rendered::parse(&v))
    }
    pub async fn breakdown(&self, task: i64, subtree: bool) -> Breakdown {
        let mut p = json!({ "task": task });
        if subtree {
            p["scope"] = json!("subtree");
        }
        self.rpc("metrics.breakdown", p).await.map(|v| Breakdown::parse(&v)).unwrap_or_default()
    }
    pub async fn models_list(&self) -> Vec<ModelRow> {
        self.rpc("models.list", json!({})).await.ok().and_then(|v| v.as_array().map(|a| a.iter().filter_map(ModelRow::parse).collect())).unwrap_or_default()
    }
    pub async fn refresh_catalog(&self) -> Result<(), String> {
        self.rpc("catalog.refresh", json!({})).await.map(|_| ())
    }

    /// an image from the hub's database (GET /blob/<sha>)
    pub async fn blob(&self, sha: &str) -> Result<(String, Vec<u8>), String> {
        let r = self.http.get(self.cfg.blob_url(sha)).send().await.map_err(|e| e.to_string())?;
        if !r.status().is_success() {
            return Err(format!("blob {sha}: {}", r.status()));
        }
        let mt = r.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("image/png").to_string();
        let bytes = r.bytes().await.map_err(|e| e.to_string())?;
        Ok((mt, bytes.to_vec()))
    }

    // ---- dictation: a second socket per dictation (app/stt/serve.ts, app/ui/dictation.ts) ----------------------------
    /// Start dictating into `task`'s composer: the microphone (or OPTCHAT_FAKE_MIC's WAV) streams pcm16 16 kHz mono in
    /// ~100 ms chunks; chunks made before the socket is open wait and go out once it is.
    pub fn dictate_start(self: &Arc<Self>, task: i64) {
        if let Some(prev) = self.dict.lock().take() {
            prev.stop(); // a new dictation finishes the previous one (the hub does the same on its side)
        }
        let s = DictSession::start(self.clone(), task);
        *self.dict.lock() = Some(s);
    }
    /// stop listening and keep the text (the final text comes as Done)
    pub fn dictate_stop(&self) {
        if let Some(s) = self.dict.lock().clone() {
            if !s.stopped.load(Ordering::SeqCst) {
                self.emit(HubEvent::Dictation { task: s.task, ev: DictEvent::State("finishing".into()) });
            }
            s.stop();
        }
    }
    fn dict_ended(&self, s: &Arc<DictSession>) {
        let mut g = self.dict.lock();
        if g.as_ref().map(|x| Arc::ptr_eq(x, s)).unwrap_or(false) {
            *g = None;
        }
    }
}

/// One dictation: the mic thread feeds a channel; a task owns the /stt socket.
pub struct DictSession {
    task: i64,
    tx: mpsc::UnboundedSender<WsMsg>,
    stopped: AtomicBool,
    mic: Mutex<Option<crate::mic::Mic>>,
    fake_stop: Arc<AtomicBool>,
}
impl DictSession {
    fn start(client: Arc<Client>, task: i64) -> Arc<DictSession> {
        let (tx, mut rx) = mpsc::unbounded_channel::<WsMsg>();
        let s = Arc::new(DictSession { task, tx, stopped: AtomicBool::new(false), mic: Mutex::new(None), fake_stop: Arc::new(AtomicBool::new(false)) });
        let emit = {
            let c = client.clone();
            move |ev: DictEvent| c.emit(HubEvent::Dictation { task, ev })
        };
        emit(DictEvent::State("connecting".into()));
        // the microphone starts at the press; its chunks queue in the channel until the socket is open
        let on_chunk: Arc<dyn Fn(crate::mic::MicChunk) + Send + Sync> = {
            let tx = s.tx.clone();
            let e = emit.clone();
            Arc::new(move |c: crate::mic::MicChunk| {
                e(DictEvent::Level((c.level.sqrt() * 2.0).min(1.0)));
                let _ = tx.send(WsMsg::binary(c.pcm));
            })
        };
        if let Ok(wav) = std::env::var("OPTCHAT_FAKE_MIC") {
            // tests: a spoken clip plays as the microphone, then silence until stopped
            let stop = s.fake_stop.clone();
            std::thread::spawn(move || {
                let data = crate::media::wav_pcm(&std::fs::read(&wav).unwrap_or_default());
                let mut k = 0;
                while !stop.load(Ordering::SeqCst) {
                    let chunk: Vec<u8> = if k < data.len() { data[k..(k + 3200).min(data.len())].to_vec() } else { vec![0u8; 3200] };
                    k += 3200;
                    let samples: Vec<i16> = chunk.chunks(2).map(|b| i16::from_le_bytes([b[0], *b.get(1).unwrap_or(&0)])).collect();
                    let rms = (samples.iter().map(|x| (*x as f32 / 32768.0).powi(2)).sum::<f32>() / samples.len().max(1) as f32).sqrt();
                    on_chunk(crate::mic::MicChunk { pcm: chunk, level: rms });
                    std::thread::sleep(Duration::from_millis(100));
                }
            });
        } else {
            match crate::mic::Mic::start(on_chunk) {
                Ok(m) => *s.mic.lock() = Some(m),
                Err(e) => {
                    emit(DictEvent::Error(format!("microphone failed: {e}")));
                    s.stopped.store(true, Ordering::SeqCst);
                }
            }
        }
        let me = s.clone();
        tokio::spawn(async move {
            let url = client.cfg.ws_url("/stt", task);
            let ws = match client.connect(&url).await {
                Ok(ws) => ws,
                Err(e) => {
                    emit(DictEvent::Error(format!("dictation could not connect: {e}")));
                    me.release();
                    emit(DictEvent::Done { text: String::new(), service: None, audio_ms: 0 });
                    client.dict_ended(&me);
                    return;
                }
            };
            let (mut w, mut r) = ws.split();
            let writer = tokio::spawn(async move {
                while let Some(m) = rx.recv().await {
                    if w.send(m).await.is_err() {
                        break;
                    }
                }
            });
            if me.stopped.load(Ordering::SeqCst) {
                let _ = me.tx.send(WsMsg::text(json!({ "type": "stop" }).to_string()));
            }
            let mut done = false;
            while let Ok(Some(msg)) = tokio::time::timeout(Duration::from_secs(120), r.next()).await {
                let Ok(WsMsg::Text(t)) = msg else {
                    if matches!(msg, Ok(WsMsg::Close(_)) | Err(_)) {
                        break;
                    }
                    continue;
                };
                let Ok(m) = serde_json::from_str::<Value>(t.as_str()) else { continue };
                match m["type"].as_str() {
                    Some("ready") if !me.stopped.load(Ordering::SeqCst) => emit(DictEvent::State("listening".into())),
                    Some("text") => emit(DictEvent::Text(m["text"].as_str().unwrap_or("").to_string())),
                    Some("error") => emit(DictEvent::Error(m["message"].as_str().unwrap_or("").to_string())),
                    Some("done") => {
                        done = true;
                        emit(DictEvent::Done { text: m["text"].as_str().unwrap_or("").to_string(), service: m["service"].as_str().map(String::from), audio_ms: m["audioMs"].as_u64().unwrap_or(0) });
                        break;
                    }
                    _ => {}
                }
            }
            writer.abort();
            me.release();
            if !done {
                emit(DictEvent::Done { text: String::new(), service: None, audio_ms: 0 });
            }
            client.dict_ended(&me);
        });
        s
    }
    fn release(&self) {
        self.fake_stop.store(true, Ordering::SeqCst);
        if let Some(m) = self.mic.lock().take() {
            m.stop();
        }
    }
    /// stop listening: the mic stops, the hub finishes and sends the final text
    pub fn stop(&self) {
        if self.stopped.swap(true, Ordering::SeqCst) {
            return;
        }
        self.release();
        let _ = self.tx.send(WsMsg::text(json!({ "type": "stop" }).to_string()));
        let _ = self.task;
    }
}
