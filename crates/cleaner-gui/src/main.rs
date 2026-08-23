use std::{
    env,
    path::PathBuf,
    sync::mpsc::{self, TryRecvError},
    thread,
    time::Duration,
};

use cleaner_core::{
    CancellationToken, CategoryScanTarget, CleanupCategory, FileSystemScanner, HomebrewScan,
    NodeScan, ScanEvent, Scanner, SystemCacheScan, UserCacheScan, XcodeScan,
};
use gpui::{
    App, Bounds, Context, Window, WindowBounds, WindowOptions, div, prelude::*, px, rgb, size,
};
use gpui_platform::application;

const MAX_EVENTS_PER_TICK: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScanState {
    Idle,
    Scanning,
    Cancelling,
    Completed,
    Cancelled,
    Failed,
}

impl ScanState {
    fn label(self) -> &'static str {
        match self {
            Self::Idle => "Ready",
            Self::Scanning => "Scanning…",
            Self::Cancelling => "Cancelling…",
            Self::Completed => "Completed",
            Self::Cancelled => "Cancelled",
            Self::Failed => "Failed",
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct Metric {
    items: usize,
    bytes: u64,
}

enum UiMessage {
    Event(ScanEvent),
    Completed,
    Cancelled,
    Failed(String),
}

struct CleanerApp {
    state: ScanState,
    user_cache: Metric,
    system_cache: Metric,
    xcode: Metric,
    homebrew: Metric,
    node: Metric,
    permission_denied: usize,
    error: Option<String>,
    cancellation: Option<CancellationToken>,
}

impl CleanerApp {
    fn new() -> Self {
        Self {
            state: ScanState::Idle,
            user_cache: Metric::default(),
            system_cache: Metric::default(),
            xcode: Metric::default(),
            homebrew: Metric::default(),
            node: Metric::default(),
            permission_denied: 0,
            error: None,
            cancellation: None,
        }
    }

    fn reset_metrics(&mut self) {
        self.user_cache = Metric::default();
        self.system_cache = Metric::default();
        self.xcode = Metric::default();
        self.homebrew = Metric::default();
        self.node = Metric::default();
        self.permission_denied = 0;
        self.error = None;
    }

    fn metric_mut(&mut self, category: CleanupCategory) -> Option<&mut Metric> {
        match category {
            CleanupCategory::UserCache => Some(&mut self.user_cache),
            CleanupCategory::SystemCache => Some(&mut self.system_cache),
            CleanupCategory::Xcode => Some(&mut self.xcode),
            CleanupCategory::Homebrew => Some(&mut self.homebrew),
            CleanupCategory::Node => Some(&mut self.node),
            _ => None,
        }
    }

    fn apply_message(&mut self, message: UiMessage) {
        match message {
            UiMessage::Event(ScanEvent::ItemFound { item }) => {
                if let Some(metric) = self.metric_mut(item.category) {
                    metric.items += 1;
                    metric.bytes = metric.bytes.saturating_add(item.bytes);
                }
            }
            UiMessage::Event(ScanEvent::PermissionDenied { .. }) => {
                self.permission_denied += 1;
            }
            UiMessage::Event(_) => {}
            UiMessage::Completed => {
                self.state = ScanState::Completed;
                self.cancellation = None;
            }
            UiMessage::Cancelled => {
                self.state = ScanState::Cancelled;
                self.cancellation = None;
            }
            UiMessage::Failed(error) => {
                self.state = ScanState::Failed;
                self.error = Some(error);
                self.cancellation = None;
            }
        }
    }

    fn start_scan(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.state, ScanState::Scanning | ScanState::Cancelling) {
            return;
        }

        let Some(home) = env::var_os("HOME").map(PathBuf::from) else {
            self.state = ScanState::Failed;
            self.error = Some("HOME is not set".to_string());
            cx.notify();
            return;
        };

        self.reset_metrics();
        self.state = ScanState::Scanning;

        let cancellation = CancellationToken::new();
        self.cancellation = Some(cancellation.clone());

        let requests = vec![
            UserCacheScan::new(home.clone()).request(),
            SystemCacheScan.request(),
            XcodeScan::new(home.clone()).request(),
            HomebrewScan::new(home.clone()).request(),
            NodeScan::new(home).request(),
        ];

        let (tx, rx) = mpsc::channel::<UiMessage>();
        let worker_cancellation = cancellation.clone();

        thread::spawn(move || {
            let scanner = FileSystemScanner;

            for request in requests {
                if worker_cancellation.is_cancelled() {
                    let _ = tx.send(UiMessage::Cancelled);
                    return;
                }

                let event_tx = tx.clone();
                let mut sink = move |event| {
                    let _ = event_tx.send(UiMessage::Event(event));
                };

                if let Err(error) = scanner.scan_with(&request, &worker_cancellation, &mut sink) {
                    let _ = tx.send(UiMessage::Failed(error.to_string()));
                    return;
                }
            }

            let terminal = if worker_cancellation.is_cancelled() {
                UiMessage::Cancelled
            } else {
                UiMessage::Completed
            };
            let _ = tx.send(terminal);
        });

        let entity = cx.entity();
        window
            .spawn(cx, async move |cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(50))
                        .await;

                    let mut terminal = false;
                    for _ in 0..MAX_EVENTS_PER_TICK {
                        match rx.try_recv() {
                            Ok(message) => {
                                terminal = matches!(
                                    message,
                                    UiMessage::Completed
                                        | UiMessage::Cancelled
                                        | UiMessage::Failed(_)
                                );
                                entity.update(cx, |this, cx| {
                                    this.apply_message(message);
                                    cx.notify();
                                });

                                if terminal {
                                    break;
                                }
                            }
                            Err(TryRecvError::Empty) => break,
                            Err(TryRecvError::Disconnected) => {
                                terminal = true;
                                break;
                            }
                        }
                    }

                    if terminal {
                        break;
                    }
                }
            })
            .detach();

        cx.notify();
    }

    fn cancel_scan(&mut self, cx: &mut Context<Self>) {
        let Some(cancellation) = &self.cancellation else {
            return;
        };

        cancellation.cancel();
        self.state = ScanState::Cancelling;
        cx.notify();
    }
}

impl Render for CleanerApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_active = matches!(self.state, ScanState::Scanning | ScanState::Cancelling);
        let primary_label = if is_active {
            self.state.label()
        } else {
            "Start Smart Scan"
        };

        let status_text = match &self.error {
            Some(error) => format!("{} · {error}", self.state.label()),
            None if self.permission_denied > 0 => format!(
                "{} · {} permission-denied path(s)",
                self.state.label(),
                self.permission_denied
            ),
            None => self.state.label().to_string(),
        };

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
                            .child(div().text_color(rgb(0xa9afb8)).child(status_text))
                            .child(
                                div()
                                    .flex()
                                    .gap_3()
                                    .child(
                                        div()
                                            .id("scan")
                                            .w(px(160.0))
                                            .px_5()
                                            .py_3()
                                            .rounded_lg()
                                            .bg(rgb(0x4f7cff))
                                            .cursor_pointer()
                                            .child(primary_label)
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                if !matches!(
                                                    this.state,
                                                    ScanState::Scanning | ScanState::Cancelling
                                                ) {
                                                    this.start_scan(window, cx);
                                                }
                                            })),
                                    )
                                    .when(is_active, |row| {
                                        row.child(
                                            div()
                                                .id("cancel")
                                                .px_5()
                                                .py_3()
                                                .rounded_lg()
                                                .bg(rgb(0x2b303a))
                                                .cursor_pointer()
                                                .child("Cancel")
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.cancel_scan(cx);
                                                })),
                                        )
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_4()
                            .child(metric_card("User Cache", self.user_cache))
                            .child(metric_card("System Cache", self.system_cache))
                            .child(metric_card("Xcode", self.xcode))
                            .child(metric_card("Homebrew", self.homebrew))
                            .child(metric_card("Node", self.node)),
                    )
                    .child(
                        div()
                            .p_5()
                            .rounded_xl()
                            .bg(rgb(0x171a20))
                            .border_1()
                            .border_color(rgb(0x262a33))
                            .child(
                                "M1 Smart Scan is read-only. Destructive cleanup remains disabled.",
                            ),
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

fn metric_card(title: &'static str, metric: Metric) -> impl IntoElement {
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
        .child(
            div()
                .text_lg()
                .child(format_bytes(metric.bytes))
                .child(format!(" · {} item(s)", metric.items)),
        )
}

fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    let bytes = bytes as f64;
    if bytes >= GB {
        format!("{:.1} GB", bytes / GB)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes / MB)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes / KB)
    } else {
        format!("{} B", bytes as u64)
    }
}

fn main() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1080.0), px(720.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|_| CleanerApp::new()),
        )
        .expect("open main window");
        cx.activate(true);
    });
}
