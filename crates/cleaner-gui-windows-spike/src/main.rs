use std::{
    env,
    path::PathBuf,
    sync::mpsc::{self, TryRecvError},
    thread,
    time::Duration,
};

use cleaner_core::{
    CancellationToken, CleanupCategory, FileSystemScanner, ScanEvent, ScanRequest, Scanner,
};
use gpui::{
    App, Bounds, Context, Render, Window, WindowBounds, WindowOptions, div, prelude::*, px, rgb,
    size,
};
use gpui_platform::application;

const MAX_EVENTS_PER_TICK: usize = 128;

enum ScanMessage {
    Event(ScanEvent),
    Failed(String),
}

struct WindowsSpike {
    root: Option<PathBuf>,
    scanning: bool,
    items: usize,
    bytes: u64,
    permission_denied: usize,
    error: Option<String>,
}

impl WindowsSpike {
    fn new() -> Self {
        Self {
            root: env::args_os().nth(1).map(PathBuf::from),
            scanning: false,
            items: 0,
            bytes: 0,
            permission_denied: 0,
            error: None,
        }
    }

    fn start_scan(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.scanning {
            return;
        }
        let Some(root) = self.root.clone() else {
            self.error = Some(
                "Pass a disposable directory as the first command-line argument to run the spike."
                    .into(),
            );
            cx.notify();
            return;
        };

        self.scanning = true;
        self.items = 0;
        self.bytes = 0;
        self.permission_denied = 0;
        self.error = None;

        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let request = ScanRequest {
                category: CleanupCategory::UserCache,
                roots: vec![root],
                excluded_roots: Vec::new(),
            };
            let cancellation = CancellationToken::new();
            let scanner = FileSystemScanner;
            let mut sink = |event| {
                let _ = tx.send(ScanMessage::Event(event));
            };
            if let Err(error) = scanner.scan_with(&request, &cancellation, &mut sink) {
                let _ = tx.send(ScanMessage::Failed(error.to_string()));
            }
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
                            Ok(ScanMessage::Event(ScanEvent::ItemFound { item })) => {
                                entity.update(cx, |this, cx| {
                                    this.items += 1;
                                    this.bytes = this.bytes.saturating_add(item.bytes);
                                    cx.notify();
                                });
                            }
                            Ok(ScanMessage::Event(ScanEvent::PermissionDenied { .. })) => {
                                entity.update(cx, |this, cx| {
                                    this.permission_denied += 1;
                                    cx.notify();
                                });
                            }
                            Ok(ScanMessage::Event(ScanEvent::Finished { .. }))
                            | Ok(ScanMessage::Event(ScanEvent::Cancelled { .. })) => {
                                entity.update(cx, |this, cx| {
                                    this.scanning = false;
                                    cx.notify();
                                });
                                terminal = true;
                                break;
                            }
                            Ok(ScanMessage::Event(ScanEvent::Started { .. })) => {}
                            Ok(ScanMessage::Failed(error)) => {
                                entity.update(cx, |this, cx| {
                                    this.scanning = false;
                                    this.error = Some(error);
                                    cx.notify();
                                });
                                terminal = true;
                                break;
                            }
                            Err(TryRecvError::Empty) => break,
                            Err(TryRecvError::Disconnected) => {
                                entity.update(cx, |this, cx| {
                                    this.scanning = false;
                                    cx.notify();
                                });
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
}

impl Render for WindowsSpike {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let root = self
            .root
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "No directory selected".into());
        let status = if self.scanning {
            "Scanning…"
        } else {
            "Ready"
        };

        div()
            .size_full()
            .p_8()
            .bg(rgb(0x0f1115))
            .text_color(rgb(0xe8eaed))
            .flex()
            .flex_col()
            .gap_5()
            .child(div().text_2xl().child("Dxtr Cleaner · Windows GPUI Spike"))
            .child(div().text_color(rgb(0xa9afb8)).child(root))
            .child(div().child(format!(
                "{status} · {} item(s) · {} bytes · {} permission-denied path(s)",
                self.items, self.bytes, self.permission_denied
            )))
            .when_some(self.error.clone(), |view, error| {
                view.child(div().text_color(rgb(0xffa6a6)).child(error))
            })
            .child(
                div()
                    .id("scan")
                    .px_5()
                    .py_3()
                    .rounded_lg()
                    .bg(rgb(0x4f7cff))
                    .cursor_pointer()
                    .child(if self.scanning {
                        "Scanning…"
                    } else {
                        "Scan directory"
                    })
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.start_scan(window, cx);
                    })),
            )
            .child(div().text_color(rgb(0xa9afb8)).child(
                "Read-only feasibility harness: no Recycle Bin or delete operation is wired yet.",
            ))
    }
}

fn main() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(900.0), px(560.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|_| WindowsSpike::new()),
        )
        .expect("open Windows GPUI spike window");
        cx.activate(true);
    });
}
