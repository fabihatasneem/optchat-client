//! OptChat for Mac, GPUI edition: a native client for OptChat's endless, task-tree chat. The engine (crates/engine)
//! runs on its own tokio runtime; this process draws the UI with GPUI and never blocks its thread on I/O.
mod composer;
mod dialogs;
mod feed;
mod fmt;
mod markdown;
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

actions!(app, [Quit, About]);

fn default_db() -> String {
    if let Ok(p) = std::env::var("OPTCHAT_DB") {
        return p;
    }
    let home = std::env::var("HOME").unwrap_or_default();
    format!("{home}/.reframe/agent-lab/optchat-mac-gpui/data/optchat.db")
}

fn main() {
    LAUNCH.get_or_init(std::time::Instant::now);
    // an app started from Finder has a bare PATH: give children the usual tool dirs (claude, codex, git, ...)
    std::env::set_var("PATH", optchat_engine::util::child_path());
    let db = default_db();
    let data_dir = format!("{}/optchat-gpui", std::path::Path::new(&db).parent().map(|p| p.display().to_string()).unwrap_or(".".into()));
    let _ = std::fs::create_dir_all(&data_dir);
    let prefs_path = format!("{data_dir}/prefs.json");
    let rt: &'static tokio::runtime::Runtime = Box::leak(Box::new(tokio::runtime::Builder::new_multi_thread().worker_threads(4).thread_name("optchat-engine").enable_all().build().expect("tokio runtime")));
    ui::start_probe_writer();

    Application::new().with_assets(Assets).run(move |cx: &mut App| {
        gpui_component::init(cx);
        let prefs: state::Prefs = std::fs::read_to_string(&prefs_path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        theme::init(cx, prefs.theme.clone(), prefs.follow_system, cx.window_appearance());
        gpui_component::Theme::global_mut(cx).font_size = px(prefs.font_size - 0.5);
        workspace::bind(cx);
        composer::bind(cx);
        dialogs::bind(cx);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
        set_menus(cx);

        let state = state::new_state(cx, rt.handle().clone(), prefs_path.clone());
        let size = size(px(1360.), px(880.));
        let bounds = Bounds::centered(None, size, cx);
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

        // the engine: open the database, load every task, start the loopback server — off the UI thread
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let (htx, hrx) = tokio::sync::oneshot::channel();
        let db2 = db.clone();
        let dd = data_dir.clone();
        rt.spawn(async move {
            let reframe = std::env::var("OPTCHAT_REFRAME_MCP").map(|v| v != "0").unwrap_or(true);
            let r = optchat_engine::hub::Hub::create(optchat_engine::hub::HubOpts { db_path: db2, data_dir: dd, reframe_mcp: reframe }, tx).await;
            let _ = htx.send(r.map_err(|e| e.to_string()));
        });
        let st3 = state.clone();
        cx.spawn(async move |cx| {
            match hrx.await {
                Ok(Ok(hub)) => {
                    let _ = st3.update(cx, |s, cx| {
                        s.stats.launch_ms = LAUNCH.get().unwrap().elapsed().as_secs_f64() * 1000.0;
                        s.attach(hub, rx, cx)
                    });
                }
                other => {
                    let msg = match other { Ok(Err(e)) => e, _ => "the engine did not start".into() };
                    let _ = st3.update(cx, |s, cx| {
                        s.error = Some(msg);
                        cx.notify();
                    });
                }
            }
        })
        .detach();
        let st4 = state.clone();
        cx.on_app_quit(move |cx| {
            let hub = st4.read(cx).hub();
            async move {
                if let Some(h) = hub {
                    h.shutdown();
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
        Menu { name: "OptChat".into(), items: vec![MenuItem::action("Settings…", ShowSettings), MenuItem::separator(), MenuItem::os_submenu("Services", SystemMenuType::Services), MenuItem::separator(), MenuItem::action("Quit OptChat", Quit)] },
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
        Menu { name: "Window".into(), items: vec![] },
    ]);
}
