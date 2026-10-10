//! A test channel (OPTCHAT_TEST_HOOKS with OPTCHAT_PROBES): commands in `<probes>.cmd` (a JSON array, taken and
//! deleted every 250 ms) run through the app's own paths: the workspace menu's and dialogs' handlers, the state's
//! methods, keystrokes through the window's dispatch. For runs where no input can reach the window (a locked screen:
//! macOS draws nothing and Computer Use finds no window); the visual checks stay with tests/workspaces.ts.
use crate::state::*;
use crate::workspace::Workspace;
use gpui::*;
use serde_json::Value;

pub fn poll(w: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    if std::env::var("OPTCHAT_TEST_HOOKS").is_err() {
        return;
    }
    let Ok(p) = std::env::var("OPTCHAT_PROBES") else { return };
    let path = format!("{p}.cmd");
    let Ok(text) = std::fs::read_to_string(&path) else { return };
    let _ = std::fs::remove_file(&path);
    let cmds: Vec<Value> = serde_json::from_str(&text).unwrap_or_default();
    for c in cmds {
        run(w, &c, window, cx);
    }
    cx.notify();
}

fn endpoint(w: &Workspace, c: &Value, cx: &App) -> String {
    c["endpoint"].as_str().map(String::from).unwrap_or_else(|| w.state.read(cx).engines.first().map(|e| e.cfg.endpoint.clone()).unwrap_or_default())
}

fn run(w: &mut Workspace, c: &Value, window: &mut Window, cx: &mut Context<Workspace>) {
    let str_ = |k: &str| c[k].as_str().unwrap_or("").to_string();
    let mut done = serde_json::json!({ "op": c["op"], "ok": true });
    match c["op"].as_str().unwrap_or("") {
        "act" => {
            let name: &'static str = Box::leak(str_("name").into_boxed_str());
            crate::dialogs::run_cmd(w, crate::dialogs::Cmd::Act(name), window, cx);
        }
        "key" => {
            if let Ok(k) = Keystroke::parse(&str_("keys")) {
                window.dispatch_keystroke(k, cx);
            }
        }
        "menu" => crate::workspaces::open_menu(w, cx),
        "addEngine" => {
            crate::workspaces::open_add_engine(w, window, cx);
            if let Some(crate::dialogs::Overlay::AddEngine(d)) = w.overlay.clone() {
                let (ep, name) = (str_("endpoint"), str_("name"));
                d.update(cx, |d, cx| {
                    d.endpoint.update(cx, |i, cx| i.set_value(ep, window, cx));
                    d.set_name(&name, window, cx);
                    d.add(cx);
                });
            }
        }
        "connect" => {
            // the menu's Connect: the dialog (its token from the test file), then its Connect button's handler
            let ep = endpoint(w, c, cx);
            crate::workspaces::open_connect(w, &ep, &str_("ws"), window, cx);
            if let Some(crate::dialogs::Overlay::Connect(d)) = w.overlay.clone() {
                d.update(cx, |d, cx| d.connect(cx));
            }
        }
        "switch" => {
            let key = crate::client::ws_key(&endpoint(w, c, cx), &str_("ws"));
            w.switch_workspace(&key, window, cx);
        }
        "disconnect" => {
            let key = crate::client::ws_key(&endpoint(w, c, cx), &str_("ws"));
            w.state.update(cx, |s, cx| s.disconnect_ws(&key, cx));
        }
        "view" => {
            let v: View = serde_json::from_value(c["view"].clone()).unwrap_or(View::Tasks);
            w.state.update(cx, |s, _| s.prefs.view = v);
            w.set_side_open(true, cx);
        }
        "open" => w.open_task(c["task"].as_i64().unwrap_or(0), window, cx),
        "loadNotices" => w.state.update(cx, |s, cx| s.load_notices(cx)),
        "readNotice" => {
            let id = str_("id");
            let task = w.state.read(cx).notices.iter().find(|n| n.id == id).and_then(|n| n.task);
            w.state.update(cx, |s, cx| s.read_notices(Some(vec![id]), None, cx));
            if let Some(t) = task {
                w.open_task(t, window, cx);
            }
        }
        "readAll" => w.state.update(cx, |s, cx| s.read_notices(None, None, cx)),
        "spawn" => {
            // as the new-task dialog's Start does
            let (title, instruction) = (str_("title"), str_("instruction"));
            let usd = c["usd"].as_f64();
            let parent = w.state.read(cx).current;
            w.state.update(cx, |s, cx| {
                s.run(
                    cx,
                    move |h| async move { h.spawn(parent, "handoff", &title, &instruction, vec![], None, usd.map(|u| optchat_proto::api::BudgetArgs { usd: Some(u), ..Default::default() }), None, true).await },
                    move |s, r, cx| match r {
                        Ok(id) => cx.emit(StateEvent::SpawnedOpen(id)),
                        Err(e) => s.local_notice("error", e),
                    },
                )
            });
        }
        "close" => w.close_overlay(window, cx),
        "quit" => cx.quit(),
        other => done = serde_json::json!({ "op": other, "ok": false }),
    }
    crate::ui::set_state_extra("lastCmd", done);
}
