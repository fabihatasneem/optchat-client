//! Engines and workspaces in the UI (spec-workspaces §4): the Workspaces panel, a rail view opened by the top-left
//! icon (and ⇧⌘O): engines → workspaces (connected ones with a check on the one on screen and a dot for unread
//! errors/warnings, available ones to connect with a token typed inline, disabled ones collapsed, an offline engine
//! greyed and retried), Add engine inline, Disconnect. No workspace name is special.
use crate::client::ws_key;
use crate::state::*;
use crate::theme::c;
use crate::ui::*;
use crate::workspace::Workspace;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::Sizable;
use optchat_proto::api::Conn;

/// the panel on screen (from the status bar's workspace chip, the empty state, Settings › App): never toggles it off
pub fn open_panel(w: &mut Workspace, cx: &mut Context<Workspace>) {
    w.state.update(cx, |s, _| {
        s.prefs.view = View::Workspaces;
        s.save_prefs();
    });
    w.set_side_open(true, cx);
}
/// the panel with its Add engine form open and focused
pub fn open_add_engine(w: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    open_panel(w, cx);
    w.ws_panel.update(cx, |p, cx| p.start_add(window, cx));
}

pub struct WorkspacesPanel {
    pub state: Entity<AppState>,
    /// engines whose disabled workspaces are expanded
    disabled_open: std::collections::HashSet<String>,
    /// the workspace whose Connect row is open: (endpoint, name)
    pub connecting: Option<(String, String)>,
    pub token: Entity<InputState>,
    pub adding: bool,
    pub endpoint: Entity<InputState>,
    pub name: Entity<InputState>,
    err: Option<String>,
    _subs: Vec<Subscription>,
}

impl WorkspacesPanel {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> WorkspacesPanel {
        let token = cx.new(|cx| InputState::new(window, cx).masked(true).placeholder("client token"));
        let endpoint = cx.new(|cx| InputState::new(window, cx).placeholder("chat.example.dev (or http://127.0.0.1:port)"));
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("name (optional)"));
        let mut subs = vec![cx.observe(&state, |_, _, cx| cx.notify())];
        subs.push(cx.subscribe(&token, |p: &mut WorkspacesPanel, _, e: &InputEvent, cx| {
            if let InputEvent::PressEnter { .. } = e {
                p.connect(cx);
            }
        }));
        for i in [&endpoint, &name] {
            subs.push(cx.subscribe(i, |p: &mut WorkspacesPanel, _, e: &InputEvent, cx| {
                if let InputEvent::PressEnter { .. } = e {
                    p.add(cx);
                }
            }));
        }
        WorkspacesPanel { state, disabled_open: Default::default(), connecting: None, token, adding: false, endpoint, name, err: None, _subs: subs }
    }
    pub fn start_add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.adding = true;
        self.connecting = None;
        self.err = None;
        self.endpoint.update(cx, |i, cx| i.focus(window, cx));
        cx.notify();
    }
    pub fn start_connect(&mut self, endpoint: &str, ws: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.connecting = Some((endpoint.to_string(), ws.to_string()));
        self.adding = false;
        self.err = None;
        self.token.update(cx, |i, cx| {
            i.set_value("", window, cx);
            i.focus(window, cx);
        });
        // UI tests (OPTCHAT_TEST_HOOKS): the workspace's token from OPTCHAT_TEST_TOKENS ({name: token}), never shown
        if std::env::var("OPTCHAT_TEST_HOOKS").is_ok() {
            let m: std::collections::BTreeMap<String, String> = std::env::var("OPTCHAT_TEST_TOKENS").ok().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
            if let Some(t) = m.get(ws).cloned() {
                self.token.update(cx, |i, cx| i.set_value(t, window, cx));
            }
        }
        cx.notify();
    }
    pub fn set_add_fields(&mut self, endpoint: &str, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        let (e, n) = (endpoint.to_string(), name.to_string());
        self.endpoint.update(cx, |i, cx| i.set_value(e, window, cx));
        self.name.update(cx, |i, cx| i.set_value(n, window, cx));
    }
    pub fn add(&mut self, cx: &mut Context<Self>) {
        let ep = self.endpoint.read(cx).value().to_string();
        let name = self.name.read(cx).value().to_string();
        match self.state.update(cx, |s, cx| s.add_engine(&ep, &name, cx)) {
            Ok(_) => {
                self.adding = false;
                self.err = None;
            }
            Err(e) => self.err = Some(e),
        }
        cx.notify();
    }
    pub fn connect(&mut self, cx: &mut Context<Self>) {
        let Some((ep, ws)) = self.connecting.clone() else { return };
        let t = self.token.read(cx).value().to_string();
        match self.state.update(cx, |s, cx| s.connect_ws(&ep, &ws, &t, cx)) {
            Ok(()) => {
                self.connecting = None;
                self.err = None;
            }
            Err(e) => self.err = Some(e),
        }
        cx.notify();
    }
    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        self.connecting = None;
        self.adding = false;
        self.err = None;
        cx.notify();
    }

    fn row_base(id: SharedString, cx: &App) -> Stateful<Div> {
        let k = c(cx).clone();
        let hover = k.hover;
        div().id(id).flex().items_center().gap(px(8.)).mx(px(6.)).pl(px(10.)).pr(px(6.)).min_h(px(30.)).rounded(px(6.)).hover(move |d| d.bg(hover))
    }
}

impl Render for WorkspacesPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let k = c(cx).clone();
        let s = self.state.read(cx);
        let engines = s.engines.clone();
        let meta: Vec<WsMeta> = s.meta.values().cloned().collect();
        let active = s.active.clone();
        let mut col = div().flex().flex_col().pb(px(16.));
        let mut rows_state = vec![];
        if engines.is_empty() && !self.adding {
            col = col.child(div().px(px(14.)).py(px(8.)).text_size(px(12.)).text_color(k.muted).child("No engine yet. Add one, then connect a workspace with its client token."));
        }
        for (ei, e) in engines.iter().enumerate() {
            let online = e.online != Some(false);
            let fg = if online { k.text } else { k.faint };
            col = col.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(7.))
                    .px(px(14.))
                    .pt(px(if ei == 0 { 4. } else { 14. }))
                    .pb(px(4.))
                    .child(icon("server", 13., if online { k.muted } else { k.faint }))
                    .child(div().text_size(px(12.5)).font_weight(FontWeight::SEMIBOLD).text_color(fg).child(e.cfg.name.clone()))
                    .child(div().flex_1())
                    .child(div().text_size(px(11.)).text_color(if online { k.faint } else { k.amber }).child(match e.online {
                        Some(false) => "offline · retrying",
                        None => "…",
                        _ => "",
                    }))
                    .child(probe(format!("wsp.engine.{ei}"))),
            );
            let connected: Vec<&WsMeta> = meta.iter().filter(|m| m.endpoint == e.cfg.endpoint).collect();
            let is_default = |n: &str| e.workspaces.iter().any(|w| w.name == n && w.default);
            for m in &connected {
                let key = ws_key(&m.endpoint, &m.ws);
                let on = key == active;
                // the one on screen has its notices in the status bar; the others get the dot
                let alert = !on && (m.unread.error > 0 || m.unread.warn > 0);
                let acol = if m.unread.error > 0 { k.red } else { k.amber };
                let state = if m.disabled.is_some() {
                    Some("disabled")
                } else if m.refused.is_some() {
                    Some("token refused")
                } else if m.no_token {
                    Some("no token")
                } else if !online || m.conn == Conn::Closed {
                    Some("offline")
                } else if m.conn == Conn::Connecting {
                    Some("connecting")
                } else {
                    None
                };
                rows_state.push(serde_json::json!({ "ws": m.ws, "engine": ei, "kind": "connected", "on": on, "alert": alert, "state": state }));
                let (key2, key3, name) = (key.clone(), key.clone(), m.ws.clone());
                col = col.child(
                    Self::row_base(SharedString::from(format!("wsp-{key}")), cx)
                        .cursor_pointer()
                        .when(on, |d| d.bg(k.selected))
                        .child(div().w(px(14.)).child(if on { icon("check", 13., k.accent).into_any_element() } else { div().into_any_element() }))
                        .child(div().flex_1().min_w_0().flex().items_center().gap(px(6.)).text_size(px(13.)).text_color(if state.is_some() { k.muted } else { fg }).child(m.ws.clone()).when(is_default(&m.ws), |d| d.child(div().text_size(px(10.5)).text_color(k.faint).child("default"))))
                        .when_some(state, |d, st| d.child(div().text_size(px(11.)).text_color(k.faint).child(st)))
                        .when(alert, |d| d.child(dot(acol, 7.)))
                        .child(
                            icon_btn(SharedString::from(format!("wsp-x-{key}")), "x", 12., cx)
                                .tooltip(tooltip(format!("Disconnect {name}")))
                                .child(probe(format!("wsp.disc.{}", m.ws)))
                                .on_click(cx.listener(move |p, _, _, cx| {
                                    cx.stop_propagation();
                                    let key = key3.clone();
                                    p.state.update(cx, |s, cx| s.disconnect_ws(&key, cx));
                                })),
                        )
                        .child(probe(format!("wsp.ws.{}", m.ws)))
                        .on_click(cx.listener(move |p, _, _, cx| {
                            let key = key2.clone();
                            p.state.update(cx, |s, cx| s.switch_ws(&key, cx));
                        })),
                );
            }
            let avail: Vec<String> = e.workspaces.iter().filter(|w| w.running && !connected.iter().any(|m| m.ws == w.name)).map(|w| w.name.clone()).collect();
            let disabled: Vec<String> = e.workspaces.iter().filter(|w| !w.running && !connected.iter().any(|m| m.ws == w.name)).map(|w| w.name.clone()).collect();
            for n in &avail {
                rows_state.push(serde_json::json!({ "ws": n, "engine": ei, "kind": "available" }));
                let open = self.connecting.as_ref().is_some_and(|(ep, w)| *ep == e.cfg.endpoint && w == n);
                let (ep, name) = (e.cfg.endpoint.clone(), n.clone());
                col = col.child(
                    Self::row_base(SharedString::from(format!("wsp-a-{ei}-{n}")), cx)
                        .when(online && !open, |d| d.cursor_pointer())
                        .child(div().w(px(14.)))
                        .child(div().flex_1().min_w_0().flex().items_center().gap(px(6.)).text_size(px(13.)).text_color(k.muted).child(n.clone()).when(is_default(n), |d| d.child(div().text_size(px(10.5)).text_color(k.faint).child("default"))))
                        .when(!open, |d| d.child(div().text_size(px(11.5)).text_color(if online { k.accent } else { k.faint }).child("Connect…")))
                        .child(probe(format!("wsp.connect.{n}")))
                        .when(online && !open, |d| {
                            d.on_click(cx.listener(move |p, _, window, cx| p.start_connect(&ep, &name, window, cx)))
                        }),
                );
                if open {
                    // the token, inline under its workspace
                    col = col.child(
                        div()
                            .mx(px(6.))
                            .ml(px(34.))
                            .mb(px(6.))
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .child(div().child(Input::new(&self.token).small()).child(probe("wsp.token")))
                            .child(div().text_size(px(11.)).text_color(k.faint).child("a client token for this workspace (from its admin page); kept in the Keychain"))
                            .when_some(self.err.clone(), |d, e| d.child(div().text_size(px(11.5)).text_color(k.red).child(e)))
                            .child(
                                div()
                                    .flex()
                                    .gap(px(6.))
                                    .child(btn_small("wsp-go", "Connect", Tone::Primary, cx).child(probe("wsp.go")).on_click(cx.listener(|p, _, _, cx| p.connect(cx))))
                                    .child(btn_small("wsp-cancel", "Cancel", Tone::Ghost, cx).on_click(cx.listener(|p, _, _, cx| p.cancel(cx)))),
                            ),
                    );
                }
            }
            if !disabled.is_empty() {
                let open = self.disabled_open.contains(&e.cfg.endpoint);
                let ep = e.cfg.endpoint.clone();
                col = col.child(
                    Self::row_base(SharedString::from(format!("wsp-d-{ei}")), cx)
                        .cursor_pointer()
                        .min_h(px(26.))
                        .child(chevron(format!("wsp-ch-{ei}"), 12., k.faint, open))
                        .child(div().text_size(px(11.5)).text_color(k.faint).child(format!("{} disabled", disabled.len())))
                        .child(probe(format!("wsp.disabled.{ei}")))
                        .on_click(cx.listener(move |p, _, _, cx| {
                            if !p.disabled_open.remove(&ep) {
                                p.disabled_open.insert(ep.clone());
                            }
                            cx.notify();
                        })),
                );
                if open {
                    for n in &disabled {
                        rows_state.push(serde_json::json!({ "ws": n, "engine": ei, "kind": "disabled" }));
                        col = col.child(div().pl(px(44.)).h(px(24.)).flex().items_center().text_size(px(12.)).text_color(k.faint).child(n.clone()));
                    }
                }
            }
        }
        // Add engine: inline form
        col = col.child(div().h(px(1.)).bg(k.line).mx(px(14.)).my(px(10.)));
        if self.adding {
            col = col.child(
                div()
                    .px(px(14.))
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(div().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).child("Add engine"))
                    .child(div().child(Input::new(&self.endpoint).small()).child(probe("wsp.endpoint")))
                    .child(div().child(Input::new(&self.name).small()))
                    .when_some(self.err.clone().filter(|_| self.connecting.is_none()), |d, e| d.child(div().text_size(px(11.5)).text_color(k.red).child(e)))
                    .child(
                        div()
                            .flex()
                            .gap(px(6.))
                            .child(btn_small("wsp-add-go", "Add", Tone::Primary, cx).child(probe("wsp.add.go")).on_click(cx.listener(|p, _, _, cx| p.add(cx))))
                            .child(btn_small("wsp-add-cancel", "Cancel", Tone::Ghost, cx).on_click(cx.listener(|p, _, _, cx| p.cancel(cx)))),
                    ),
            );
        } else {
            col = col.child(
                Self::row_base("wsp-add".into(), cx)
                    .cursor_pointer()
                    .child(icon("plus", 13., k.muted))
                    .child(div().text_size(px(13.)).child("Add engine…"))
                    .child(probe("wsp.add"))
                    .on_click(cx.listener(|p, _, window, cx| p.start_add(window, cx))),
            );
        }
        set_state_extra(
            "wsPanel",
            serde_json::json!({ "rows": rows_state, "connecting": self.connecting.as_ref().map(|c| c.1.clone()), "hasToken": !self.token.read(cx).value().is_empty(), "adding": self.adding, "err": self.err,
                "engines": engines.iter().map(|e| serde_json::json!({ "name": e.cfg.name, "online": e.online, "workspaces": e.workspaces.len() })).collect::<Vec<_>>() }),
        );
        crate::ui::scroll_body("WorkspacesPanel", col.child(probe("wspanel")).into_any_element())
    }
}
