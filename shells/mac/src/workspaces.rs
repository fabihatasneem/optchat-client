//! Engines and workspaces in the UI (spec-workspaces §4): the workspace menu under the top-left icon (engines →
//! workspaces: connected ones with a check on the one on screen, available ones to connect with a token, disabled ones
//! collapsed, an offline engine greyed and retried), Add engine and Connect dialogs.
use crate::client::ws_key;
use crate::dialogs::{dialog_box, overlay_bg, Overlay};
use crate::state::*;
use crate::theme::c;
use crate::ui::*;
use crate::workspace::Workspace;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::input::{Input, InputEvent, InputState};
use optchat_proto::api::Conn;

pub fn open_menu(w: &mut Workspace, cx: &mut Context<Workspace>) {
    w.overlay = Some(Overlay::WsMenu);
    cx.notify();
}

/// the menu, anchored under the icon
pub fn menu(w: &mut Workspace, cx: &mut Context<Workspace>) -> AnyElement {
    let k = c(cx).clone();
    let s = w.state.read(cx);
    let mut col = div().flex().flex_col().py(px(6.));
    let mut probe_rows = vec![];
    col = col.child(div().px(px(12.)).pt(px(2.)).pb(px(6.)).text_size(px(11.)).font_weight(FontWeight::SEMIBOLD).text_color(k.muted).child("WORKSPACES"));
    if s.engines.is_empty() {
        col = col.child(div().px(px(12.)).py(px(6.)).max_w(px(300.)).text_size(px(12.)).text_color(k.muted).child("No engine yet. Add one, then connect a workspace with its client token."));
    }
    for (ei, e) in s.engines.iter().enumerate() {
        let online = e.online != Some(false);
        let fg = if online { k.text } else { k.faint };
        col = col.child(
            div()
                .flex()
                .items_center()
                .gap(px(7.))
                .px(px(12.))
                .pt(px(if ei == 0 { 2. } else { 8. }))
                .pb(px(3.))
                .child(icon("server", 13., if online { k.muted } else { k.faint }))
                .child(div().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).text_color(fg).child(e.cfg.name.clone()))
                .when(!online, |d| d.child(div().text_size(px(11.)).text_color(k.faint).child("offline · retrying")))
                .when(e.online.is_none(), |d| d.child(div().text_size(px(11.)).text_color(k.faint).child("…")))
                .child(probe(format!("wsmenu.engine.{ei}"))),
        );
        // connected: every one of this engine's connections (also ones the engine no longer lists)
        let connected: Vec<WsMeta> = s.meta.values().filter(|m| m.endpoint == e.cfg.endpoint).cloned().collect();
        for m in &connected {
            let key = ws_key(&m.endpoint, &m.ws);
            let on = key == s.active;
            let alert = m.unread.error > 0 || m.unread.warn > 0;
            let acol = if m.unread.error > 0 { k.red } else { k.amber };
            let state = if m.disabled.is_some() {
                Some("disabled")
            } else if m.no_token {
                Some("no token")
            } else if !online || m.conn == Conn::Closed {
                Some("offline")
            } else if m.conn == Conn::Connecting {
                Some("connecting")
            } else {
                None
            };
            probe_rows.push(serde_json::json!({ "ws": m.ws, "engine": ei, "kind": "connected", "on": on, "alert": alert, "state": state }));
            let k2 = k.clone();
            let key2 = key.clone();
            let key3 = key.clone();
            let ws_name = m.ws.clone();
            col = col.child(
                div()
                    .id(SharedString::from(format!("wsm-{key}")))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .mx(px(5.))
                    .pl(px(9.))
                    .pr(px(4.))
                    .h(px(30.))
                    .rounded(px(6.))
                    .cursor_pointer()
                    .hover(move |d| d.bg(k2.hover))
                    .when(on, |d| d.bg(k.selected))
                    .child(div().w(px(14.)).child(if on { icon("check", 13., k.accent).into_any_element() } else { div().into_any_element() }))
                    .child(div().flex_1().min_w_0().text_size(px(13.)).text_color(if state.is_some() { k.muted } else { fg }).child(m.ws.clone()))
                    .when_some(state, |d, st| d.child(div().text_size(px(11.)).text_color(k.faint).child(st)))
                    .when(alert, |d| d.child(dot(acol, 7.)))
                    .child(
                        icon_btn(SharedString::from(format!("wsm-x-{key}")), "x", 12., cx)
                            .tooltip(tooltip(format!("Disconnect {ws_name}")))
                            .child(probe(format!("wsmenu.disc.{}", m.ws)))
                            .on_click(cx.listener(move |w, _, _, cx| {
                                cx.stop_propagation();
                                let key = key3.clone();
                                w.state.update(cx, |s, cx| s.disconnect_ws(&key, cx));
                            })),
                    )
                    .child(probe(format!("wsmenu.ws.{}", m.ws)))
                    .on_click(cx.listener(move |w, _, window, cx| {
                        let key = key2.clone();
                        w.switch_workspace(&key, window, cx);
                    })),
            );
        }
        // available: running, not connected; disabled ones collapsed
        let avail: Vec<String> = e.workspaces.iter().filter(|(n, r)| *r && !connected.iter().any(|m| &m.ws == n)).map(|(n, _)| n.clone()).collect();
        let disabled: Vec<String> = e.workspaces.iter().filter(|(n, r)| !*r && !connected.iter().any(|m| &m.ws == n)).map(|(n, _)| n.clone()).collect();
        for n in &avail {
            probe_rows.push(serde_json::json!({ "ws": n, "engine": ei, "kind": "available" }));
            let k2 = k.clone();
            let ep = e.cfg.endpoint.clone();
            let name = n.clone();
            col = col.child(
                div()
                    .id(SharedString::from(format!("wsa-{ei}-{n}")))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .mx(px(5.))
                    .pl(px(9.))
                    .pr(px(8.))
                    .h(px(28.))
                    .rounded(px(6.))
                    .when(online, |d| d.cursor_pointer().hover(move |d| d.bg(k2.hover)))
                    .child(div().w(px(14.)))
                    .child(div().flex_1().min_w_0().text_size(px(13.)).text_color(k.muted).child(n.clone()))
                    .child(div().text_size(px(11.5)).text_color(if online { k.accent } else { k.faint }).child("Connect…"))
                    .child(probe(format!("wsmenu.connect.{n}")))
                    .when(online, |d| {
                        d.on_click(cx.listener(move |w, _, window, cx| {
                            open_connect(w, &ep, &name, window, cx);
                        }))
                    }),
            );
        }
        if !disabled.is_empty() {
            let open = w.menu_disabled_open.contains(&e.cfg.endpoint);
            let ep = e.cfg.endpoint.clone();
            let k2 = k.clone();
            col = col.child(
                div()
                    .id(SharedString::from(format!("wsd-{ei}")))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .mx(px(5.))
                    .pl(px(9.))
                    .h(px(26.))
                    .rounded(px(6.))
                    .cursor_pointer()
                    .hover(move |d| d.bg(k2.hover))
                    .child(chevron(format!("wsd-ch-{ei}"), 12., k.faint, open))
                    .child(div().text_size(px(11.5)).text_color(k.faint).child(format!("{} disabled", disabled.len())))
                    .child(probe(format!("wsmenu.disabled.{ei}")))
                    .on_click(cx.listener(move |w, _, _, cx| {
                        if !w.menu_disabled_open.remove(&ep) {
                            w.menu_disabled_open.insert(ep.clone());
                        }
                        cx.notify();
                    })),
            );
            if open {
                for n in &disabled {
                    probe_rows.push(serde_json::json!({ "ws": n, "engine": ei, "kind": "disabled" }));
                    col = col.child(div().pl(px(42.)).h(px(24.)).flex().items_center().text_size(px(12.)).text_color(k.faint).child(n.clone()));
                }
            }
        }
    }
    let k2 = k.clone();
    col = col.child(div().h(px(1.)).bg(k.line).my(px(6.))).child(
        div()
            .id("wsm-add")
            .flex()
            .items_center()
            .gap(px(8.))
            .mx(px(5.))
            .pl(px(9.))
            .h(px(28.))
            .rounded(px(6.))
            .cursor_pointer()
            .hover(move |d| d.bg(k2.hover))
            .child(icon("plus", 13., k.muted))
            .child(div().text_size(px(13.)).child("Add engine…"))
            .child(probe("wsmenu.add"))
            .on_click(cx.listener(|w, _, window, cx| open_add_engine(w, window, cx))),
    );
    set_state_extra("wsMenu", serde_json::json!({ "rows": probe_rows, "engines": s.engines.iter().map(|e| serde_json::json!({ "name": e.cfg.name, "online": e.online, "workspaces": e.workspaces.len(), "error": e.error })).collect::<Vec<_>>() }));
    div()
        .id("ov-wsmenu")
        .absolute()
        .inset_0()
        .occlude()
        .on_click(cx.listener(|w, _, window, cx| w.close_overlay(window, cx)))
        .child(
            div()
                .id("wsmenu-box")
                .absolute()
                .left(px(46.))
                .top(px(6.))
                .min_w(px(260.))
                .max_w(px(360.))
                .bg(k.panel)
                .border_1()
                .border_color(k.line2)
                .rounded(px(10.))
                .shadow(vec![BoxShadow { color: k.shadow, offset: point(px(0.), px(10.)), blur_radius: px(30.), spread_radius: px(0.) }])
                .on_click(|_, _, cx| cx.stop_propagation())
                .child(col)
                .child(probe("wsmenu")),
        )
        .into_any_element()
}

// ---- Add engine ------------------------------------------------------------------------------------------------------
pub struct AddEngineDialog {
    state: Entity<AppState>,
    pub endpoint: Entity<InputState>,
    name: Entity<InputState>,
    err: Option<String>,
    _subs: Vec<Subscription>,
}
impl EventEmitter<DismissEvent> for AddEngineDialog {}
pub enum Added {
    Engine,
}
impl EventEmitter<Added> for AddEngineDialog {}

pub fn open_add_engine(w: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    let state = w.state.clone();
    let d = cx.new(|cx| {
        let endpoint = cx.new(|cx| InputState::new(window, cx).placeholder("chat.example.dev (https) or http://127.0.0.1:port"));
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("a name (optional)"));
        endpoint.update(cx, |i, cx| i.focus(window, cx));
        let mut subs = vec![];
        for i in [&endpoint, &name] {
            subs.push(cx.subscribe(i, |d: &mut AddEngineDialog, _, e: &InputEvent, cx| {
                if let InputEvent::PressEnter { .. } = e {
                    d.add(cx);
                }
            }));
        }
        AddEngineDialog { state, endpoint, name, err: None, _subs: subs }
    });
    let sub = cx.subscribe_in(&d, window, |w: &mut Workspace, _, _: &DismissEvent, window, cx| w.close_overlay(window, cx));
    let sub2 = cx.subscribe_in(&d, window, |w: &mut Workspace, _, _: &Added, _, cx| open_menu(w, cx));
    w.overlay_subs = vec![sub, sub2];
    w.overlay = Some(Overlay::AddEngine(d));
    cx.notify();
}
impl AddEngineDialog {
    pub fn input(&self) -> Entity<InputState> {
        self.endpoint.clone()
    }
    pub fn set_name(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        let n = name.to_string();
        self.name.update(cx, |i, cx| i.set_value(n, window, cx));
    }
    pub fn add(&mut self, cx: &mut Context<Self>) {
        let ep = self.endpoint.read(cx).value().to_string();
        let name = self.name.read(cx).value().to_string();
        match self.state.update(cx, |s, cx| s.add_engine(&ep, &name, cx)) {
            Ok(_) => cx.emit(Added::Engine),
            Err(e) => {
                self.err = Some(e);
                cx.notify();
            }
        }
    }
}
impl Render for AddEngineDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let k = c(cx).clone();
        set_state_extra("addEngine", serde_json::json!({ "endpoint": self.endpoint.read(cx).value().to_string(), "err": self.err }));
        dialog_box(cx, 440.)
            .child(div().text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).mb(px(6.)).child("Add engine"))
            .child(div().text_size(px(12.)).text_color(k.muted).mb(px(12.)).child("An OptChat engine: its address. Its workspaces show in the menu; connect one with its client token."))
            .child(div().text_size(px(11.5)).text_color(k.muted).mb(px(4.)).child("address"))
            .child(div().mb(px(10.)).child(Input::new(&self.endpoint)).child(probe("addEngine.endpoint")))
            .child(div().text_size(px(11.5)).text_color(k.muted).mb(px(4.)).child("name"))
            .child(div().mb(px(12.)).child(Input::new(&self.name)))
            .when_some(self.err.clone(), |d, e| d.child(div().text_size(px(12.)).text_color(k.red).mb(px(10.)).child(e)))
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .child(btn("ae-cancel", "Cancel", Tone::Ghost, cx).on_click(cx.listener(|_, _, _, cx| cx.emit(DismissEvent))))
                    .child(btn("ae-add", "Add", Tone::Primary, cx).child(probe("addEngine.add")).on_click(cx.listener(|d, _, _, cx| d.add(cx)))),
            )
    }
}

// ---- Connect -----------------------------------------------------------------------------------------------------------
pub struct ConnectDialog {
    state: Entity<AppState>,
    pub endpoint: String,
    pub ws: String,
    pub token: Entity<InputState>,
    err: Option<String>,
    _subs: Vec<Subscription>,
}
impl EventEmitter<DismissEvent> for ConnectDialog {}

pub fn open_connect(w: &mut Workspace, endpoint: &str, ws: &str, window: &mut Window, cx: &mut Context<Workspace>) {
    let state = w.state.clone();
    let (endpoint, ws) = (endpoint.to_string(), ws.to_string());
    let d = cx.new(|cx| {
        let token = cx.new(|cx| InputState::new(window, cx).masked(true).placeholder("client token"));
        token.update(cx, |i, cx| i.focus(window, cx));
        let sub = cx.subscribe(&token, |d: &mut ConnectDialog, _, e: &InputEvent, cx| {
            if let InputEvent::PressEnter { .. } = e {
                d.connect(cx);
            }
        });
        let mut d = ConnectDialog { state, endpoint, ws, token, err: None, _subs: vec![sub] };
        // UI tests (OPTCHAT_TEST_HOOKS): the test workspace's token from OPTCHAT_TEST_TOKENS, never shown or logged
        if std::env::var("OPTCHAT_TEST_HOOKS").is_ok() {
            d.fill_test_token(window, cx);
        }
        d
    });
    let sub = cx.subscribe_in(&d, window, |w: &mut Workspace, _, _: &DismissEvent, window, cx| w.close_overlay(window, cx));
    w.overlay_subs = vec![sub];
    w.overlay = Some(Overlay::Connect(d));
    cx.notify();
}
impl ConnectDialog {
    pub fn input(&self) -> Entity<InputState> {
        self.token.clone()
    }
    pub fn connect(&mut self, cx: &mut Context<Self>) {
        let t = self.token.read(cx).value().to_string();
        let (ep, ws) = (self.endpoint.clone(), self.ws.clone());
        match self.state.update(cx, |s, cx| s.connect_ws(&ep, &ws, &t, cx)) {
            Ok(()) => cx.emit(DismissEvent),
            Err(e) => {
                self.err = Some(e);
                cx.notify();
            }
        }
    }
    /// tests: the token from OPTCHAT_TEST_TOKENS ({name: token}); never shown or logged
    pub fn fill_test_token(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let m: std::collections::BTreeMap<String, String> = std::env::var("OPTCHAT_TEST_TOKENS").ok().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        if let Some(t) = m.get(&self.ws) {
            let t = t.clone();
            self.token.update(cx, |i, cx| i.set_value(t, window, cx));
        }
    }
}
impl Render for ConnectDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let k = c(cx).clone();
        let engine = self.state.read(cx).engines.iter().find(|e| e.cfg.endpoint == self.endpoint).map(|e| e.cfg.name.clone()).unwrap_or_default();
        set_state_extra("connect", serde_json::json!({ "ws": self.ws, "hasToken": !self.token.read(cx).value().is_empty(), "err": self.err }));
        dialog_box(cx, 420.)
            .child(div().text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).mb(px(6.)).child(format!("Connect {}", self.ws)))
            .child(div().text_size(px(12.)).text_color(k.muted).mb(px(12.)).child(format!("on {engine}. Paste a client token for this workspace (from its admin page). It is kept in the macOS Keychain.")))
            .child(div().mb(px(12.)).child(Input::new(&self.token)).child(probe("connect.token")))
            .when_some(self.err.clone(), |d, e| d.child(div().text_size(px(12.)).text_color(k.red).mb(px(10.)).child(e)))
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .child(btn("cn-cancel", "Cancel", Tone::Ghost, cx).on_click(cx.listener(|_, _, _, cx| cx.emit(DismissEvent))))
                    .child(btn("cn-go", "Connect", Tone::Primary, cx).child(probe("connect.go")).on_click(cx.listener(|d, _, _, cx| d.connect(cx)))),
            )
    }
}

pub fn render_dialog(id: &str, el: AnyView, cx: &mut Context<Workspace>) -> AnyElement {
    overlay_bg(id, cx).on_click(cx.listener(|w, _, window, cx| w.close_overlay(window, cx))).child(div().id(SharedString::from(format!("{id}-inner"))).on_click(|_, _, cx| cx.stop_propagation()).child(el)).into_any_element()
}
