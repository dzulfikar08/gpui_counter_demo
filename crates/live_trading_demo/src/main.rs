use std::{collections::VecDeque, sync::Arc, time::Duration};

use anyhow::Context as _;
use async_channel::{Receiver, Sender};
use futures::{FutureExt as _, StreamExt as _};
use gpui::{
    fill, App, AsyncApp, Bounds, Context, DropdownSelectEvent, ElementId, Half, IntoElement,
    PathBuilder, Pixels, Point, Render, WeakEntity, Window, WindowBounds, WindowOptions, canvas,
    div, native_dropdown, point, prelude::*, px, size,
};
use gpui_platform::application;
use gpui_tokio::Tokio;
use serde_json::Value;
use tokio_tungstenite::{
    connect_async_tls_with_config,
    tungstenite::protocol::WebSocketConfig,
    Connector,
};
use url::Url;

const BINANCE_STREAM: &str = "btcusdt@trade";
const BINANCE_LABEL: &str = "BTCUSDT";
const MOCK_MODE: bool = true;
const TIMEFRAMES: [i64; 4] = [15, 30, 60, 300];
const TIMEFRAME_LABELS: [&str; 4] = ["15s", "30s", "1m", "5m"];
const MAX_TRADES: usize = 50;
const MAX_POINTS: usize = 300;
const INSECURE_TLS: bool = true;
const MAX_CANDLES: usize = 80;

#[derive(Debug, Clone, Copy)]
struct Candle {
    start_s: i64,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
}

#[derive(Debug)]
struct AcceptAllCerts;

impl rustls::client::danger::ServerCertVerifier for AcceptAllCerts {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::ED25519,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
            rustls::SignatureScheme::RSA_PSS_SHA256,
            rustls::SignatureScheme::RSA_PSS_SHA384,
            rustls::SignatureScheme::RSA_PSS_SHA512,
            rustls::SignatureScheme::RSA_PKCS1_SHA256,
            rustls::SignatureScheme::RSA_PKCS1_SHA384,
            rustls::SignatureScheme::RSA_PKCS1_SHA512,
        ]
    }
}

fn tls_connector() -> Option<Connector> {
    if !INSECURE_TLS {
        return None;
    }

    let config = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(AcceptAllCerts))
        .with_no_client_auth();

    Some(Connector::Rustls(Arc::new(config)))
}

#[derive(Debug, Clone)]
enum FeedMsg {
    Status(String),
    Trade(Trade),
}

#[derive(Debug, Clone)]
struct Trade {
    price: f64,
    qty: f64,
    is_sell: bool,
    time_s: f64,
}

struct TradingApp {
    status: String,
    last_price: Option<f64>,
    last_qty: Option<f64>,
    trades: VecDeque<Trade>,
    prices: VecDeque<f64>,
    candles: VecDeque<Candle>,
    current_candle: Option<Candle>,
    timeframe_index: usize,
    _keepalive_tx: Sender<FeedMsg>,
    _rx_task: gpui::Task<()>,
}

impl TradingApp {
    fn new(cx: &mut Context<Self>) -> Self {
        let (tx, rx) = async_channel::unbounded::<FeedMsg>();

        if MOCK_MODE {
            let tx_for_feed = tx.clone();
            Tokio::spawn(cx, async move {
                run_mock_feed(tx_for_feed).await;
            })
            .detach();
        } else {
            // Supervise the WS feed on Tokio so it never exits silently.
            let tx_for_feed = tx.clone();
            Tokio::spawn(cx, async move {
                loop {
                    if let Err(e) = stream_trades(tx_for_feed.clone()).await {
                        let _ = tx_for_feed
                            .send(FeedMsg::Status(format!("Feed error: {e:#}. Retrying…")))
                            .await;
                    }
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            })
            .detach();
        }

        // Bridge loop on GPUI executor (keeps UI updates on UI thread).
        let rx_task = cx.spawn(async move |this, cx| {
            bridge_trades(rx, this, cx).await;
        });

        Self {
            status: if MOCK_MODE { "Mock".to_string() } else { "Connecting…".to_string() },
            last_price: None,
            last_qty: None,
            trades: VecDeque::with_capacity(MAX_TRADES),
            prices: VecDeque::with_capacity(MAX_POINTS),
            candles: VecDeque::with_capacity(MAX_CANDLES),
            current_candle: None,
            timeframe_index: 1,
            _keepalive_tx: tx,
            _rx_task: rx_task,
        }
    }

    fn push_trade(&mut self, trade: Trade) {
        self.status = "Live".to_string();
        self.last_price = Some(trade.price);
        self.last_qty = Some(trade.qty);

        if self.trades.len() == MAX_TRADES {
            self.trades.pop_back();
        }
        self.trades.push_front(trade.clone());

        if self.prices.len() == MAX_POINTS {
            self.prices.pop_front();
        }
        self.prices.push_back(trade.price);

        self.push_candle_sample(trade.time_s, trade.price);
    }

    fn set_status(&mut self, status: String) {
        self.status = status;
    }

    fn push_candle_sample(&mut self, time_s: f64, price: f64) {
        let tf = TIMEFRAMES[self.timeframe_index.min(TIMEFRAMES.len() - 1)];
        let start_s = (time_s.floor() as i64).div_euclid(tf) * tf;
        match self.current_candle {
            None => {
                self.current_candle = Some(Candle {
                    start_s,
                    open: price,
                    high: price,
                    low: price,
                    close: price,
                });
            }
            Some(mut c) if c.start_s == start_s => {
                c.high = c.high.max(price);
                c.low = c.low.min(price);
                c.close = price;
                self.current_candle = Some(c);
            }
            Some(c) => {
                if self.candles.len() == MAX_CANDLES {
                    self.candles.pop_front();
                }
                self.candles.push_back(c);
                self.current_candle = Some(Candle {
                    start_s,
                    open: price,
                    high: price,
                    low: price,
                    close: price,
                });
            }
        }
    }

    fn set_timeframe(&mut self, new_index: usize) {
        if self.timeframe_index == new_index {
            return;
        }
        self.timeframe_index = new_index.min(TIMEFRAMES.len() - 1);
        self.candles.clear();
        self.current_candle = None;
    }
}

impl Render for TradingApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let last_price = self
            .last_price
            .map(|p| format!("{p:.2}"))
            .unwrap_or_else(|| "—".to_string());
        let last_qty = self
            .last_qty
            .map(|q| format!("{q:.6}"))
            .unwrap_or_else(|| "—".to_string());

        let _prices: Vec<f64> = self.prices.iter().copied().collect();
        let mut candles: Vec<Candle> = self.candles.iter().copied().collect();
        if let Some(c) = self.current_candle {
            candles.push(c);
        }

        // Tokens inspired by `/Users/macbookpro/Documents/CobaCoba/Glass/docs/theme/css/variables.css`
        let tokens = gpui::glass::glass_tokens(_window);

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(tokens.rgba(tokens.bg))
            .child(
                div()
                    .px(px(12.0))
                    .py(px(10.0))
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(tokens.rgba(tokens.divider))
                    .child(
                        div()
                            .flex()
                            .gap(px(10.0))
                            .items_center()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(tokens.rgba(tokens.fg_strong))
                                    .child("BTCUSDT (Binance)"),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(tokens.rgba(tokens.fg_muted))
                                    .child(self.status.clone()),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(18.0))
                            .items_center()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.0))
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(tokens.rgba(tokens.fg_muted))
                                            .child("TF"),
                                    )
                                    .child(
                                        native_dropdown("timeframe", &TIMEFRAME_LABELS)
                                            .selected_index(self.timeframe_index)
                                            .on_select(cx.listener(
                                                |this: &mut TradingApp,
                                                 event: &DropdownSelectEvent,
                                                 _window,
                                                 cx| {
                                                    this.set_timeframe(event.index);
                                                    cx.notify();
                                                },
                                            ))
                                            .w(px(84.0)),
                                    ),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(tokens.rgba(tokens.fg_muted))
                                    .child(format!("Last: {last_price}")),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(tokens.rgba(tokens.fg_muted))
                                    .child(format!("Qty: {last_qty}")),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .gap(px(12.0))
                    .p(px(12.0))
                    .child(
                        div()
                            .flex_1()
                            .rounded_lg()
                            .border_1()
                            .border_color(tokens.rgba(tokens.border))
                            .bg(tokens.rgba(tokens.panel))
                            .p(px(12.0))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(tokens.rgba(tokens.fg_muted))
                                    .child(format!(
                                        "Candles ({})",
                                        TIMEFRAME_LABELS[self.timeframe_index]
                                    )),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(tokens.rgba(tokens.fg_muted.opacity(0.9)))
                                            .child("mock stream"),
                                    ),
                            )
                            .child(div().h(px(140.0)).w_full().mt(px(10.0)).child(
                                div()
                                    .id(ElementId::Name("candles".into()))
                                    .size_full()
                                    .child(canvas(
                                        move |_bounds, _window, _cx| {},
                                        move |bounds, _prepaint, window, _cx| {
                                            paint_candles(bounds, window, &candles);
                                        },
                                    )),
                            )),
                    )
                    .child(
                        div()
                            .w(px(340.0))
                            .rounded_lg()
                            .border_1()
                            .border_color(tokens.rgba(tokens.border))
                            .bg(tokens.rgba(tokens.panel))
                            .p(px(12.0))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(tokens.rgba(tokens.fg_muted))
                                    .child("Trades (latest first)"),
                            )
                            .child(
                                div()
                                    .mt(px(10.0))
                                    .flex()
                                    .flex_col()
                                    .gap(px(6.0))
                                    .children(self.trades.iter().take(18).map(|trade| {
                                        // Keep green/red semantics, but soften them to match the calmer palette.
                                        let side_color = if trade.is_sell {
                                            tokens.rgba(gpui::hsla(0.0, 0.75, 0.62, 1.0))
                                        } else {
                                            tokens.rgba(gpui::hsla(130.0 / 360.0, 0.55, 0.45, 1.0))
                                        };
                                        div()
                                            .flex()
                                            .justify_between()
                                            .text_sm()
                                            .child(
                                                div()
                                                    .text_color(side_color)
                                                    .child(format!("{:>10.2}", trade.price)),
                                            )
                                            .child(
                                                div()
                                                    .text_color(tokens.rgba(tokens.fg_muted))
                                                    .child(format!("{:>10.6}", trade.qty)),
                                            )
                                    })),
                            ),
                    ),
            )
            .child(
                div()
                    .px(px(12.0))
                    .py(px(8.0))
                    .border_t_1()
                    .border_color(tokens.rgba(tokens.divider))
                    .text_xs()
                    .text_color(tokens.rgba(tokens.fg_muted.opacity(0.9)))
                    .child("Data: Binance public WebSocket (trade). This is a demo UI, not a broker."),
            )
    }
}

async fn stream_trades(tx: Sender<FeedMsg>) -> anyhow::Result<()> {
    let url = format!("wss://stream.binance.com:9443/ws/{BINANCE_STREAM}");
    let _ = Url::parse(&url).context("parse websocket url")?;

    let _ = tx
        .send(FeedMsg::Status(format!("Connecting… ({url})")))
        .await;

    let mut backoff_ms = 250u64;
    loop {
        let connector = tls_connector();
        let connect = tokio::time::timeout(
            Duration::from_secs(8),
            connect_async_tls_with_config(&url, Some(WebSocketConfig::default()), false, connector),
        )
        .await;
        match connect {
            Err(_) => {
                let _ = tx
                    .send(FeedMsg::Status("Connect timed out. Retrying…".into()))
                    .await;
            }
            Ok(Err(e)) => {
                let _ = tx
                    .send(FeedMsg::Status(format!("Connect failed: {e}. Retrying…")))
                    .await;
            }
            Ok(Ok((ws, _resp))) => {
                backoff_ms = 250;
                let _ = tx.send(FeedMsg::Status("Live".into())).await;
                let (_write, mut read) = ws.split();

                while let Some(msg) = read.next().await {
                    let msg = match msg {
                        Ok(m) => m,
                        Err(e) => {
                            let _ = tx
                                .send(FeedMsg::Status(format!("WS error: {e}")))
                                .await;
                            break;
                        }
                    };
                    if !msg.is_text() {
                        continue;
                    }
                    let text = msg.into_text().unwrap_or_default();
                    let v: Value = match serde_json::from_str(&text) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };

                    // Binance trade payload contains:
                    // p (price), q (qty), m (is buyer maker), T (trade time)
                    if v.get("e").and_then(|x| x.as_str()) != Some("trade") {
                        continue;
                    }
                    if v.get("s").and_then(|x| x.as_str()) != Some(BINANCE_LABEL) {
                        continue;
                    }

                    let price: f64 = v
                        .get("p")
                        .and_then(|x| x.as_str())
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0.0);
                    let qty: f64 = v
                        .get("q")
                        .and_then(|x| x.as_str())
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0.0);
                    let is_buyer_maker = v.get("m").and_then(|x| x.as_bool()).unwrap_or(false);
                    let trade_time_ms: u64 = v.get("T").and_then(|x| x.as_u64()).unwrap_or(0);

                    let trade = Trade {
                        price,
                        qty,
                        // If buyer is maker, taker side is sell.
                        is_sell: is_buyer_maker,
                        time_s: trade_time_ms as f64 / 1000.0,
                    };
                    if tx.send(FeedMsg::Trade(trade)).await.is_err() {
                        return Ok(());
                    }
                }
            }
        }

        tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
        backoff_ms = (backoff_ms * 2).min(10_000);
    }
}

async fn bridge_trades(
    rx: Receiver<FeedMsg>,
    this: WeakEntity<TradingApp>,
    cx: &mut AsyncApp,
) {
    let mut pending: Option<Trade> = None;
    let mut pending_status: Option<String> = None;

    loop {
        let mut tick = futures::FutureExt::fuse(smol::Timer::after(Duration::from_millis(33)));
        loop {
            futures::select! {
                t = rx.recv().fuse() => {
                    match t {
                        Ok(FeedMsg::Trade(trade)) => pending = Some(trade),
                        Ok(FeedMsg::Status(s)) => pending_status = Some(s),
                        Err(_) => {
                            let _ = this.update(cx, |app, cx| {
                                app.status = "Disconnected".to_string();
                                cx.notify();
                            });
                            return;
                        }
                    }
                }
                _ = tick => break,
            }
        }

        if let Some(s) = pending_status.take() {
            let _ = this.update(cx, |app, cx| {
                app.set_status(s);
                cx.notify();
            });
        }

        if let Some(trade) = pending.take() {
            let _ = this.update(cx, |app, cx| {
                app.push_trade(trade);
                cx.notify();
            });
        }
    }
}

fn paint_sparkline(bounds: Bounds<Pixels>, window: &mut Window, prices: &[f64]) {
    if prices.len() < 2 {
        return;
    }

    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    for &p in prices {
        min = min.min(p);
        max = max.max(p);
    }

    let range = (max - min).max(1e-9);
    let w = bounds.size.width;
    let h = bounds.size.height;

    let n = prices.len();
    let dx = w / (n as f32 - 1.0);

    let mut builder = PathBuilder::stroke(px(2.0));
    for (i, &p) in prices.iter().enumerate() {
        let x = bounds.origin.x + dx * i as f32;
        let yn = ((p - min) / range) as f32;
        let y = bounds.origin.y + h - (h * yn);
        let pt: Point<Pixels> = point(x, y);
        if i == 0 {
            builder.move_to(pt);
        } else {
            builder.line_to(pt);
        }
    }

    if let Ok(path) = builder.build() {
        // Use the accent color from the Glass tokens.
        let t = gpui::glass::glass_tokens(window);
        window.paint_path(path, t.rgba(t.accent));
    }
}

fn paint_candles(bounds: Bounds<Pixels>, window: &mut Window, candles: &[Candle]) {
    if candles.is_empty() {
        return;
    }

    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    // Auto-zoom to recent candles to keep motion visible.
    let slice = if candles.len() > 24 {
        &candles[candles.len() - 24..]
    } else {
        candles
    };
    for c in slice {
        min = min.min(c.low);
        max = max.max(c.high);
    }
    // Small padding: too much makes candles look flat.
    let pad = ((max - min) * 0.005).max(0.5);
    min -= pad;
    max += pad;
    let range = (max - min).max(1e-9);

    let w = bounds.size.width;
    let h = bounds.size.height;
    let n = slice.len().max(1) as f32;
    let slot = w / n;
    let body_w = (slot * 0.7).max(px(4.0)).min(px(14.0));
    let wick_w = px(2.0);
    let t = gpui::glass::glass_tokens(window);

    for (i, c) in slice.iter().enumerate() {
        let x_center = bounds.origin.x + slot * (i as f32 + 0.5);

        let y_for = |p: f64| -> Pixels {
            let yn = ((p - min) / range) as f32;
            bounds.origin.y + h - (h * yn)
        };

        let y_open = y_for(c.open);
        let y_close = y_for(c.close);
        let y_high = y_for(c.high);
        let y_low = y_for(c.low);

        let is_up = c.close >= c.open;
        let color = if is_up {
            gpui::rgba(0x22c55eff) // green
        } else {
            gpui::rgba(0xef4444ff) // red
        };

        // Wick
        let wick_bounds = Bounds {
            origin: point(x_center - wick_w.half(), y_high.min(y_low)),
            size: size(wick_w, (y_low - y_high).abs()),
        };
        window.paint_quad(fill(wick_bounds, color));

        // Body
        let top = y_open.min(y_close);
        let bottom = y_open.max(y_close);
        let body_h = (bottom - top).max(px(4.0));
        let body_bounds = Bounds {
            origin: point(x_center - body_w.half(), top),
            size: size(body_w, body_h),
        };
        window.paint_quad(fill(body_bounds, color));
    }

    // Subtle baseline at min
    let baseline_y = bounds.origin.y + h - px(1.0);
    let baseline = Bounds {
        origin: point(bounds.origin.x, baseline_y),
        size: size(w, px(1.0)),
    };
    window.paint_quad(fill(baseline, t.rgba(t.divider)));
}

async fn run_mock_feed(tx: Sender<FeedMsg>) {
    let _ = tx.send(FeedMsg::Status("Mock".into())).await;

    // Deterministic pseudo-random-ish walk.
    let mut t: f64 = 0.0;
    let mean: f64 = 68_000.0;
    let mut price: f64 = mean;
    let mut v: f64 = 0.001;

    loop {
        // Advance *market time* faster than real time so candles form quickly.
        // Each tick advances 1 second of simulated time.
        t += 1.0;

        // Oscillate with mean-reversion so candles fill the chart (TradingView-like).
        let wave = (t / 5.0).sin() * 120.0 + (t / 1.9).sin() * 55.0;
        let shock = (t * 1.7).sin() * 22.0;
        let reversion = -(price - mean) * 0.08;
        let step = wave + shock + reversion;
        price = (price + step).max(100.0);
        v = (v * 1.03).clamp(0.0002, 0.02);

        let trade = Trade {
            price,
            qty: v,
            is_sell: step < 0.0,
            time_s: t,
        };

        if tx.send(FeedMsg::Trade(trade)).await.is_err() {
            return;
        }

        // ~10x speed: 1s simulated per 100ms real.
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

fn main() {
    env_logger::init();

    application().run(|cx: &mut App| {
        gpui_tokio::init(cx);

        let bounds = Bounds::centered(None, size(px(980.0), px(560.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some("Live Trading Demo".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(|cx| TradingApp::new(cx)),
        )
        .expect("open window");

        cx.activate(true);
    });
}

