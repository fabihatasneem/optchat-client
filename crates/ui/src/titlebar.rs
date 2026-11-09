//! Client-side window decorations (Linux: GNOME on Wayland has no server-side decorations for GPUI windows, so
//! without this there is no title bar, no close / maximize / minimize button and no resize border).
//! Shown only when the platform says the app must draw them (`Decorations::Client`); macOS and KDE/X11 with
//! server-side decorations are unchanged.
use crate::theme::c;
use crate::ui::icon;
use gpui::*;

pub const HEIGHT: f32 = 34.;
const EDGE: f32 = 5.;

/// Whether this window needs the app to draw its own title bar and resize border.
pub fn client_side(window: &Window) -> bool {
    matches!(window.window_decorations(), Decorations::Client { .. })
}

fn tiling(window: &Window) -> Tiling {
    match window.window_decorations() {
        Decorations::Client { tiling } => tiling,
        Decorations::Server => Tiling::tiled(),
    }
}

fn button(id: &'static str, name: &'static str, danger: bool, cx: &App, on: impl Fn(&mut Window, &mut App) + 'static) -> Stateful<Div> {
    let k = c(cx).clone();
    let hover_bg = if danger { k.red } else { k.hover };
    div()
        .id(id)
        .w(px(30.))
        .h(px(24.))
        .rounded(px(6.))
        .flex()
        .items_center()
        .justify_center()
        .hover(move |s| s.bg(hover_bg))
        .child(icon(name, 14., k.text2))
        // keep the bar's drag / double-click away from the buttons
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(move |_, window, cx| {
            cx.stop_propagation();
            on(window, cx)
        })
}

/// The title bar: the window title, a drag area (double-click maximizes, right-click opens the window menu) and
/// minimize / maximize / close.
pub fn render(title: &str, window: &Window, cx: &App) -> Stateful<Div> {
    let k = c(cx).clone();
    let controls = window.window_controls();
    let maxed = window.is_maximized();
    let mut buttons = div().flex().items_center().gap(px(4.)).pr(px(10.));
    if controls.minimize {
        buttons = buttons.child(button("win-min", "minus", false, cx, |window, _| window.minimize_window()));
    }
    if controls.maximize {
        buttons = buttons.child(button("win-max", if maxed { "copy" } else { "square" }, false, cx, |window, _| window.zoom_window()));
    }
    buttons = buttons.child(button("win-close", "x", true, cx, |window, _| window.remove_window()));
    let title: SharedString = if title.is_empty() { "OptChat".into() } else { title.to_string().into() };
    div()
        .id("titlebar")
        .flex_none()
        .h(px(HEIGHT))
        .w_full()
        .flex()
        .items_center()
        .bg(k.rail)
        .border_b_1()
        .border_color(k.line)
        .on_mouse_down(MouseButton::Left, |e, window, _| {
            if e.click_count >= 2 {
                window.zoom_window();
            } else {
                window.start_window_move();
            }
        })
        .on_mouse_down(MouseButton::Right, |e, window, _| window.show_window_menu(e.position))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .pl(px(14.))
                .text_size(px(12.5))
                .text_color(k.muted)
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .child(title),
        )
        .child(buttons.child(crate::ui::probe("titlebar.buttons")))
}

/// Invisible resize handles along the window's edges and corners (none on tiled / maximized sides).
pub fn edges(window: &Window) -> Vec<AnyElement> {
    if window.is_maximized() || window.is_fullscreen() {
        return vec![];
    }
    let t = tiling(window);
    let e = px(EDGE);
    let c2 = px(EDGE * 2.);
    let mut out = vec![];
    let mut add = |id: &'static str, edge: ResizeEdge, cursor: CursorStyle, d: Div| {
        out.push(
            d.id(id)
                .absolute()
                .occlude()
                .cursor(cursor)
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    cx.stop_propagation();
                    window.start_window_resize(edge)
                })
                .into_any_element(),
        );
    };
    if !t.top {
        add("rs-t", ResizeEdge::Top, CursorStyle::ResizeUpDown, div().top_0().left(c2).right(c2).h(e));
    }
    if !t.bottom {
        add("rs-b", ResizeEdge::Bottom, CursorStyle::ResizeUpDown, div().bottom_0().left(c2).right(c2).h(e));
    }
    if !t.left {
        add("rs-l", ResizeEdge::Left, CursorStyle::ResizeLeftRight, div().left_0().top(c2).bottom(c2).w(e));
    }
    if !t.right {
        add("rs-r", ResizeEdge::Right, CursorStyle::ResizeLeftRight, div().right_0().top(c2).bottom(c2).w(e));
    }
    if !t.top && !t.left {
        add("rs-tl", ResizeEdge::TopLeft, CursorStyle::ResizeUpLeftDownRight, div().top_0().left_0().size(c2));
    }
    if !t.top && !t.right {
        add("rs-tr", ResizeEdge::TopRight, CursorStyle::ResizeUpRightDownLeft, div().top_0().right_0().size(c2));
    }
    if !t.bottom && !t.left {
        add("rs-bl", ResizeEdge::BottomLeft, CursorStyle::ResizeUpRightDownLeft, div().bottom_0().left_0().size(c2));
    }
    if !t.bottom && !t.right {
        add("rs-br", ResizeEdge::BottomRight, CursorStyle::ResizeUpLeftDownRight, div().bottom_0().right_0().size(c2));
    }
    out
}
