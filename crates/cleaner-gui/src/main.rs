use gpui::{
    App, Bounds, Context, Window, WindowBounds, WindowOptions, div, prelude::*, px, rgb, size,
};
use gpui_platform::application;

struct CleanerApp;

impl Render for CleanerApp {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .size_full()
            .bg(rgb(0x0f1115))
            .text_color(rgb(0xe8eaed))
            .child(sidebar())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .p_8()
                    .gap_6()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(div().text_2xl().child("Smart Care"))
                            .child(
                                div()
                                    .px_3()
                                    .py_1()
                                    .rounded_full()
                                    .bg(rgb(0x173624))
                                    .text_color(rgb(0x83e6a2))
                                    .child("Safe mode · dry-run"),
                            ),
                    )
                    .child(
                        div()
                            .p_6()
                            .rounded_xl()
                            .bg(rgb(0x171a20))
                            .border_1()
                            .border_color(rgb(0x262a33))
                            .flex()
                            .flex_col()
                            .gap_4()
                            .child(div().text_xl().child("Reclaim your Mac"))
                            .child(
                                div().text_color(rgb(0xa9afb8)).child(
                                    "Scan first. Review every cleanup plan before execution.",
                                ),
                            )
                            .child(
                                div()
                                    .id("scan")
                                    .w(px(150.0))
                                    .px_5()
                                    .py_3()
                                    .rounded_lg()
                                    .bg(rgb(0x4f7cff))
                                    .cursor_pointer()
                                    .child("Start Smart Scan"),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_4()
                            .child(metric_card("User Cache", "Ready"))
                            .child(metric_card("Xcode", "Ready"))
                            .child(metric_card("Homebrew", "Ready")),
                    )
                    .child(
                        div()
                            .p_5()
                            .rounded_xl()
                            .bg(rgb(0x171a20))
                            .border_1()
                            .border_color(rgb(0x262a33))
                            .child("M0 foundation: destructive cleanup is disabled."),
                    ),
            )
    }
}

fn sidebar() -> impl IntoElement {
    div()
        .w(px(220.0))
        .h_full()
        .p_5()
        .bg(rgb(0x13161b))
        .border_r_1()
        .border_color(rgb(0x262a33))
        .flex()
        .flex_col()
        .gap_3()
        .child(div().text_xl().child("Dxtr Cleaner"))
        .child(nav_item("Smart Care", true))
        .child(nav_item("Cleanup", false))
        .child(nav_item("Uninstaller", false))
        .child(nav_item("Orphans", false))
        .child(nav_item("Settings", false))
}

fn nav_item(label: &'static str, selected: bool) -> impl IntoElement {
    div()
        .px_3()
        .py_2()
        .rounded_md()
        .when(selected, |item| item.bg(rgb(0x222733)))
        .child(label)
}

fn metric_card(title: &'static str, value: &'static str) -> impl IntoElement {
    div()
        .flex_1()
        .p_4()
        .rounded_lg()
        .bg(rgb(0x171a20))
        .border_1()
        .border_color(rgb(0x262a33))
        .flex()
        .flex_col()
        .gap_2()
        .child(div().text_color(rgb(0xa9afb8)).child(title))
        .child(div().text_lg().child(value))
}

fn main() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1080.0), px(720.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|_| CleanerApp),
        )
        .expect("open main window");
        cx.activate(true);
    });
}
