use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::time::Duration as StdDuration;
use thiserror::Error;
use time::{Duration, OffsetDateTime};
use tokio::time::timeout;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use tokio_tungstenite::{client_async_tls, connect_async, tungstenite::Message};

use crate::realtime::{
    normalize_coinbase_ticker, normalize_polymarket_snapshot, MarketKey, RealtimeError,
    RealtimeEvent, RealtimeRuntime,
};

pub const COINBASE_WS_ENDPOINT: &str = "wss://advanced-trade-ws.coinbase.com";
pub const POLYMARKET_MARKET_WS_ENDPOINT: &str =
    "wss://ws-subscriptions-clob.polymarket.com/ws/market";
const COINBASE_SOURCE: &str = "coinbase";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoinbaseCollectorConfig {
    pub endpoint: String,
    pub symbols: Vec<String>,
    pub connect_timeout: StdDuration,
    pub proxy: Option<String>,
}

impl Default for CoinbaseCollectorConfig {
    fn default() -> Self {
        Self {
            endpoint: COINBASE_WS_ENDPOINT.to_string(),
            symbols: vec!["BTC-USD".to_string(), "ETH-USD".to_string()],
            connect_timeout: StdDuration::from_secs(10),
            proxy: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolymarketSnapshotRefresherConfig {
    pub market_ws_endpoint: String,
    pub markets: Vec<MarketKey>,
    pub interval: Duration,
}

impl Default for PolymarketSnapshotRefresherConfig {
    fn default() -> Self {
        Self {
            market_ws_endpoint: POLYMARKET_MARKET_WS_ENDPOINT.to_string(),
            markets: vec![MarketKey::Btc5m, MarketKey::Eth15m],
            interval: Duration::seconds(15),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CollectorEvent {
    CoinbaseTicker {
        payload: Value,
        received_at: OffsetDateTime,
    },
    PolymarketSnapshot {
        market_key: MarketKey,
        payload: Value,
        captured_at: OffsetDateTime,
    },
    Heartbeat {
        source: String,
        received_at: OffsetDateTime,
    },
}

pub trait CollectorEventSink: Clone + Send + Sync + 'static {
    fn try_publish(&self, event: RealtimeEvent) -> Result<(), RealtimeError>;
}

impl CollectorEventSink for RealtimeRuntime {
    fn try_publish(&self, event: RealtimeEvent) -> Result<(), RealtimeError> {
        self.publish(event)
    }
}

#[derive(Clone)]
pub struct CollectorIngress<S> {
    sink: S,
}

impl<S> CollectorIngress<S>
where
    S: CollectorEventSink,
{
    pub fn new(sink: S) -> Self {
        Self { sink }
    }

    pub fn handle(&self, event: CollectorEvent) -> Result<(), RealtimeError> {
        let event = match event {
            CollectorEvent::CoinbaseTicker {
                payload,
                received_at,
            } => RealtimeEvent::Tick(normalize_coinbase_ticker(&payload, received_at)?),
            CollectorEvent::PolymarketSnapshot {
                market_key,
                payload,
                captured_at,
            } => RealtimeEvent::PolymarketSnapshot(normalize_polymarket_snapshot(
                market_key,
                &payload,
                captured_at,
            )?),
            CollectorEvent::Heartbeat {
                source,
                received_at,
            } => RealtimeEvent::SourceHeartbeat {
                source,
                received_at,
            },
        };

        self.sink.try_publish(event)
    }
}

pub fn build_coinbase_subscribe_message(config: &CoinbaseCollectorConfig) -> Value {
    json!({
        "type": "subscribe",
        "product_ids": config.symbols,
        "channel": "ticker"
    })
}

pub fn build_coinbase_heartbeat_subscribe_message() -> Value {
    json!({
        "type": "subscribe",
        "channel": "heartbeats"
    })
}

pub fn handle_coinbase_ws_message<S>(
    ingress: &CollectorIngress<S>,
    payload: &Value,
    received_at: OffsetDateTime,
) -> Result<bool, RealtimeError>
where
    S: CollectorEventSink,
{
    if payload.get("channel").and_then(Value::as_str) == Some("ticker") {
        return handle_advanced_trade_ticker_message(ingress, payload, received_at);
    }
    if payload.get("channel").and_then(Value::as_str) == Some("heartbeats") {
        ingress.handle(CollectorEvent::Heartbeat {
            source: COINBASE_SOURCE.to_string(),
            received_at,
        })?;
        return Ok(true);
    }

    match payload.get("type").and_then(Value::as_str) {
        Some("ticker") => {
            ingress.handle(CollectorEvent::CoinbaseTicker {
                payload: payload.clone(),
                received_at,
            })?;
            Ok(true)
        }
        Some("heartbeat") => {
            ingress.handle(CollectorEvent::Heartbeat {
                source: COINBASE_SOURCE.to_string(),
                received_at,
            })?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn handle_advanced_trade_ticker_message<S>(
    ingress: &CollectorIngress<S>,
    payload: &Value,
    received_at: OffsetDateTime,
) -> Result<bool, RealtimeError>
where
    S: CollectorEventSink,
{
    let Some(events) = payload.get("events").and_then(Value::as_array) else {
        return Ok(false);
    };

    let mut published = false;
    for event in events {
        let Some(tickers) = event.get("tickers").and_then(Value::as_array) else {
            continue;
        };
        for ticker in tickers {
            let mut normalized = ticker.clone();
            normalized["time"] = payload
                .get("timestamp")
                .cloned()
                .unwrap_or_else(|| json!(OffsetDateTime::now_utc().to_string()));
            if let Some(sequence) = payload.get("sequence_num") {
                normalized["sequence"] = sequence.clone();
            }
            ingress.handle(CollectorEvent::CoinbaseTicker {
                payload: normalized,
                received_at,
            })?;
            published = true;
        }
    }

    Ok(published)
}

pub async fn run_coinbase_ws_collector_until<S>(
    config: CoinbaseCollectorConfig,
    sink: S,
    max_published: usize,
) -> Result<usize, CollectorRunError>
where
    S: CollectorEventSink,
{
    let (mut ws, _) = timeout(config.connect_timeout, connect_coinbase_ws(&config))
        .await
        .map_err(|_| CollectorRunError::ConnectTimeout)??;
    ws.send(Message::Text(
        build_coinbase_subscribe_message(&config).to_string().into(),
    ))
    .await?;
    ws.send(Message::Text(
        build_coinbase_heartbeat_subscribe_message()
            .to_string()
            .into(),
    ))
    .await?;

    let ingress = CollectorIngress::new(sink);
    let mut published = 0_usize;
    while let Some(message) = ws.next().await {
        match message? {
            Message::Text(text) => {
                let payload = serde_json::from_str::<Value>(&text)?;
                if handle_coinbase_ws_message(&ingress, &payload, OffsetDateTime::now_utc())? {
                    published += 1;
                    if published >= max_published {
                        break;
                    }
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    Ok(published)
}

async fn connect_coinbase_ws(
    config: &CoinbaseCollectorConfig,
) -> Result<
    (
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        tokio_tungstenite::tungstenite::handshake::client::Response,
    ),
    tokio_tungstenite::tungstenite::Error,
> {
    let Some(proxy) = config.proxy.as_deref() else {
        return connect_async(&config.endpoint).await;
    };

    let (host, port) = websocket_host_port(&config.endpoint).map_err(|error| {
        tokio_tungstenite::tungstenite::Error::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            error,
        ))
    })?;
    let proxy_addr = proxy
        .strip_prefix("http://")
        .or_else(|| proxy.strip_prefix("https://"))
        .unwrap_or(proxy);
    let mut stream = TcpStream::connect(proxy_addr).await?;
    let request = format!("CONNECT {host}:{port} HTTP/1.1\r\nHost: {host}:{port}\r\n\r\n");
    stream.write_all(request.as_bytes()).await?;

    let mut response = Vec::with_capacity(1024);
    let mut buffer = [0_u8; 1024];
    loop {
        let read = stream.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        response.extend_from_slice(&buffer[..read]);
        if response.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
        if response.len() > 8192 {
            return Err(tokio_tungstenite::tungstenite::Error::Io(
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "proxy CONNECT response too large",
                ),
            ));
        }
    }

    let response_text = String::from_utf8_lossy(&response);
    if !response_text.starts_with("HTTP/1.1 200") && !response_text.starts_with("HTTP/1.0 200") {
        return Err(tokio_tungstenite::tungstenite::Error::Io(
            std::io::Error::new(
                std::io::ErrorKind::ConnectionRefused,
                format!("proxy CONNECT failed: {response_text}"),
            ),
        ));
    }

    client_async_tls(&config.endpoint, stream).await
}

fn websocket_host_port(endpoint: &str) -> Result<(String, u16), &'static str> {
    let rest = endpoint
        .strip_prefix("wss://")
        .ok_or("only wss endpoints are supported")?;
    let host_port = rest.split('/').next().ok_or("missing websocket host")?;
    if let Some((host, port)) = host_port.rsplit_once(':') {
        let port = port.parse::<u16>().map_err(|_| "invalid websocket port")?;
        Ok((host.to_string(), port))
    } else {
        Ok((host_port.to_string(), 443))
    }
}

#[derive(Debug, Error)]
pub enum CollectorRunError {
    #[error("websocket connect timed out")]
    ConnectTimeout,
    #[error("websocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("realtime error: {0}")]
    Realtime(#[from] RealtimeError),
}
