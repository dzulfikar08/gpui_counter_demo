use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::{Duration, SystemTime},
};

use async_channel::{Receiver, Sender};
use bytes::Bytes;
use chrono::{DateTime, Local};
use gpui::{
    App, AsyncApp, Bounds, Context, IntoElement, Render, WeakEntity, Window, WindowBounds,
    DropdownSelectEvent, MouseButton, WindowOptions, div, native_button, native_dropdown,
    native_text_field, prelude::*, px, size,
    uniform_list, UniformListScrollHandle,
};
use gpui_platform::application;
use gpui_tokio::Tokio;
use rumqttc::{AsyncClient, Event, EventLoop, Incoming, MqttOptions, QoS, Transport};

const MAX_MESSAGES: usize = 50_000;
const UI_TICK_MS: u64 = 33;
const ROW_HEIGHT_PX: f32 = 26.0;

type ConnectionId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MessageDirection {
    Incoming,
    Outgoing,
}

#[derive(Debug, Clone)]
struct MqttMessage {
    received_at: SystemTime,
    topic: Arc<str>,
    payload: Bytes,
    qos: QoS,
    retain: bool,
    direction: MessageDirection,
}

#[derive(Debug, Clone)]
enum UiEvent {
    Status {
        connection_id: ConnectionId,
        status: String,
    },
    Message {
        connection_id: ConnectionId,
        message: MqttMessage,
    },
}

#[derive(Debug, Clone)]
enum Command {
    Connect {
        host: String,
        port: u16,
        client_id: String,
        username: String,
        password: String,
        tls: bool,
    },
    Disconnect,
    Subscribe { topic: String, qos: QoS },
    Unsubscribe { topic: String },
    Publish {
        topic: String,
        payload: String,
        qos: QoS,
        retain: bool,
    },
    ClearMessages,
}

struct ConnectionState {
    id: ConnectionId,
    title: String,
    expanded: bool,
    selected: bool,

    // Connection inputs
    host: String,
    port: String,
    client_id: String,
    username: String,
    password: String,
    tls: bool,

    // Subscription inputs
    sub_topic: String,
    pub_topic: String,
    pub_payload: String,
    pub_qos: QoS,

    status: String,
    subscribed: Vec<String>,
    messages: VecDeque<MqttMessage>,
    auto_scroll: bool,
    scroll_handle: UniformListScrollHandle,

    cmd_tx: Sender<Command>,
}

struct MqttExplorerApp {
    connections: Vec<ConnectionState>,
    next_connection_id: ConnectionId,
    ui_tx: Sender<UiEvent>,
    _bridge_task: gpui::Task<()>,
}

impl ConnectionState {
    fn new(cx: &mut Context<MqttExplorerApp>, id: ConnectionId, ui_tx: Sender<UiEvent>) -> Self {
        let (cmd_tx, cmd_rx) = async_channel::unbounded::<Command>();
        Tokio::spawn(cx, mqtt_manager(id, cmd_rx, ui_tx)).detach();

        Self {
            id,
            title: format!("Connection {id}"),
            expanded: true,
            selected: false,
            host: "broker.emqx.io".to_string(),
            port: "1883".to_string(),
            client_id: format!("gpui-{}", rand_suffix()),
            username: "".to_string(),
            password: "".to_string(),
            tls: false,
            sub_topic: "gpui/demo/#".to_string(),
            pub_topic: "gpui/demo/hello".to_string(),
            pub_payload: r#"{"hello":"world"}"#.to_string(),
            pub_qos: QoS::AtMostOnce,
            status: "Disconnected".to_string(),
            subscribed: Vec::new(),
            messages: VecDeque::new(),
            auto_scroll: true,
            scroll_handle: UniformListScrollHandle::new(),
            cmd_tx,
        }
    }

    fn send(&self, cmd: Command) {
        let _ = self.cmd_tx.try_send(cmd);
    }

    fn connect(&mut self) {
        let port: u16 = self.port.parse().unwrap_or(1883);
        self.send(Command::Connect {
            host: self.host.clone(),
            port,
            client_id: self.client_id.clone(),
            username: self.username.clone(),
            password: self.password.clone(),
            tls: self.tls,
        });
        self.status = "Connecting…".to_string();
    }

    fn disconnect(&mut self) {
        self.send(Command::Disconnect);
        self.status = "Disconnecting…".to_string();
    }

    fn subscribe(&mut self) {
        let topic = self.sub_topic.trim().to_string();
        if topic.is_empty() {
            return;
        }
        if !self.subscribed.iter().any(|t| t == &topic) {
            self.subscribed.push(topic.clone());
        }
        self.send(Command::Subscribe {
            topic,
            qos: QoS::AtMostOnce,
        });
    }

    fn publish(&mut self) {
        let topic = self.pub_topic.trim().to_string();
        if topic.is_empty() {
            return;
        }
        let qos = self.pub_qos;
        self.push_message(MqttMessage {
            received_at: SystemTime::now(),
            topic: Arc::<str>::from(topic.clone()),
            payload: Bytes::from(self.pub_payload.clone().into_bytes()),
            qos,
            retain: false,
            direction: MessageDirection::Outgoing,
        });
        self.send(Command::Publish {
            topic,
            payload: self.pub_payload.clone(),
            qos,
            retain: false,
        });
    }

    fn clear_messages(&mut self) {
        self.messages.clear();
        self.send(Command::ClearMessages);
    }

    fn push_message(&mut self, msg: MqttMessage) {
        if self.messages.len() >= MAX_MESSAGES {
            let overflow = self.messages.len() + 1 - MAX_MESSAGES;
            for _ in 0..overflow {
                self.messages.pop_front();
            }
        }
        self.messages.push_back(msg);
    }
}

impl MqttExplorerApp {
    fn new(cx: &mut Context<Self>) -> Self {
        let (ui_tx, ui_rx) = async_channel::unbounded::<UiEvent>();

        let bridge_task = cx.spawn(async move |this, cx| {
            ui_bridge(ui_rx, this, cx).await;
        });

        let mut this = Self {
            connections: Vec::new(),
            next_connection_id: 1,
            ui_tx: ui_tx.clone(),
            _bridge_task: bridge_task,
        };
        this.add_connection(cx);
        this
    }

    fn add_connection(&mut self, cx: &mut Context<Self>) {
        let id = self.next_connection_id;
        self.next_connection_id += 1;
        let mut conn = ConnectionState::new(cx, id, self.ui_tx.clone());
        if self.connections.is_empty() {
            conn.selected = true;
        }
        self.connections.push(conn);
    }

    fn selected_connection_mut(&mut self) -> Option<&mut ConnectionState> {
        self.connections.iter_mut().find(|c| c.selected)
    }

    fn select_connection(&mut self, id: ConnectionId) {
        for c in &mut self.connections {
            c.selected = c.id == id;
        }
    }
}

impl Render for MqttExplorerApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tokens = gpui::glass::glass_tokens(window);
        let bg = tokens.rgba(tokens.bg);
        let panel = tokens.rgba(tokens.panel);
        let border = tokens.rgba(tokens.border);
        let divider = tokens.rgba(tokens.divider);
        let fg = tokens.rgba(tokens.fg);
        let fg_strong = tokens.rgba(tokens.fg_strong);
        let muted = tokens.rgba(tokens.fg_muted);
        let _accent = tokens.rgba(tokens.accent);
        let incoming_fg = tokens.rgba(tokens.fg_strong);
        let incoming_meta_fg = tokens.rgba(tokens.fg_muted);
        // Outgoing rows: right-aligned and accent-colored, without bubble padding.
        let outgoing_border = tokens.rgba(tokens.accent.alpha(0.35));
        let outgoing_fg = tokens.rgba(tokens.accent);
        let outgoing_meta_fg = tokens.rgba(tokens.fg_muted);

        let selected = self.connections.iter().find(|c| c.selected);
        let status = selected
            .map(|c| c.status.clone())
            .unwrap_or_else(|| "No connection selected".to_string());
        let selected_id = selected.map(|c| c.id);
        let messages_len = selected.map(|c| c.messages.len()).unwrap_or(0);

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(bg)
            .text_color(fg)
            .child(
                div()
                    .px(px(12.0))
                    .py(px(10.0))
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(divider)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.0))
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(fg_strong)
                                    .child("MQTT Explorer (GPUI)"),
                            )
                            .child(div().text_sm().text_color(muted).child(status)),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                native_button("new-connection", "New Connection").on_click(
                                    cx.listener(|this, _ev, _window, cx| {
                                        this.add_connection(cx);
                                        cx.notify();
                                    }),
                                ),
                            )
                            .child(
                                native_button("connect", "Connect").on_click(cx.listener(
                                    |this, _ev, _window, cx| {
                                        if let Some(conn) = this.selected_connection_mut() {
                                            conn.connect();
                                        }
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(
                                native_button("disconnect", "Disconnect").on_click(cx.listener(
                                    |this, _ev, _window, cx| {
                                        if let Some(conn) = this.selected_connection_mut() {
                                            conn.disconnect();
                                        }
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(
                                native_button("clear", "Clear").on_click(cx.listener(
                                    |this, _ev, _window, cx| {
                                        if let Some(conn) = this.selected_connection_mut() {
                                            conn.clear_messages();
                                        }
                                        cx.notify();
                                    },
                                )),
                            ),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .gap(px(12.0))
                    .p(px(12.0))
                    .child(
                        div()
                            .w(px(380.0))
                            .min_w(px(260.0))
                            .max_w(px(520.0))
                            .flex_shrink()
                            .rounded_lg()
                            .border_1()
                            .border_color(border)
                            .bg(panel)
                            .p(px(12.0))
                            .flex()
                            .flex_col()
                            .gap(px(12.0))
                            .child(div().text_sm().text_color(muted).child("Connections"))
                            .children(self.connections.iter().map(|conn| {
                                let id = conn.id;
                                let expanded = conn.expanded;
                                let selected = conn.selected;
                                let title = conn.title.clone();
                                let status = conn.status.clone();

                                div()
                                    .w_full()
                                    .rounded(px(10.0))
                                    .border_1()
                                    .border_color(if selected { outgoing_border } else { border })
                                    .bg(bg)
                                    .child(
                                        div()
                                            .px(px(10.0))
                                            .py(px(8.0))
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .cursor_pointer()
                                            .on_mouse_up(MouseButton::Left, cx.listener(move |this, _ev, _w, cx| {
                                                let mut toggled = false;
                                                for c in &mut this.connections {
                                                    if c.id == id {
                                                        if c.selected {
                                                            c.expanded = !c.expanded;
                                                            toggled = true;
                                                        }
                                                    }
                                                }
                                                if !toggled {
                                                    this.select_connection(id);
                                                }
                                                cx.notify();
                                            }))
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .gap(px(2.0))
                                                    .child(div().text_sm().text_color(fg_strong).child(title))
                                                    .child(div().text_xs().text_color(muted).child(status)),
                                            )
                                            .child(div().text_sm().text_color(muted).child(if expanded { "▾" } else { "▸" })),
                                    )
                                    .when(expanded, |s| {
                                        s.child(
                                            div()
                                                .px(px(10.0))
                                                .pb(px(10.0))
                                                .pt(px(2.0))
                                                .flex()
                                                .flex_col()
                                                .gap(px(10.0))
                                                .child(div().h(px(1.0)).w_full().bg(divider))
                                                .child(
                                                    div()
                                                        .flex()
                                                        .gap(px(8.0))
                                                        .child(
                                                            native_text_field(format!("host-{id}"))
                                                                .placeholder("host")
                                                                .value(conn.host.clone())
                                                                .on_change(cx.listener(move |this, ev: &gpui::TextChangeEvent, _w, cx| {
                                                                    if let Some(c) = this.connections.iter_mut().find(|c| c.id == id) {
                                                                        c.host = ev.text.clone();
                                                                    }
                                                                    cx.notify();
                                                                }))
                                                                .flex_1(),
                                                        )
                                                        .child(
                                                            native_text_field(format!("port-{id}"))
                                                                .placeholder("1883")
                                                                .value(conn.port.clone())
                                                                .on_change(cx.listener(move |this, ev: &gpui::TextChangeEvent, _w, cx| {
                                                                    if let Some(c) = this.connections.iter_mut().find(|c| c.id == id) {
                                                                        c.port = ev.text.clone();
                                                                    }
                                                                    cx.notify();
                                                                }))
                                                                .w(px(90.0)),
                                                        ),
                                                )
                                                .child(
                                                    native_text_field(format!("sub-{id}"))
                                                        .placeholder("subscribe topic (e.g. foo/#)")
                                                        .value(conn.sub_topic.clone())
                                                        .on_change(cx.listener(move |this, ev: &gpui::TextChangeEvent, _w, cx| {
                                                            if let Some(c) = this.connections.iter_mut().find(|c| c.id == id) {
                                                                c.sub_topic = ev.text.clone();
                                                            }
                                                            cx.notify();
                                                        })),
                                                )
                                                .child(
                                                    native_button(format!("subscribe-{id}"), "Subscribe")
                                                        .on_click(cx.listener(move |this, _ev, _w, cx| {
                                                            if let Some(c) = this.connections.iter_mut().find(|c| c.id == id) {
                                                                c.subscribe();
                                                            }
                                                            cx.notify();
                                                        })),
                                                ),
                                        )
                                    })
                            })),
                    )
                    .child(
                        div()
                            .flex_1()
                            .rounded_lg()
                            .border_1()
                            .border_color(border)
                            .bg(panel)
                            .p(px(12.0))
                            .flex()
                            .flex_col()
                            .gap(px(10.0))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(muted)
                                            .child("Messages"),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(10.0))
                                            .child(
                                                native_button("toggle-autoscroll", "Auto-scroll")
                                                    .on_click(cx.listener(|this, _ev, _w, cx| {
                                                        if let Some(conn) = this.selected_connection_mut() {
                                                            conn.auto_scroll = !conn.auto_scroll;
                                                            if conn.auto_scroll {
                                                                conn.scroll_handle.scroll_to_bottom();
                                                            }
                                                        }
                                                        cx.notify();
                                                    })),
                                            )
                                            .child(
                                                div().text_xs().text_color(muted).child(format!(
                                                    "{} (cap {})",
                                                    messages_len, MAX_MESSAGES
                                                )),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .rounded(px(8.0))
                                    .border_1()
                                    .border_color(border)
                                    .bg(bg)
                                    .overflow_hidden()
                                    .child({
                                        let handle = self
                                            .connections
                                            .iter()
                                            .find(|c| c.selected)
                                            .map(|c| c.scroll_handle.clone());
                                        let list = uniform_list(
                                            "messages",
                                            messages_len,
                                            cx.processor(
                                                move |this, range: std::ops::Range<usize>, _window, _cx| {
                                                    let mut items = Vec::with_capacity(range.len());
                                                    for i in range {
                                                        let Some(conn_id) = selected_id else {
                                                            continue;
                                                        };
                                                        let Some(conn) = this
                                                            .connections
                                                            .iter()
                                                            .find(|c| c.id == conn_id)
                                                        else {
                                                            continue;
                                                        };
                                                        let Some(msg) = conn.messages.get(i) else {
                                                            continue;
                                                        };

                                                    let preview = payload_preview(&msg.payload);
                                                    let outgoing = msg.direction == MessageDirection::Outgoing;
                                                    let topic = msg.topic.as_ref().to_string();
                                                    let ts = format_timestamp(msg.received_at);

                                                        items.push(
                                                        div()
                                                            .id(i)
                                                            .h(px(ROW_HEIGHT_PX))
                                                            .w_full()
                                                            .px(px(10.0))
                                                            .flex()
                                                            .items_center()
                                                            .gap(px(10.0))
                                                            .border_b_1()
                                                            .border_color(divider)
                                                            .when(outgoing, |s| s.justify_end())
                                                            .when(!outgoing, |s| s.justify_start())
                                                            .child(
                                                                div()
                                                                    .flex()
                                                                    .items_center()
                                                                    .gap(px(10.0))
                                                                    .when(outgoing, |s| s.text_right())
                                                                    .child(
                                                                        div()
                                                                            .text_xs()
                                                                            .text_color(muted)
                                                                            .child(ts),
                                                                    )
                                                                    .child(
                                                                        div()
                                                                            .text_xs()
                                                                            .text_color(if outgoing { outgoing_meta_fg } else { incoming_meta_fg })
                                                                            .child(topic),
                                                                    )
                                                                    .child(
                                                                        div()
                                                                            .text_sm()
                                                                            .text_color(if outgoing { outgoing_fg } else { incoming_fg })
                                                                            .child(preview),
                                                                    ),
                                                            ),
                                                        );
                                                }
                                                items
                                            },
                                            ),
                                        );
                                        let list = if let Some(handle) = handle.as_ref() {
                                            list.track_scroll(handle)
                                        } else {
                                            list
                                        };
                                        list.h_full().w_full()
                                    }),
                            )
                            .child(
                                div()
                                    .mt(px(10.0))
                                    .pt(px(10.0))
                                    .border_t_1()
                                    .border_color(border)
                                    .flex()
                                    .gap(px(8.0))
                                    .items_center()
                                    .child(
                                        native_text_field("chat_topic")
                                            .placeholder("topic")
                                            .value(
                                                self.connections
                                                    .iter()
                                                    .find(|c| c.selected)
                                                    .map(|c| c.pub_topic.clone())
                                                    .unwrap_or_default(),
                                            )
                                            .on_change(cx.listener(|this, ev: &gpui::TextChangeEvent, _w, cx| {
                                                if let Some(conn) = this.selected_connection_mut() {
                                                    conn.pub_topic = ev.text.clone();
                                                }
                                                cx.notify();
                                            }))
                                            .w(px(220.0))
                                            .flex_shrink_0(),
                                    )
                                    .child(
                                        native_dropdown("chat_qos", &["QoS 0", "QoS 1", "QoS 2"])
                                            .selected_index(
                                                self.connections
                                                    .iter()
                                                    .find(|c| c.selected)
                                                    .map(|c| match c.pub_qos {
                                                        QoS::AtMostOnce => 0,
                                                        QoS::AtLeastOnce => 1,
                                                        QoS::ExactlyOnce => 2,
                                                    })
                                                    .unwrap_or(0),
                                            )
                                            .on_select(cx.listener(|this, ev: &DropdownSelectEvent, _w, cx| {
                                                if let Some(conn) = this.selected_connection_mut() {
                                                    conn.pub_qos = match ev.index {
                                                        1 => QoS::AtLeastOnce,
                                                        2 => QoS::ExactlyOnce,
                                                        _ => QoS::AtMostOnce,
                                                    };
                                                }
                                                cx.notify();
                                            }))
                                            .w(px(110.0))
                                            .flex_shrink_0(),
                                    )
                                    .child(
                                        native_text_field("chat_input")
                                            .placeholder("Type a message…")
                                            .value(
                                                self.connections
                                                    .iter()
                                                    .find(|c| c.selected)
                                                    .map(|c| c.pub_payload.clone())
                                                    .unwrap_or_default(),
                                            )
                                            .on_change(cx.listener(|this, ev: &gpui::TextChangeEvent, _w, cx| {
                                                if let Some(conn) = this.selected_connection_mut() {
                                                    conn.pub_payload = ev.text.clone();
                                                }
                                                cx.notify();
                                            }))
                                            .on_submit(cx.listener(|this, _ev: &gpui::TextSubmitEvent, _w, cx| {
                                                if let Some(conn) = this.selected_connection_mut() {
                                                    conn.publish();
                                                }
                                                cx.notify();
                                            }))
                                            .flex_1(),
                                    )
                                    .child(
                                        native_button("chat_send", "Send").on_click(cx.listener(|this, _ev, _w, cx| {
                                            if let Some(conn) = this.selected_connection_mut() {
                                                conn.publish();
                                            }
                                            cx.notify();
                                        })),
                                    ),
                            ),
                    ),
            )
    }
}

fn labeled_field(label: &'static str, field: impl IntoElement, muted: gpui::Rgba) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(4.0))
        .child(div().text_xs().text_color(muted).child(label))
        .child(field)
}

fn payload_preview(payload: &Bytes) -> String {
    const MAX: usize = 240;
    if payload.is_empty() {
        return "∅".to_string();
    }
    match std::str::from_utf8(payload) {
        Ok(s) => {
            let mut s = s.replace('\n', "⏎");
            if s.len() > MAX {
                s.truncate(MAX);
                s.push('…');
            }
            s
        }
        Err(_) => format!("<{} bytes>", payload.len()),
    }
}

fn format_timestamp(ts: SystemTime) -> String {
    let dt: DateTime<Local> = ts.into();
    dt.format("%H:%M:%S%.3f").to_string()
}

fn rand_suffix() -> String {
    // No external rand dep; good enough for demo uniqueness.
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{:x}", (nanos as u64).wrapping_mul(0x9e3779b97f4a7c15))
}

async fn ui_bridge(rx: Receiver<UiEvent>, this: WeakEntity<MqttExplorerApp>, cx: &mut AsyncApp) {
    let mut pending_status: HashMap<ConnectionId, String> = HashMap::new();
    let mut pending_messages: HashMap<ConnectionId, Vec<MqttMessage>> = HashMap::new();

    loop {
        // Wait for at least one event, then drain immediately and flush to UI.
        let first = match rx.recv().await {
            Ok(ev) => ev,
            Err(_) => break,
        };
        match first {
            UiEvent::Status {
                connection_id,
                status,
            } => {
                pending_status.insert(connection_id, status);
            }
            UiEvent::Message {
                connection_id,
                message,
            } => {
                pending_messages
                    .entry(connection_id)
                    .or_default()
                    .push(message);
            }
        }
        while let Ok(ev) = rx.try_recv() {
            match ev {
                UiEvent::Status {
                    connection_id,
                    status,
                } => {
                    pending_status.insert(connection_id, status);
                }
                UiEvent::Message {
                    connection_id,
                    message,
                } => {
                    pending_messages
                        .entry(connection_id)
                        .or_default()
                        .push(message);
                }
            }
        }

        let mut statuses = HashMap::new();
        std::mem::swap(&mut statuses, &mut pending_status);
        let mut messages = HashMap::new();
        std::mem::swap(&mut messages, &mut pending_messages);
        let _ = this.update(cx, move |app, cx| {
            for (id, s) in statuses {
                if let Some(conn) = app.connections.iter_mut().find(|c| c.id == id) {
                    conn.status = s;
                }
            }
            for (id, msgs) in messages {
                if let Some(conn) = app.connections.iter_mut().find(|c| c.id == id) {
                    for m in msgs {
                        conn.push_message(m);
                    }
                    if conn.auto_scroll {
                        conn.scroll_handle.scroll_to_bottom();
                    }
                }
            }
            cx.notify();
        });

        // Throttle repaint frequency under heavy load.
        cx.background_executor()
            .timer(Duration::from_millis(UI_TICK_MS))
            .await;
    }
}

async fn mqtt_manager(
    connection_id: ConnectionId,
    cmd_rx: Receiver<Command>,
    ui_tx: Sender<UiEvent>,
) -> anyhow::Result<()> {
    let mut connection: Option<MqttConnection> = None;

    while let Ok(cmd) = cmd_rx.recv().await {
        match cmd {
            Command::Connect {
                host,
                port,
                client_id,
                username,
                password,
                tls,
            } => {
                if let Some(mut c) = connection.take() {
                    c.stop().await;
                }

                let _ = ui_tx
                    .send(UiEvent::Status {
                        connection_id,
                        status: "Connecting…".to_string(),
                    })
                    .await;

                let mut opts = MqttOptions::new(client_id, host, port);
                opts.set_keep_alive(Duration::from_secs(30));
                if !username.is_empty() {
                    opts.set_credentials(username, password);
                }
                if tls {
                    opts.set_transport(Transport::tls_with_default_config());
                }

                let (client, eventloop) = AsyncClient::new(opts, 1000);
                let conn = MqttConnection::start(connection_id, client, eventloop, ui_tx.clone());
                let _ = ui_tx
                    .send(UiEvent::Status {
                        connection_id,
                        status: "Connected".to_string(),
                    })
                    .await;
                connection = Some(conn);
            }
            Command::Disconnect => {
                if let Some(mut c) = connection.take() {
                    let _ = ui_tx
                        .send(UiEvent::Status {
                            connection_id,
                            status: "Disconnected".to_string(),
                        })
                        .await;
                    c.stop().await;
                } else {
                    let _ = ui_tx
                        .send(UiEvent::Status {
                            connection_id,
                            status: "Disconnected".to_string(),
                        })
                        .await;
                }
            }
            Command::Subscribe { topic, qos } => {
                if let Some(c) = connection.as_mut() {
                    c.subscribe(topic, qos).await;
                }
            }
            Command::Unsubscribe { topic } => {
                if let Some(c) = connection.as_mut() {
                    c.unsubscribe(topic).await;
                }
            }
            Command::Publish {
                topic,
                payload,
                qos,
                retain,
            } => {
                if let Some(c) = connection.as_mut() {
                    c.publish(topic, payload.into_bytes(), qos, retain).await;
                }
            }
            Command::ClearMessages => {}
        }
    }

    if let Some(mut c) = connection.take() {
        c.stop().await;
    }

    Ok(())
}

struct MqttConnection {
    connection_id: ConnectionId,
    client: AsyncClient,
    poll_task: Option<tokio::task::JoinHandle<()>>,
    ui_tx: Sender<UiEvent>,
}

impl MqttConnection {
    fn start(
        connection_id: ConnectionId,
        client: AsyncClient,
        mut eventloop: EventLoop,
        ui_tx: Sender<UiEvent>,
    ) -> Self {
        let poll_client = client.clone();
        let poll_ui = ui_tx.clone();
        let poll_ui_task = ui_tx.clone();
        let poll_task = tokio::spawn(async move {
            loop {
                match eventloop.poll().await {
                    Ok(Event::Incoming(Incoming::Publish(p))) => {
                        let msg = MqttMessage {
                            received_at: SystemTime::now(),
                            topic: Arc::<str>::from(p.topic),
                            payload: Bytes::from(p.payload),
                            qos: p.qos,
                            retain: p.retain,
                            direction: MessageDirection::Incoming,
                        };
                        let _ = poll_ui_task
                            .send(UiEvent::Message {
                                connection_id,
                                message: msg,
                            })
                            .await;
                    }
                    Ok(_) => {}
                    Err(e) => {
                        let _ = poll_ui_task
                            .send(UiEvent::Status {
                                connection_id,
                                status: format!("Error: {e}"),
                            })
                            .await;
                        // Avoid a tight error loop if connection is down.
                        tokio::time::sleep(Duration::from_millis(250)).await;
                        // Best effort: tell broker we are done if possible.
                        let _ = poll_client.disconnect().await;
                        break;
                    }
                }
            }
        });

        // Mark as live when poll task starts.
        let _ = poll_ui.try_send(UiEvent::Status {
            connection_id,
            status: "Live".to_string(),
        });

        Self {
            connection_id,
            client,
            poll_task: Some(poll_task),
            ui_tx,
        }
    }

    async fn stop(&mut self) {
        if let Some(task) = self.poll_task.take() {
            task.abort();
            let _ = task.await;
        }
        let _ = self.client.disconnect().await;
    }

    async fn subscribe(&mut self, topic: String, qos: QoS) {
        match self.client.subscribe(topic.clone(), qos).await {
            Ok(_) => {
                let _ = self
                    .ui_tx
                    .try_send(UiEvent::Status {
                        connection_id: self.connection_id,
                        status: format!("Subscribed: {topic}"),
                    });
            }
            Err(e) => {
                let _ = self
                    .ui_tx
                    .try_send(UiEvent::Status {
                        connection_id: self.connection_id,
                        status: format!("Subscribe failed: {e}"),
                    });
            }
        }
    }

    async fn unsubscribe(&mut self, topic: String) {
        match self.client.unsubscribe(topic.clone()).await {
            Ok(_) => {
                let _ = self
                    .ui_tx
                    .try_send(UiEvent::Status {
                        connection_id: self.connection_id,
                        status: format!("Unsubscribed: {topic}"),
                    });
            }
            Err(e) => {
                let _ = self
                    .ui_tx
                    .try_send(UiEvent::Status {
                        connection_id: self.connection_id,
                        status: format!("Unsubscribe failed: {e}"),
                    });
            }
        }
    }

    async fn publish(&mut self, topic: String, payload: Vec<u8>, qos: QoS, retain: bool) {
        match self
            .client
            .publish(topic.clone(), qos, retain, payload)
            .await
        {
            Ok(_) => {
                let _ = self.ui_tx.try_send(UiEvent::Status {
                    connection_id: self.connection_id,
                    status: format!("Published: {topic}"),
                });
            }
            Err(e) => {
                let _ = self
                    .ui_tx
                    .try_send(UiEvent::Status {
                        connection_id: self.connection_id,
                        status: format!("Publish failed: {e}"),
                    });
            }
        }
    }
}

fn main() {
    env_logger::init();
    application().run(|cx: &mut App| {
        gpui_tokio::init(cx);
        let bounds = Bounds::centered(None, size(px(1240.0), px(720.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some("MQTT Explorer (GPUI)".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(|cx| MqttExplorerApp::new(cx)),
        )
        .expect("open window");
        cx.activate(true);
    })
}

