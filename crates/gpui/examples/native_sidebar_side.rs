use gpui::{
    div, native_button, native_controls::NativeSidebarSide, native_sidebar, native_toggle_group,
    prelude::*, px, rgb, size, App, Bounds, Context, Entity, NativeSegmentedStyle,
    SegmentSelectEvent, Window, WindowAppearance, WindowBounds, WindowOptions,
};

struct SidebarPanel;

impl Render for SidebarPanel {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let (bg, fg, muted, border) = match window.appearance() {
            WindowAppearance::Dark | WindowAppearance::VibrantDark => {
                (rgb(0x1f1f21), rgb(0xffffff), rgb(0xa0a0a6), rgb(0x3a3a3c))
            }
            _ => (rgb(0xf7f7f8), rgb(0x1d1d1f), rgb(0x6e6e73), rgb(0xd8d8dc)),
        };

        div()
            .size_full()
            .flex()
            .flex_col()
            .gap_2()
            .bg(bg)
            .p_3()
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(fg)
                    .child("Sidebar Surface"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(muted)
                    .child("This content is rendered in the hosted GPUI sidebar surface."),
            )
            .child(div().h(px(1.0)).w_full().bg(border))
            .child(div().text_sm().text_color(fg).child("Project"))
            .child(div().text_sm().text_color(fg).child("Agents"))
            .child(div().text_sm().text_color(fg).child("Threads"))
    }
}

struct NativeSidebarSideExample {
    collapsed: bool,
    side: NativeSidebarSide,
    sidebar_panel: Entity<SidebarPanel>,
}

impl NativeSidebarSideExample {
    fn side_label(&self) -> &'static str {
        match self.side {
            NativeSidebarSide::Leading => "left",
            NativeSidebarSide::Trailing => "right",
        }
    }
}

impl Render for NativeSidebarSideExample {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (bg, panel_bg, fg, muted, border) = match window.appearance() {
            WindowAppearance::Dark | WindowAppearance::VibrantDark => (
                rgb(0x1a1a1c),
                rgb(0x242426),
                rgb(0xffffff),
                rgb(0xa0a0a6),
                rgb(0x3a3a3c),
            ),
            _ => (
                rgb(0xf2f2f4),
                rgb(0xffffff),
                rgb(0x1d1d1f),
                rgb(0x6e6e73),
                rgb(0xd8d8dc),
            ),
        };

        let selected_side = match self.side {
            NativeSidebarSide::Leading => 0,
            NativeSidebarSide::Trailing => 1,
        };

        div()
            .size_full()
            .bg(bg)
            .text_color(fg)
            .child(
                native_sidebar("sidebar-side-demo", &[""; 0])
                    .side(self.side)
                    .sidebar_view(self.sidebar_panel.clone())
                    .sidebar_width(280.0)
                    .min_sidebar_width(200.0)
                    .max_sidebar_width(420.0)
                    .collapsed(self.collapsed)
                    .size_full(),
            )
            .child(
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .bg(panel_bg)
                    .p_4()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("Native Sidebar Side"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(muted)
                            .child("Move the hosted native sidebar between the leading and trailing window edges."),
                    )
                    .child(
                        native_toggle_group("sidebar-side", &["Left", "Right"])
                            .selected_index(selected_side)
                            .segment_style(NativeSegmentedStyle::Automatic)
                            .on_select(cx.listener(
                                |this, event: &SegmentSelectEvent, _window, cx| {
                                    this.side = if event.index == 0 {
                                        NativeSidebarSide::Leading
                                    } else {
                                        NativeSidebarSide::Trailing
                                    };
                                    cx.notify();
                                },
                            )),
                    )
                    .child(
                        native_button(
                            "toggle-collapse",
                            if self.collapsed {
                                "Expand Sidebar"
                            } else {
                                "Collapse Sidebar"
                            },
                        )
                        .on_click(cx.listener(|this, _event, _window, cx| {
                            this.collapsed = !this.collapsed;
                            cx.notify();
                        })),
                    )
                    .child(
                        div()
                            .rounded(px(10.0))
                            .border_1()
                            .border_color(border)
                            .p_4()
                            .text_sm()
                            .text_color(muted)
                            .child(format!(
                                "Expected result: the native sidebar should be attached to the {} edge of the window.",
                                self.side_label()
                            )),
                    ),
            )
    }
}

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1120.), px(760.)), cx);

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_window, cx| {
                let sidebar_panel = cx.new(|_| SidebarPanel);
                cx.new(|_| NativeSidebarSideExample {
                    collapsed: false,
                    side: NativeSidebarSide::Leading,
                    sidebar_panel,
                })
            },
        )
        .expect("failed to open window");
    });
}
