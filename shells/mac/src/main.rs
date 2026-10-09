use gpui::*;
use gpui_component::{Root, input::{InputState, Input}};

struct Hello { input: Entity<InputState> }
impl Render for Hello {
    fn render(&mut self, _w: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().bg(rgb(0x0b0d10)).text_color(rgb(0xe7e9ec)).p_4().child("engine").child(Input::new(&self.input))
    }
}
fn main() {
    Application::new().run(|cx: &mut App| {
        gpui_component::init(cx);
        cx.open_window(WindowOptions::default(), |window, cx| {
            let input = cx.new(|cx| InputState::new(window, cx).multi_line(true));
            let view = cx.new(|_| Hello { input });
            cx.new(|cx| Root::new(view, window, cx))
        }).unwrap();
        cx.activate(true);
    });
}
