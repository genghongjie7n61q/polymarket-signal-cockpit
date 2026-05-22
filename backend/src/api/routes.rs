use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use time::OffsetDateTime;

use crate::{
    api::dto::{
        MarketStateDto, MarketSummaryDto, MarketTickDto, MarketsResponseDto, PolymarketSnapshotDto,
        RuntimeHealthDto,
    },
    realtime::{LiveMarketState, MarketKey},
    router::AppState,
};

pub fn api_router() -> Router<AppState> {
    Router::new()
        .route("/markets", get(list_markets))
        .route("/markets/{market_key}/state", get(market_state))
        .route("/runtime/health", get(runtime_health))
}

async fn list_markets(State(state): State<AppState>) -> Json<MarketsResponseDto> {
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

    Json(MarketsResponseDto { markets })
}

async fn market_state(
    State(state): State<AppState>,
    Path(market_key): Path<String>,
) -> Result<Json<MarketStateDto>, StatusCode> {
    let market_key = market_key
        .parse::<MarketKey>()
        .map_err(|_| StatusCode::NOT_FOUND)?;
    if !state
        .config
        .supported_markets
        .iter()
        .any(|supported| supported == market_key.as_str())
    {
        return Err(StatusCode::NOT_FOUND);
    }

    let runtime_state = state
        .realtime
        .as_ref()
        .map(|runtime| runtime.state_snapshot());
    let live = runtime_state
        .as_ref()
        .and_then(|snapshot| snapshot.markets.get(&market_key));

    Ok(Json(market_state_dto(market_key, live)))
}

async fn runtime_health(State(state): State<AppState>) -> Json<RuntimeHealthDto> {
    Json(RuntimeHealthDto {
        storage_writer: state
            .storage_writer
            .as_ref()
            .map(|writer| writer.snapshot()),
        realtime: state
            .realtime
            .as_ref()
            .map(|runtime| runtime.snapshot(OffsetDateTime::now_utc())),
    })
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

fn market_state_dto(market_key: MarketKey, live: Option<&LiveMarketState>) -> MarketStateDto {
    MarketStateDto {
        market_key: market_key.as_str().to_string(),
        latest_tick: live
            .and_then(|market| market.latest_tick.as_ref())
            .map(MarketTickDto::from_tick),
        current_window: live.and_then(|market| market.current_window.clone()),
        latest_snapshot: live
            .and_then(|market| market.latest_snapshot.as_ref())
            .map(PolymarketSnapshotDto::from_snapshot),
        recent_candles: live
            .map(|market| market.candles.clone())
            .unwrap_or_default(),
    }
}

fn default_source(market_key: MarketKey) -> &'static str {
    match market_key {
        MarketKey::Btc5m | MarketKey::Eth15m => "coinbase",
    }
}
