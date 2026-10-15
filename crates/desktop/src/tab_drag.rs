use gpui_kit::component::ActiveTheme;
use gpui_kit::*;
#[derive(Clone)]
pub struct DraggedRequest {
    pub path: std::path::PathBuf,
    pub title: String,
}
pub struct Preview {
    pub title: String,
}
impl Render for Preview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_3()
            .py_2()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .border_1()
            .border_color(cx.theme().border)
            .rounded(cx.theme().radius)
            .text_sm()
            .child(self.title.clone())
    }
}
