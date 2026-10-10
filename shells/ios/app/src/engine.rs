//! The engine client (spec-workspaces §2): one websocket per engine, `connect {ws}`, `ws` on every message.
//! A background thread owns the socket (TLS via the system's Security framework); the UI sends commands through a
//! channel and receives events through another. Reconnects with backoff (0.4 s doubling to 5 s).
//!
//! Auth: the client token as the websocket subprotocol `optchat.token.<token>` (the tunnel relays subprotocols but
//! drops Authorization). Through the tunnel (*.tunnel.reframe.sh) the access key goes as the `reframe_tunnel_key`
//! cookie and Origin must be the tunnel's own origin; direct to an engine, Origin is the engine's origin.
use serde_json::{json, Value};
use std::io::ErrorKind;
use std::net::TcpStream;
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::time::Duration;
use tungstenite::client::IntoClientRequest;
use tungstenite::{Message, WebSocket};

#[derive(Clone, Debug)]
pub struct Config {
    pub endpoint: String, // https://host
    pub workspace: String,
    pub token: String,
    pub tunnel_key: String, // empty unless the endpoint is a tunnel
}
impl Config {
    pub fn to_json(&self) -> String {
        json!({"endpoint": self.endpoint, "workspace": self.workspace, "token": self.token, "tunnel_key": self.tunnel_key}).to_string()
    }
    pub fn from_json(s: &str) -> Option<Config> {
        let v: Value = serde_json::from_str(s).ok()?;
        Some(Config {
            endpoint: v["endpoint"].as_str()?.to_string(),
            workspace: v["workspace"].as_str()?.to_string(),
            token: v["token"].as_str()?.to_string(),
            tunnel_key: v["tunnel_key"].as_str().unwrap_or("").to_string(),
        })
    }
}

pub enum Event {
    Status(String),       // connecting / open / closed: <why>
    Message(Value),       // an engine message for our workspace
}

pub enum Command {
    Send(Value), // `ws` is added here
    Stop,
}

pub fn start(cfg: Config, events: Sender<Event>, commands: Receiver<Command>) {
    std::thread::Builder::new().name("engine".into()).spawn(move || run(cfg, events, commands)).ok();
}

fn run(cfg: Config, events: Sender<Event>, commands: Receiver<Command>) {
    let mut retry = 0u32;
    loop {
        let _ = events.send(Event::Status("connecting".into()));
        match connect(&cfg) {
            Ok(mut ws) => {
                retry = 0;
                let _ = events.send(Event::Status("open".into()));
                let why = pump(&cfg, &mut ws, &events, &commands);
                let _ = events.send(Event::Status(format!("closed: {why}")));
                if why == "stopped" { return }
            }
            Err(e) => { let _ = events.send(Event::Status(format!("closed: {e}"))); }
        }
        // drain a Stop while waiting
        let wait = Duration::from_millis((400u64 << retry.min(4)).min(5000));
        retry += 1;
        let until = std::time::Instant::now() + wait;
        while std::time::Instant::now() < until {
            if let Ok(Command::Stop) = commands.try_recv() { return }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

fn connect(cfg: &Config) -> Result<WebSocket<native_tls::TlsStream<TcpStream>>, String> {
    let base = url::Url::parse(&cfg.endpoint).map_err(|e| e.to_string())?;
    let host = base.host_str().ok_or("no host")?.to_string();
    let port = base.port_or_known_default().unwrap_or(443);
    let mut req = format!("wss://{host}/engine/ws").into_client_request().map_err(|e| e.to_string())?;
    let h = req.headers_mut();
    h.insert("Origin", format!("https://{host}").parse().unwrap());
    h.insert("Sec-WebSocket-Protocol", format!("optchat, optchat.token.{}", cfg.token).parse().map_err(|_| "bad token")?);
    if !cfg.tunnel_key.is_empty() {
        h.insert("Cookie", format!("reframe_tunnel_key={}", cfg.tunnel_key).parse().map_err(|_| "bad key")?);
    }
    let tcp = TcpStream::connect((host.as_str(), port)).map_err(|e| e.to_string())?;
    tcp.set_nodelay(true).ok();
    let mut tls = native_tls::TlsConnector::builder();
    if cfg!(debug_assertions) {
        // debug builds: trust the local edge's mkcert root (the simulator has its own trust store), *.reframe.dev only
        if host.ends_with(".reframe.dev") {
            if let Ok(der) = std::fs::read(std::env::current_exe().unwrap().parent().unwrap().join("dev-root-ca.der")) {
                if let Ok(c) = native_tls::Certificate::from_der(&der) { tls.add_root_certificate(c); }
            }
        }
    }
    let tls = tls.build().map_err(|e| e.to_string())?;
    let stream = tls.connect(&host, tcp).map_err(|e| format!("TLS: {e}"))?;
    let (ws, resp) = tungstenite::client(req, stream).map_err(|e| format!("handshake: {e}"))?;
    let _ = resp;
    // short read timeouts so one thread can both read and write
    ws.get_ref().get_ref().set_read_timeout(Some(Duration::from_millis(30))).ok();
    Ok(ws)
}

fn pump(cfg: &Config, ws: &mut WebSocket<native_tls::TlsStream<TcpStream>>, events: &Sender<Event>, commands: &Receiver<Command>) -> String {
    let mut last_ping = std::time::Instant::now();
    loop {
        // outgoing
        loop {
            match commands.try_recv() {
                Ok(Command::Send(mut v)) => {
                    v["ws"] = json!(cfg.workspace);
                    if let Err(e) = ws.send(Message::text(v.to_string())) { return e.to_string() }
                }
                Ok(Command::Stop) => { let _ = ws.close(None); return "stopped".into() }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return "stopped".into(),
            }
        }
        if last_ping.elapsed() > Duration::from_secs(15) {
            last_ping = std::time::Instant::now();
            if let Err(e) = ws.send(Message::Ping(Vec::new().into())) { return e.to_string() }
        }
        // incoming
        match ws.read() {
            Ok(Message::Text(t)) => {
                let Ok(v) = serde_json::from_str::<Value>(t.as_str()) else { continue };
                match v["type"].as_str() {
                    // the engine's list: subscribe to our workspace on the task list's root
                    Some("workspaces") => { let _ = ws.send(Message::text(json!({"type": "connect", "ws": cfg.workspace, "task": 0}).to_string())); }
                    _ => {
                        if v["ws"].as_str().map_or(true, |w| w == cfg.workspace) { let _ = events.send(Event::Message(v)); }
                    }
                }
            }
            Ok(Message::Close(f)) => return format!("closed by the engine {}", f.map(|f| f.reason.to_string()).unwrap_or_default()),
            Ok(_) => {}
            Err(tungstenite::Error::Io(e)) if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::TimedOut => {}
            Err(e) => return e.to_string(),
        }
    }
}
