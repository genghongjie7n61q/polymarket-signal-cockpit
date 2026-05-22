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
pub const POLYMARKET_GAMMA_EVENTS_BASE_URL: &str = "https://gamma-api.polymarket.com/events";
pub const POLYMARKET_CLOB_PRICES_URL: &str = "https://clob.polymarket.com/prices";
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
    pub gamma_events_base_url: String,
    pub clob_prices_url: String,
    pub markets: Vec<MarketKey>,
    pub interval: Duration,
    pub request_timeout: StdDuration,
    pub http_proxy: Option<String>,
}

impl Default for PolymarketSnapshotRefresherConfig {
    fn default() -> Self {
        Self {
            market_ws_endpoint: POLYMARKET_MARKET_WS_ENDPOINT.to_string(),
            gamma_events_base_url: POLYMARKET_GAMMA_EVENTS_BASE_URL.to_string(),
            clob_prices_url: POLYMARKET_CLOB_PRICES_URL.to_string(),
            markets: vec![MarketKey::Btc5m, MarketKey::Eth15m],
            interval: Duration::seconds(15),
            request_timeout: StdDuration::from_secs(10),
            http_proxy: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolymarketMarketMetadata {
    pub market_key: MarketKey,
    pub event_slug: String,
    pub up_token_id: String,
    pub down_token_id: String,
    pub fallback_up_price: Option<String>,
    pub fallback_down_price: Option<String>,
    pub liquidity: Option<String>,
    pub spread: Option<String>,
    pub accepting_orders: Option<bool>,
    pub closed: Option<bool>,
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

pub fn polymarket_event_slug_at(market_key: MarketKey, ts: OffsetDateTime) -> String {
    crate::realtime::window_for_tick(market_key, ts).event_slug
}

pub fn build_polymarket_prices_request(up_token_id: &str, down_token_id: &str) -> Value {
    json!([
        { "token_id": up_token_id, "side": "BUY" },
        { "token_id": down_token_id, "side": "BUY" }
    ])
}

pub fn extract_polymarket_market(
    market_key: MarketKey,
    event_slug: &str,
    event: &Value,
) -> Result<PolymarketMarketMetadata, CollectorRunError> {
    let markets = event
        .get("markets")
        .and_then(Value::as_array)
        .ok_or_else(|| CollectorRunError::PolymarketMetadata("missing markets".to_string()))?;
    let market = markets
        .iter()
        .find(|market| market.get("slug").and_then(Value::as_str) == Some(event_slug))
        .ok_or_else(|| {
            CollectorRunError::PolymarketMetadata(format!("missing market slug {event_slug}"))
        })?;

    let outcomes = parse_polymarket_string_array(market.get("outcomes"), "outcomes")?;
    let token_ids = parse_polymarket_string_array(market.get("clobTokenIds"), "clobTokenIds")?;
    let fallback_prices =
        parse_polymarket_string_array(market.get("outcomePrices"), "outcomePrices")
            .unwrap_or_default();
    let up_idx = outcome_index(&outcomes, "up")?;
    let down_idx = outcome_index(&outcomes, "down")?;

    Ok(PolymarketMarketMetadata {
        market_key,
        event_slug: event_slug.to_string(),
        up_token_id: token_ids
            .get(up_idx)
            .cloned()
            .ok_or_else(|| CollectorRunError::PolymarketMetadata("missing Up token".to_string()))?,
        down_token_id: token_ids.get(down_idx).cloned().ok_or_else(|| {
            CollectorRunError::PolymarketMetadata("missing Down token".to_string())
        })?,
        fallback_up_price: fallback_prices.get(up_idx).cloned(),
        fallback_down_price: fallback_prices.get(down_idx).cloned(),
        liquidity: stringify_optional_field(market.get("liquidity")),
        spread: stringify_optional_field(market.get("spread")),
        accepting_orders: market.get("acceptingOrders").and_then(Value::as_bool),
        closed: market.get("closed").and_then(Value::as_bool),
    })
}

pub fn merge_polymarket_snapshot_payload(
    market: &PolymarketMarketMetadata,
    clob_prices: &Value,
) -> Result<Value, CollectorRunError> {
    let up_price =
        clob_price_for_token(clob_prices, &market.up_token_id).or(market.fallback_up_price.clone());
    let down_price = clob_price_for_token(clob_prices, &market.down_token_id)
        .or(market.fallback_down_price.clone());

    let mut payload = json!({
        "market_key": market.market_key.as_str(),
        "event_slug": market.event_slug,
        "up_token_id": market.up_token_id,
        "down_token_id": market.down_token_id,
        "accepting_orders": market.accepting_orders,
        "closed": market.closed
    });
    if let Some(price) = up_price {
        payload["up_price"] = Value::String(price);
    }
    if let Some(price) = down_price {
        payload["down_price"] = Value::String(price);
    }
    if let Some(liquidity) = &market.liquidity {
        payload["liquidity"] = Value::String(liquidity.clone());
    }
    if let Some(spread) = &market.spread {
        payload["spread"] = Value::String(spread.clone());
    }

    Ok(payload)
}

pub async fn run_polymarket_snapshot_refresher_until<S>(
    config: PolymarketSnapshotRefresherConfig,
    sink: S,
    max_published: usize,
) -> Result<usize, CollectorRunError>
where
    S: CollectorEventSink,
{
    let client = build_polymarket_http_client(&config)?;
    let ingress = CollectorIngress::new(sink);
    let mut published = 0_usize;

    loop {
        for market_key in &config.markets {
            let captured_at = OffsetDateTime::now_utc();
            let event_slug = polymarket_event_slug_at(*market_key, captured_at);
            match fetch_and_publish_polymarket_snapshot(
                &client,
                &config,
                &ingress,
                *market_key,
                &event_slug,
                captured_at,
            )
            .await
            {
                Ok(()) => {
                    published += 1;
                    if published >= max_published {
                        return Ok(published);
                    }
                }
                Err(error) => {
                    tracing::warn!(%error, ?market_key, %event_slug, "polymarket snapshot refresh failed");
                }
            }
        }

        tokio::time::sleep(config.interval.unsigned_abs()).await;
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

fn parse_polymarket_string_array(
    value: Option<&Value>,
    field: &'static str,
) -> Result<Vec<String>, CollectorRunError> {
    match value {
        Some(Value::String(raw)) => serde_json::from_str::<Vec<String>>(raw).map_err(|error| {
            CollectorRunError::PolymarketJsonField {
                field,
                error: error.to_string(),
            }
        }),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| {
                item.as_str().map(str::to_string).ok_or_else(|| {
                    CollectorRunError::PolymarketJsonField {
                        field,
                        error: "array item is not a string".to_string(),
                    }
                })
            })
            .collect(),
        Some(other) => Err(CollectorRunError::PolymarketJsonField {
            field,
            error: format!("expected JSON string array, got {other}"),
        }),
        None => Err(CollectorRunError::PolymarketJsonField {
            field,
            error: "missing field".to_string(),
        }),
    }
}

fn outcome_index(outcomes: &[String], expected: &str) -> Result<usize, CollectorRunError> {
    outcomes
        .iter()
        .position(|outcome| outcome.eq_ignore_ascii_case(expected))
        .ok_or_else(|| CollectorRunError::PolymarketMetadata(format!("missing {expected} outcome")))
}

fn stringify_optional_field(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(value)) if !value.is_empty() => Some(value.clone()),
        Some(Value::Number(value)) => Some(value.to_string()),
        _ => None,
    }
}

fn clob_price_for_token(prices: &Value, token_id: &str) -> Option<String> {
    stringify_optional_field(prices.get(token_id)?.get("BUY"))
}

async fn fetch_and_publish_polymarket_snapshot<S>(
    client: &reqwest::Client,
    config: &PolymarketSnapshotRefresherConfig,
    ingress: &CollectorIngress<S>,
    market_key: MarketKey,
    event_slug: &str,
    captured_at: OffsetDateTime,
) -> Result<(), CollectorRunError>
where
    S: CollectorEventSink,
{
    let event = fetch_polymarket_event(client, config, event_slug).await?;
    let market = extract_polymarket_market(market_key, event_slug, &event)?;
    let prices = fetch_polymarket_prices(client, config, &market).await?;
    let payload = merge_polymarket_snapshot_payload(&market, &prices)?;
    ingress.handle(CollectorEvent::PolymarketSnapshot {
        market_key,
        payload,
        captured_at,
    })?;
    Ok(())
}

fn build_polymarket_http_client(
    config: &PolymarketSnapshotRefresherConfig,
) -> Result<reqwest::Client, CollectorRunError> {
    let mut builder = reqwest::Client::builder().timeout(config.request_timeout);
    if let Some(proxy) = config.http_proxy.as_deref() {
        builder = builder.proxy(reqwest::Proxy::all(proxy)?);
    }
    Ok(builder.build()?)
}

async fn fetch_polymarket_event(
    client: &reqwest::Client,
    config: &PolymarketSnapshotRefresherConfig,
    event_slug: &str,
) -> Result<Value, CollectorRunError> {
    let base = config.gamma_events_base_url.trim_end_matches('/');
    Ok(client
        .get(format!("{base}/slug/{event_slug}"))
        .send()
        .await?
        .error_for_status()?
        .json::<Value>()
        .await?)
}

async fn fetch_polymarket_prices(
    client: &reqwest::Client,
    config: &PolymarketSnapshotRefresherConfig,
    market: &PolymarketMarketMetadata,
) -> Result<Value, CollectorRunError> {
    Ok(client
        .post(&config.clob_prices_url)
        .json(&build_polymarket_prices_request(
            &market.up_token_id,
            &market.down_token_id,
        ))
        .send()
        .await?
        .error_for_status()?
        .json::<Value>()
        .await?)
}

#[derive(Debug, Error)]
pub enum CollectorRunError {
    #[error("websocket connect timed out")]
    ConnectTimeout,
    #[error("websocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("realtime error: {0}")]
    Realtime(#[from] RealtimeError),
    #[error("polymarket metadata error: {0}")]
    PolymarketMetadata(String),
    #[error("polymarket json field {field}: {error}")]
    PolymarketJsonField { field: &'static str, error: String },
}
