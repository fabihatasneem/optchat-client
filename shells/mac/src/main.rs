//! OptChat for Mac, GPUI edition: a native client of the OptChat hub (the web app's engine at chat.reframe.dev),
//! an equal of the browser and the phone: one websocket, everything in sync. The connection runs on its own tokio
//! runtime; this process draws the UI with GPUI and never blocks its thread on I/O.
#![recursion_limit = "256"] // the test state's json! literal
mod client;
mod composer;
mod dialogs;
mod feed;
mod fmt;
mod markdown;
mod media;
mod mic;
mod panels;
mod state;
mod statusbar;
mod tabs;
mod tasks_view;
mod theme;
mod ui;
mod workspace;

use gpui::*;
use gpui_component::Root;
use std::sync::OnceLock;

pub static LAUNCH: OnceLock<std::time::Instant> = OnceLock::new();

mod assets {
    include!(concat!(env!("OUT_DIR"), "/assets.rs"));
}
struct Assets;
impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<std::borrow::Cow<'static, [u8]>>> {
        Ok(assets::ASSETS.iter().find(|(p, _)| *p == path).map(|(_, b)| std::borrow::Cow::Borrowed(*b)))
    }
    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(assets::ASSETS.iter().filter(|(p, _)| p.starts_with(path)).map(|(p, _)| SharedString::from(*p)).collect())
    }
}

actions!(app, [Quit, Hide, HideOthers, ShowAll]);

/// Per-viewer prefs (tabs, sidebar, theme, window): `~/Library/Application Support/com.reframe.optchat.gpui`, one file
/// per hub instance (task ids differ between instances); OPTCHAT_PREFS_DIR overrides the folder (tests).
fn prefs_path(cfg: &client::HubCfg) -> String {
    let dir = std::env::var("OPTCHAT_PREFS_DIR").unwrap_or_else(|_| format!("{}/Library/Application Support/com.reframe.optchat.gpui", std::env::var("HOME").unwrap_or_default()));
    let _ = std::fs::create_dir_all(&dir);
    format!("{dir}/prefs{}.json", cfg.db.as_ref().map(|d| format!("-{d}")).unwrap_or_default())
}

fn main() {
    LAUNCH.get_or_init(std::time::Instant::now);
    let cfg = match client::HubCfg::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    let prefs_path = prefs_path(&cfg);
    let rt: &'static tokio::runtime::Runtime = Box::leak(Box::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).thread_name("optchat-client").enable_all().build().expect("tokio runtime")));
    ui::start_probe_writer();

    Application::new().with_assets(Assets).run(move |cx: &mut App| {
        gpui_component::init(cx);
        let prefs: state::Prefs = std::fs::read_to_string(&prefs_path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        theme::init(cx, prefs.theme.clone(), prefs.follow_system, cx.window_appearance());
        gpui_component::Theme::global_mut(cx).font_size = px(prefs.font_size - 0.5);
        crate::ui::set_zoom(prefs.font_size);
        workspace::bind(cx);
        composer::bind(cx);
        dialogs::bind(cx);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.on_action(|_: &Hide, cx| cx.hide());
        cx.on_action(|_: &HideOthers, cx| cx.hide_other_apps());
        cx.on_action(|_: &ShowAll, cx| cx.unhide_other_apps());
        cx.bind_keys([
            KeyBinding::new("cmd-q", Quit, None),
            KeyBinding::new("cmd-h", Hide, None),
            KeyBinding::new("alt-cmd-h", HideOthers, None),
            KeyBinding::new("cmd-m", workspace::Minimize, None),
            KeyBinding::new("ctrl-cmd-f", workspace::ToggleFullScreen, None),
            KeyBinding::new("cmd-/", workspace::ShowShortcuts, None),
        ]);
        set_menus(cx);

        let state = state::new_state(cx, rt.handle().clone(), prefs_path.clone());
        // the last window bounds, when they are sane; else 1360×880 centered
        let bounds = match prefs.window {
            Some([x, y, w, h]) if w >= 560. && h >= 420. && w < 10000. && h < 10000. && x > -5000. && y > -5000. => Bounds::new(point(px(x), px(y)), size(px(w), px(h))),
            _ => Bounds::centered(None, size(px(1360.), px(880.)), cx),
        };
        let st2 = state.clone();
        let win = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions { title: Some("OptChat".into()), appears_transparent: false, traffic_light_position: None }),
                    window_min_size: Some(gpui::size(px(560.), px(420.))),
                    app_id: Some("com.reframe.optchat.gpui".into()),
                    focus: true,
                    show: true,
                    ..Default::default()
                },
                |window, cx| {
                    theme::sync_window(window, cx);
                    let ws = cx.new(|cx| workspace::Workspace::new(st2.clone(), window, cx));
                    ws.update(cx, |w, cx| w.composer.update(cx, |c, cx| c.focus(window, cx)));
                    cx.new(|cx| Root::new(ws, window, cx))
                },
            )
            .expect("open window");
        let _ = win;
        cx.activate(true);

        // the hub: one websocket (snapshot, events, commands, rpc), reconnecting by itself
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let first = state.read(cx).current;
        let client = {
            let _g = rt.enter();
            client::Client::start(cfg.clone(), first, tx)
        };
        state.update(cx, |s, cx| {
            s.stats.launch_ms = LAUNCH.get().unwrap().elapsed().as_secs_f64() * 1000.0;
            s.attach(client, rx, cx)
        });
        let st4 = state.clone();
        cx.on_app_quit(move |cx| {
            let c = st4.read(cx).hub();
            async move {
                if let Some(c) = c {
                    c.shutdown();
                }
            }
        })
        .detach();
    });
}


fn set_menus(cx: &mut App) {
    use gpui_component::input::{Copy, Cut, Paste, Redo, SelectAll, Undo};
    use workspace::*;
    cx.set_menus(vec![
        Menu {
            name: "OptChat".into(),
            items: vec![
                MenuItem::action("About OptChat", About),
                MenuItem::separator(),
                MenuItem::action("Settings…", ShowSettings),
                MenuItem::separator(),
                MenuItem::os_submenu("Services", SystemMenuType::Services),
                MenuItem::separator(),
                MenuItem::action("Hide OptChat", Hide),
                MenuItem::action("Hide Others", HideOthers),
                MenuItem::action("Show All", ShowAll),
                MenuItem::separator(),
                MenuItem::action("Quit OptChat", Quit),
            ],
        },
        Menu { name: "File".into(), items: vec![MenuItem::action("New Task…", NewTask), MenuItem::action("Open Main Chat", OpenMainChat), MenuItem::action("Go to Task…", TaskSwitcher), MenuItem::separator(), MenuItem::action("Close Tab", CloseTab)] },
        Menu {
            name: "Edit".into(),
            items: vec![
                MenuItem::os_action("Undo", Undo, OsAction::Undo),
                MenuItem::os_action("Redo", Redo, OsAction::Redo),
                MenuItem::separator(),
                MenuItem::os_action("Cut", Cut, OsAction::Cut),
                MenuItem::os_action("Copy", Copy, OsAction::Copy),
                MenuItem::os_action("Paste", Paste, OsAction::Paste),
                MenuItem::os_action("Select All", SelectAll, OsAction::SelectAll),
            ],
        },
        Menu {
            name: "View".into(),
            items: vec![
                MenuItem::action("Command Palette…", CommandPalette),
                MenuItem::separator(),
                MenuItem::action("Toggle Sidebar", ToggleSidebar),
                MenuItem::action("Tasks", ShowTasks),
                MenuItem::action("Memory", ShowMemory),
                MenuItem::action("Context", ShowContext),
                MenuItem::action("Log", ShowLog),
                MenuItem::action("Models", ShowModels),
                MenuItem::separator(),
                MenuItem::action("Next Theme", NextTheme),
                MenuItem::action("Bigger Text", ZoomIn),
                MenuItem::action("Smaller Text", ZoomOut),
                MenuItem::action("Actual Size", ZoomReset),
                MenuItem::separator(),
                MenuItem::action("Next Tab", NextTab),
                MenuItem::action("Previous Tab", PrevTab),
            ],
        },
        Menu {
            name: "Task".into(),
            items: vec![
                MenuItem::action("Focus Composer", FocusComposer),
                MenuItem::action("Stop the Running Turn", StopTurn),
                MenuItem::separator(),
                MenuItem::action("Pause…", PauseTask),
                MenuItem::action("Resume…", ResumeTask),
                MenuItem::action("Close as Done…", CloseTask),
                MenuItem::action("Cancel…", CancelTask),
            ],
        },
        Menu {
            name: "Window".into(),
            items: vec![
                MenuItem::action("Minimize", Minimize),
                MenuItem::action("Zoom", ZoomWindow),
                MenuItem::action("Enter Full Screen", ToggleFullScreen),
                MenuItem::separator(),
                MenuItem::action("Show Next Tab", NextTab),
                MenuItem::action("Show Previous Tab", PrevTab),
                MenuItem::action("Main Chat", OpenMainChat),
            ],
        },
        Menu {
            name: "Help".into(),
            items: vec![MenuItem::action("Keyboard Shortcuts", ShowShortcuts), MenuItem::action("Commands…", CommandPalette), MenuItem::separator(), MenuItem::action("Show Data Folder", OpenDataFolder)],
        },
    ]);
}
