use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use time::OffsetDateTime;

use crate::{
    api::dto::{
        CandleDto, CandlesResponseDto, MarketStateDto, MarketSummaryDto, MarketTickDto,
        MarketsResponseDto, PolymarketSnapshotDto, RuntimeHealthDto, SignalDto, SignalsResponseDto,
    },
    realtime::{LiveMarketState, MarketKey},
    router::AppState,
};

pub fn api_router() -> Router<AppState> {
    Router::new()
        .route("/markets", get(list_markets))
        .route("/markets/{market_key}/state", get(market_state))
        .route("/markets/{market_key}/candles", get(market_candles))
        .route("/signals", get(latest_signals))
        .route("/runtime/health", get(runtime_health))
}

#[derive(Debug, Clone, Deserialize)]
struct LimitQuery {
    limit: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
struct SignalsQuery {
    market_key: String,
    limit: Option<i64>,
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

async fn market_candles(
    State(state): State<AppState>,
    Path(market_key): Path<String>,
    Query(query): Query<LimitQuery>,
) -> Result<Json<CandlesResponseDto>, StatusCode> {
    let market_key = supported_market(&state, &market_key)?;
    let limit = query.limit.unwrap_or(60).clamp(1, 500);
    let candles = match state.storage.as_ref() {
        Some(storage) => storage
            .recent_candles(market_key.as_str(), limit)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
        None => Vec::new(),
    };

    Ok(Json(CandlesResponseDto {
        market_key: market_key.as_str().to_string(),
        candles: candles.into_iter().map(CandleDto::from_record).collect(),
    }))
}

async fn latest_signals(
    State(state): State<AppState>,
    Query(query): Query<SignalsQuery>,
) -> Result<Json<SignalsResponseDto>, StatusCode> {
    let market_key = supported_market(&state, &query.market_key)?;
    let limit = query.limit.unwrap_or(20).clamp(1, 200);
    let signals = match state.storage.as_ref() {
        Some(storage) => storage
            .latest_signals(market_key.as_str(), limit)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
        None => Vec::new(),
    };

    Ok(Json(SignalsResponseDto {
        market_key: market_key.as_str().to_string(),
        signals: signals.into_iter().map(SignalDto::from_record).collect(),
    }))
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

fn supported_market(state: &AppState, market_key: &str) -> Result<MarketKey, StatusCode> {
    let market_key = market_key
        .parse::<MarketKey>()
        .map_err(|_| StatusCode::NOT_FOUND)?;
    if state
        .config
        .supported_markets
        .iter()
        .any(|supported| supported == market_key.as_str())
    {
        Ok(market_key)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

fn default_source(market_key: MarketKey) -> &'static str {
    match market_key {
        MarketKey::Btc5m | MarketKey::Eth15m => "coinbase",
    }
}
