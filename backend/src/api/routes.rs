use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use time::OffsetDateTime;

use crate::{
    api::dto::{
        BacktestRunDto, BacktestsResponseDto, CandleDto, CandlesResponseDto, MarketStateDto,
        MarketSummaryDto, MarketTickDto, MarketsResponseDto, ModelAssignmentDto, ModelAssignmentsResponseDto,
        NotificationChannelDto, NotificationChannelsResponseDto, PolymarketSnapshotDto,
        RuntimeHealthDto, SignalDto, SignalsResponseDto,
    },
    api::ws::markets_ws,
    realtime::{LiveMarketState, MarketKey},
    router::AppState,
    storage::{NewModelAssignment, NewNotificationChannel},
};

pub fn api_router() -> Router<AppState> {
    Router::new()
        .route("/markets", get(list_markets))
        .route("/markets/{market_key}/state", get(market_state))
        .route("/markets/{market_key}/candles", get(market_candles))
        .route("/signals", get(latest_signals))
        .route("/backtests", get(latest_backtests))
        .route("/ws/markets", get(markets_ws))
        .route("/config/model-assignments", get(list_model_assignments))
        .route(
            "/config/model-assignments/{market_key}",
            axum::routing::put(set_model_assignment),
        )
        .route(
            "/config/notification-channels",
            get(list_notification_channels).post(upsert_notification_channel),
        )
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

#[derive(Debug, Clone, Deserialize)]
struct SetModelAssignmentRequest {
    model_key: String,
    display_name: Option<String>,
    version: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
struct NotificationChannelsQuery {
    market_key: String,
}

#[derive(Debug, Clone, Deserialize)]
struct BacktestsQuery {
    market_key: Option<String>,
    model_key: Option<String>,
    limit: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
struct UpsertNotificationChannelRequest {
    market_key: String,
    channel_type: String,
    name: String,
    webhook_url: String,
    enabled: bool,
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

async fn latest_backtests(
    State(state): State<AppState>,
    Query(query): Query<BacktestsQuery>,
) -> Result<Json<BacktestsResponseDto>, StatusCode> {
    let market_key = match query.market_key.as_deref() {
        Some(value) => Some(supported_market(&state, value)?),
        None => None,
    };
    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    let runs = match state.storage.as_ref() {
        Some(storage) => storage
            .latest_backtest_runs(
                market_key.map(|key| key.as_str()),
                query.model_key.as_deref(),
                limit,
            )
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
        None => Vec::new(),
    };

    Ok(Json(BacktestsResponseDto {
        runs: runs.into_iter().map(BacktestRunDto::from_record).collect(),
    }))
}

async fn list_model_assignments(
    State(state): State<AppState>,
) -> Result<Json<ModelAssignmentsResponseDto>, StatusCode> {
    let assignments = match state.storage.as_ref() {
        Some(storage) => storage
            .list_model_assignments()
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
        None => Vec::new(),
    };

    Ok(Json(ModelAssignmentsResponseDto {
        assignments: assignments
            .into_iter()
            .map(ModelAssignmentDto::from_record)
            .collect(),
    }))
}

async fn set_model_assignment(
    State(state): State<AppState>,
    Path(market_key): Path<String>,
    headers: HeaderMap,
    Json(request): Json<SetModelAssignmentRequest>,
) -> Result<Json<ModelAssignmentDto>, StatusCode> {
    require_admin_token(&state, &headers)?;
    let market_key = supported_market(&state, &market_key)?;
    let storage = state
        .storage
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let model_key = non_empty(request.model_key)?;
    let version = non_empty(request.version)?;
    let display_name = request
        .display_name
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| model_key.clone());

    let assignment = storage
        .set_active_model_assignment(&NewModelAssignment {
            market_key: market_key.as_str().to_string(),
            model_key,
            display_name,
            version,
            parameters: request.parameters,
        })
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(ModelAssignmentDto::from_record(assignment)))
}

async fn list_notification_channels(
    State(state): State<AppState>,
    Query(query): Query<NotificationChannelsQuery>,
) -> Result<Json<NotificationChannelsResponseDto>, StatusCode> {
    let market_key = supported_market(&state, &query.market_key)?;
    let channels = match state.storage.as_ref() {
        Some(storage) => storage
            .list_notification_channels(market_key.as_str())
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
        None => Vec::new(),
    };

    Ok(Json(NotificationChannelsResponseDto {
        market_key: market_key.as_str().to_string(),
        channels: channels
            .into_iter()
            .map(NotificationChannelDto::from_record)
            .collect(),
    }))
}

async fn upsert_notification_channel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<UpsertNotificationChannelRequest>,
) -> Result<Json<NotificationChannelDto>, StatusCode> {
    require_admin_token(&state, &headers)?;
    let market_key = supported_market(&state, &request.market_key)?;
    let storage = state
        .storage
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let channel_type = non_empty(request.channel_type)?;
    if channel_type != "feishu" {
        return Err(StatusCode::BAD_REQUEST);
    }
    let name = non_empty(request.name)?;
    let webhook_url = non_empty(request.webhook_url)?;

    let channel = storage
        .upsert_notification_channel(&NewNotificationChannel {
            market_key: market_key.as_str().to_string(),
            channel_type,
            name,
            webhook_url,
            enabled: request.enabled,
        })
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(NotificationChannelDto::from_record(channel)))
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

fn non_empty(value: String) -> Result<String, StatusCode> {
    let value = value.trim().to_string();
    if value.is_empty() {
        Err(StatusCode::BAD_REQUEST)
    } else {
        Ok(value)
    }
}

fn require_admin_token(state: &AppState, headers: &HeaderMap) -> Result<(), StatusCode> {
    let expected = state
        .config
        .admin_api_token
        .as_deref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let provided = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    if provided == expected {
        Ok(())
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

fn default_source(market_key: MarketKey) -> &'static str {
    match market_key {
        MarketKey::Btc5m | MarketKey::Eth15m => "coinbase",
    }
}
