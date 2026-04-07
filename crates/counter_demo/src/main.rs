use std::time::{Duration, Instant};

use gpui::{
    div, prelude::*, px, rgb, size, App, Bounds, Context, ElementId, IntoElement, MouseButton,
    Render, Window, WindowBounds, WindowOptions,
};
use gpui_platform::application;

/// Delay before auto-repeat starts (same idea as keyboard / spinner controls).
const HOLD_REPEAT_DELAY_MS: u64 = 320;

#[derive(Clone, Copy)]
enum RepeatDirection {
    Inc,
    Dec,
}

struct CounterApp {
    count: i32,
    hold_repeat_direction: Option<RepeatDirection>,
    hold_repeat_started: Option<Instant>,
    /// Bumped when a hold ends so stray `on_next_frame` chains stop immediately.
    hold_repeat_generation: u64,
    last_frame_time: Option<Instant>,
    frame_count: u32,
    current_fps: f64,
}

impl CounterApp {
    fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            count: 0,
            hold_repeat_direction: None,
            hold_repeat_started: None,
            hold_repeat_generation: 0,
            last_frame_time: None,
            frame_count: 0,
            current_fps: 0.0,
        }
    }

    fn step(&mut self, direction: RepeatDirection) {
        match direction {
            RepeatDirection::Inc => self.count = self.count.saturating_add(1),
            RepeatDirection::Dec => self.count = self.count.saturating_sub(1),
        }
    }

    fn stop_hold_repeat(&mut self) {
        self.hold_repeat_direction = None;
        self.hold_repeat_started = None;
        self.hold_repeat_generation = self.hold_repeat_generation.wrapping_add(1);
    }

    /// Chains [`Context::on_next_frame`] so each display frame can step the counter (120 Hz on ProMotion, etc.).
    fn chain_hold_frame(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        generation: u64,
    ) {
        cx.on_next_frame(window, move |this, window, cx| {
            if this.hold_repeat_generation != generation {
                return;
            }
            if this.hold_repeat_direction.is_none() {
                return;
            }
            if let Some(started) = this.hold_repeat_started {
                if started.elapsed() >= Duration::from_millis(HOLD_REPEAT_DELAY_MS) {
                    if let Some(dir) = this.hold_repeat_direction {
                        this.step(dir);
                    }
                }
            }
            cx.notify();
            this.chain_hold_frame(window, cx, generation);
        });
    }

    fn begin_hold(&mut self, direction: RepeatDirection, window: &mut Window, cx: &mut Context<Self>) {
        self.stop_hold_repeat();
        self.hold_repeat_direction = Some(direction);
        self.hold_repeat_started = Some(Instant::now());
        let generation = self.hold_repeat_generation;
        self.step(direction);
        cx.notify();
        self.chain_hold_frame(window, cx, generation);
    }
}

impl Render for CounterApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = Instant::now();
        if let Some(last) = self.last_frame_time {
            let elapsed = now.duration_since(last).as_secs_f64();
            if elapsed > 0.0 {
                let instant_fps = 1.0 / elapsed;
                self.current_fps = self.current_fps * 0.95 + instant_fps * 0.05;
            }
        }
        self.last_frame_time = Some(now);
        self.frame_count += 1;

        let current_fps = self.current_fps;

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0x0f172a))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(16.0))
                    .px(px(12.0))
                    .py(px(4.0))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(rgb(0x38bdf8))
                            .child(format!("{:.1} FPS", current_fps)),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0x94a3b8))
                            .child(format!("Frame #{}", self.frame_count)),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(20.0))
                    .child(
                        div()
                            .text_3xl()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(rgb(0xe2e8f0))
                            .child(format!("{}", self.count)),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(12.0))
                            .child(
                                div()
                                    .id(ElementId::Name("counter-decrement".into()))
                                    .px(px(20.0))
                                    .py(px(10.0))
                                    .rounded(px(6.0))
                                    .cursor_pointer()
                                    .bg(rgb(0x1e293b))
                                    .border_1()
                                    .border_color(rgb(0x334155))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, window, cx| {
                                            this.begin_hold(RepeatDirection::Dec, window, cx);
                                        }),
                                    )
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, _cx| {
                                            this.stop_hold_repeat();
                                        }),
                                    )
                                    .on_mouse_up_out(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, _cx| {
                                            this.stop_hold_repeat();
                                        }),
                                    )
                                    .child(
                                        div()
                                            .text_3xl()
                                            .text_color(rgb(0xe2e8f0))
                                            .child("−"),
                                    ),
                            )
                            .child(
                                div()
                                    .id(ElementId::Name("counter-increment".into()))
                                    .px(px(20.0))
                                    .py(px(10.0))
                                    .rounded(px(6.0))
                                    .cursor_pointer()
                                    .bg(rgb(0x1e293b))
                                    .border_1()
                                    .border_color(rgb(0x334155))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, window, cx| {
                                            this.begin_hold(RepeatDirection::Inc, window, cx);
                                        }),
                                    )
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, _cx| {
                                            this.stop_hold_repeat();
                                        }),
                                    )
                                    .on_mouse_up_out(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, _cx| {
                                            this.stop_hold_repeat();
                                        }),
                                    )
                                    .child(
                                        div()
                                            .text_3xl()
                                            .text_color(rgb(0xe2e8f0))
                                            .child("+"),
                                    ),
                            ),
                    ),
            )
    }
}

fn main() {
    env_logger::init();
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(420.0), px(280.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some("Counter".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(|cx| CounterApp::new(cx)),
        )
        .expect("open window");
        cx.activate(true);
    });
}
