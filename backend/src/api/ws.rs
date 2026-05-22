use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        RawQuery, State,
    },
    response::IntoResponse,
};
use futures_util::SinkExt;
use serde::Serialize;
use time::OffsetDateTime;

use crate::{
    api::dto::{MarketSummaryDto, MarketTickDto, PolymarketSnapshotDto},
    realtime::{LiveMarketState, MarketKey},
    router::AppState,
};

#[derive(Debug, Clone, Serialize)]
struct MarketsSnapshotMessage {
    #[serde(rename = "type")]
    message_type: &'static str,
    generated_at: OffsetDateTime,
    markets: Vec<MarketSummaryDto>,
}

#[derive(Debug, Clone, Serialize)]
struct ErrorMessage {
    #[serde(rename = "type")]
    message_type: &'static str,
    error: &'static str,
}

pub async fn markets_ws(
    State(state): State<AppState>,
    RawQuery(query): RawQuery,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    let unsupported_query = query
        .as_deref()
        .map(|query| !query.trim().is_empty())
        .unwrap_or(false);
    ws.on_upgrade(move |socket| handle_markets_socket(socket, state, unsupported_query))
}

async fn handle_markets_socket(mut socket: WebSocket, state: AppState, unsupported_query: bool) {
    if unsupported_query {
        let _ = send_json(
            &mut socket,
            &ErrorMessage {
                message_type: "error",
                error: "unsupported_query",
            },
        )
        .await;
        let _ = socket.close().await;
        return;
    }

    if send_json(&mut socket, &build_snapshot(&state))
        .await
        .is_err()
    {
        return;
    }

    let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
    loop {
        interval.tick().await;
        if send_json(&mut socket, &build_snapshot(&state))
            .await
            .is_err()
        {
            return;
        }
    }
}

async fn send_json<T: Serialize>(socket: &mut WebSocket, value: &T) -> Result<(), axum::Error> {
    let payload = serde_json::to_string(value).expect("websocket payload should serialize");
    socket.send(Message::Text(payload.into())).await
}

fn build_snapshot(state: &AppState) -> MarketsSnapshotMessage {
    let runtime_state = state
        .realtime
        .as_ref()
        .map(|runtime| runtime.state_snapshot());
    let source_status = state
        .realtime
        .as_ref()
        .map(|runtime| runtime.source_status_at(OffsetDateTime::now_utc()))
        .unwrap_or_default();

    let mut markets = state
        .config
        .supported_markets
        .iter()
        .filter_map(|market| market.parse::<MarketKey>().ok())
        .map(|market_key| {
            let live = runtime_state
                .as_ref()
                .and_then(|snapshot| snapshot.markets.get(&market_key));
            market_summary(
                market_key,
                live,
                source_status.get(default_source(market_key)),
            )
        })
        .collect::<Vec<_>>();
    markets.sort_by_key(|market| market.market_key.clone());

    MarketsSnapshotMessage {
        message_type: "snapshot",
        generated_at: OffsetDateTime::now_utc(),
        markets,
    }
}

fn market_summary(
    market_key: MarketKey,
    live: Option<&LiveMarketState>,
    source_status: Option<&String>,
) -> MarketSummaryDto {
    MarketSummaryDto {
        market_key: market_key.as_str().to_string(),
        symbol: market_key.symbol().to_string(),
        interval_seconds: market_key.interval_seconds(),
        source_status: source_status.cloned(),
        current_window: live.and_then(|market| market.current_window.clone()),
        latest_tick: live
            .and_then(|market| market.latest_tick.as_ref())
            .map(MarketTickDto::from_tick),
        latest_snapshot: live
            .and_then(|market| market.latest_snapshot.as_ref())
            .map(PolymarketSnapshotDto::from_snapshot),
    }
}

fn default_source(market_key: MarketKey) -> &'static str {
    match market_key {
        MarketKey::Btc5m | MarketKey::Eth15m => "coinbase",
    }
}
