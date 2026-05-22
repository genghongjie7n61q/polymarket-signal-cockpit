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

use crate::{api::dto::MarketSummaryDto, realtime::MarketKey, router::AppState};

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
    interval.tick().await;
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
    let now = OffsetDateTime::now_utc();
    let market_keys = state
        .config
        .supported_markets
        .iter()
        .filter_map(|market| market.parse::<MarketKey>().ok())
        .collect::<Vec<_>>();

    let mut markets = match state.realtime.as_ref() {
        Some(runtime) => runtime
            .market_summaries_at(&market_keys, now)
            .into_iter()
            .map(MarketSummaryDto::from_live_summary)
            .collect::<Vec<_>>(),
        None => market_keys
            .into_iter()
            .map(|market_key| MarketSummaryDto {
                market_key: market_key.as_str().to_string(),
                symbol: market_key.symbol().to_string(),
                interval_seconds: market_key.interval_seconds(),
                source_status: None,
                current_window: None,
                latest_tick: None,
                latest_snapshot: None,
            })
            .collect::<Vec<_>>(),
    };
    markets.sort_by_key(|market| market.market_key.clone());

    MarketsSnapshotMessage {
        message_type: "snapshot",
        generated_at: now,
        markets,
    }
}
