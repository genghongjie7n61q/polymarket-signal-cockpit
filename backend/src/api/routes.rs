use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use serde::Deserialize;
use std::collections::BTreeMap;
use time::OffsetDateTime;

use crate::{
    api::dto::{
        BacktestRunDto, BacktestsResponseDto, CandleDto, CandlesResponseDto, CockpitBootstrapDto,
        CockpitMarketDto, FeishuDryRunCardSummaryDto, FeishuDryRunDeliveryDto,
        FeishuDryRunResponseDto, MarketStateDto, MarketSummaryDto, MarketTickDto,
        MarketsResponseDto, ModelAssignmentDto, ModelAssignmentsResponseDto,
        NotificationChannelDto, NotificationChannelsResponseDto, NotificationDeliveriesResponseDto,
        NotificationDeliveryDto, PolymarketSnapshotDto, RuntimeHealthDto, SignalDto,
        SignalsResponseDto,
    },
    api::ws::markets_ws,
    notification::{FeishuCardInput, NotificationChannelView, render_feishu_card},
    realtime::{LiveMarketState, MarketKey},
    router::AppState,
    storage::{
        NewModelAssignment, NewNotificationChannel, NewRuntimeEvent, NotificationChannelRecord,
        StorageCommand,
    },
};

pub fn api_router() -> Router<AppState> {
    Router::new()
        .route("/cockpit/bootstrap", get(cockpit_bootstrap))
        .route("/markets", get(list_markets))
        .route("/markets/{market_key}/state", get(market_state))
        .route("/markets/{market_key}/candles", get(market_candles))
        .route("/signals", get(latest_signals))
        .route("/backtests", get(latest_backtests))
        .route(
            "/notifications/deliveries",
            get(latest_notification_deliveries),
        )
        .route("/notifications/feishu/dry-run", post(feishu_dry_run))
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
struct NotificationDeliveriesQuery {
    market_key: String,
    limit: Option<i64>,
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

#[derive(Debug, Clone, Deserialize)]
struct FeishuDryRunRequest {
    market_key: String,
    channel_name: Option<String>,
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
    Json(runtime_health_dto(&state))
}

async fn cockpit_bootstrap(
    State(state): State<AppState>,
) -> Result<Json<CockpitBootstrapDto>, StatusCode> {
    let now = OffsetDateTime::now_utc();
    let markets = supported_market_keys(&state);
    let summaries = market_summary_map(&state, &markets, now);
    let assignments = match state.storage.as_ref() {
        Some(storage) => storage
            .list_model_assignments()
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
        None => Vec::new(),
    };

    let mut cockpit_markets = Vec::with_capacity(markets.len());
    for market_key in markets {
        let summary = summaries
            .get(market_key.as_str())
            .cloned()
            .unwrap_or_else(|| market_summary(market_key, None, None));
        let active_model = assignments
            .iter()
            .find(|assignment| assignment.market_key == market_key.as_str())
            .cloned();
        let recent_candles = match state.storage.as_ref() {
            Some(storage) => storage
                .recent_candles(market_key.as_str(), 60)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
            None => Vec::new(),
        };
        let signals = match state.storage.as_ref() {
            Some(storage) => storage
                .latest_signals(market_key.as_str(), 20)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
            None => Vec::new(),
        };
        let backtest_runs = match state.storage.as_ref() {
            Some(storage) => {
                let model_key = active_model.as_ref().map(|model| model.model_key.as_str());
                let mut runs = storage
                    .latest_backtest_runs(Some(market_key.as_str()), model_key, 1)
                    .await
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                if runs.is_empty() && model_key.is_some() {
                    runs = storage
                        .latest_backtest_runs(Some(market_key.as_str()), None, 1)
                        .await
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                }
                runs
            }
            None => Vec::new(),
        };
        let channels = match state.storage.as_ref() {
            Some(storage) => storage
                .list_notification_channels(market_key.as_str())
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
            None => Vec::new(),
        };
        let deliveries = match state.storage.as_ref() {
            Some(storage) => storage
                .latest_notification_deliveries(market_key.as_str(), 20)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
            None => Vec::new(),
        };

        cockpit_markets.push(CockpitMarketDto {
            summary,
            recent_candles: recent_candles
                .into_iter()
                .map(CandleDto::from_record)
                .collect(),
            active_model: active_model.map(ModelAssignmentDto::from_record),
            latest_signal: signals.first().cloned().map(SignalDto::from_record),
            latest_actionable_alert: signals
                .iter()
                .find(|signal| signal.signal_type == "actionable_alert")
                .cloned()
                .map(SignalDto::from_record),
            latest_backtest: backtest_runs
                .into_iter()
                .next()
                .map(BacktestRunDto::from_record),
            notification_channels: channels
                .into_iter()
                .map(NotificationChannelDto::from_record)
                .collect(),
            notification_deliveries: deliveries
                .into_iter()
                .map(NotificationDeliveryDto::from_record)
                .collect(),
        });
    }

    Ok(Json(CockpitBootstrapDto {
        generated_at: now,
        markets: cockpit_markets,
        runtime: runtime_health_dto(&state),
    }))
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

async fn latest_notification_deliveries(
    State(state): State<AppState>,
    Query(query): Query<NotificationDeliveriesQuery>,
) -> Result<Json<NotificationDeliveriesResponseDto>, StatusCode> {
    let market_key = supported_market(&state, &query.market_key)?;
    let limit = query.limit.unwrap_or(20).clamp(1, 200);
    let deliveries = match state.storage.as_ref() {
        Some(storage) => storage
            .latest_notification_deliveries(market_key.as_str(), limit)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
        None => Vec::new(),
    };

    Ok(Json(NotificationDeliveriesResponseDto {
        market_key: market_key.as_str().to_string(),
        deliveries: deliveries
            .into_iter()
            .map(NotificationDeliveryDto::from_record)
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

async fn feishu_dry_run(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<FeishuDryRunRequest>,
) -> Result<Json<FeishuDryRunResponseDto>, StatusCode> {
    require_admin_token(&state, &headers)?;
    let market_key = supported_market(&state, &request.market_key)?;
    let storage = state
        .storage
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let notification = state
        .notification
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let channel_name = request
        .channel_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let channels = storage
        .list_notification_channels(market_key.as_str())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .filter(|channel| channel.enabled && channel.channel_type == "feishu")
        .filter(|channel| channel_name.is_none_or(|name| channel.name == name))
        .collect::<Vec<_>>();
    if channels.is_empty() {
        return Err(StatusCode::NOT_FOUND);
    }

    let title = format!("Polymarket {} Dry Run", market_label(market_key));
    let reason = "Dry-run notification probe; no signal or order was created.".to_string();
    let mut sent = Vec::with_capacity(channels.len());
    for channel in channels {
        let payload = render_feishu_card(&dry_run_card_input(market_key, &channel, &reason));
        let result = notification.send_card(&channel.webhook_url, payload).await;
        let (status, response_summary) = match result {
            Ok(outcome) => ("sent".to_string(), outcome.response_summary),
            Err(error) => ("failed".to_string(), Some(error.safe_summary())),
        };
        record_dry_run_attempt(
            &state,
            market_key,
            &channel,
            &status,
            response_summary.as_deref(),
        );
        sent.push(FeishuDryRunDeliveryDto {
            channel: NotificationChannelDto::from_record(channel),
            status,
            response_summary,
        });
    }

    Ok(Json(FeishuDryRunResponseDto {
        market_key: market_key.as_str().to_string(),
        card_summary: FeishuDryRunCardSummaryDto { title, reason },
        sent,
        notification: Some(notification.snapshot()),
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

fn market_summary_map(
    state: &AppState,
    markets: &[MarketKey],
    now: OffsetDateTime,
) -> BTreeMap<String, MarketSummaryDto> {
    let runtime_state = state
        .realtime
        .as_ref()
        .map(|runtime| runtime.state_snapshot());
    let source_status = state
        .realtime
        .as_ref()
        .map(|runtime| runtime.source_status_at(now))
        .unwrap_or_default();

    markets
        .iter()
        .map(|market_key| {
            let live = runtime_state
                .as_ref()
                .and_then(|snapshot| snapshot.markets.get(market_key));
            (
                market_key.as_str().to_string(),
                market_summary(
                    *market_key,
                    live,
                    source_status.get(default_source(*market_key)),
                ),
            )
        })
        .collect()
}

fn runtime_health_dto(state: &AppState) -> RuntimeHealthDto {
    RuntimeHealthDto {
        storage_writer: state
            .storage_writer
            .as_ref()
            .map(|writer| writer.snapshot()),
        realtime: state
            .realtime
            .as_ref()
            .map(|runtime| runtime.snapshot(OffsetDateTime::now_utc())),
        notification: state
            .notification
            .as_ref()
            .map(|runtime| runtime.snapshot()),
    }
}

fn supported_market_keys(state: &AppState) -> Vec<MarketKey> {
    let mut market_keys = state
        .config
        .supported_markets
        .iter()
        .filter_map(|market| market.parse::<MarketKey>().ok())
        .collect::<Vec<_>>();
    market_keys.sort_by_key(|market| market.as_str());
    market_keys
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

fn dry_run_card_input(
    market_key: MarketKey,
    channel: &NotificationChannelRecord,
    reason: &str,
) -> FeishuCardInput {
    let now = OffsetDateTime::now_utc();
    FeishuCardInput {
        market_key: market_key.as_str().to_string(),
        market_label: market_label(market_key),
        window_start: now,
        window_end: now,
        side: "DRY RUN".to_string(),
        confidence: "n/a".to_string(),
        limit_price: "n/a".to_string(),
        suggested_size: "n/a".to_string(),
        ttl_ms: 0,
        model_key: "notification-dry-run".to_string(),
        model_version: "manual".to_string(),
        reason: reason.to_string(),
        features: serde_json::json!({
            "dry_run": true,
            "orders_created": false,
            "signals_created": false
        }),
        channel: NotificationChannelView {
            name: channel.name.clone(),
            webhook_url_masked: mask_webhook_url(&channel.webhook_url),
        },
    }
}

fn record_dry_run_attempt(
    state: &AppState,
    market_key: MarketKey,
    channel: &NotificationChannelRecord,
    status: &str,
    response_summary: Option<&str>,
) {
    let Some(writer) = state.storage_writer.as_ref() else {
        return;
    };

    if writer
        .handle()
        .try_enqueue(StorageCommand::RuntimeEvent(NewRuntimeEvent {
            component: "notification".to_string(),
            severity: if status == "sent" { "info" } else { "warn" }.to_string(),
            event_type: "feishu_dry_run".to_string(),
            message: format!(
                "Feishu dry-run {status} for {} channel {}",
                market_key.as_str(),
                channel.name
            ),
            details: serde_json::json!({
                "market_key": market_key.as_str(),
                "channel_id": channel.id,
                "channel_name": channel.name,
                "channel_type": channel.channel_type,
                "webhook_url_masked": mask_webhook_url(&channel.webhook_url),
                "status": status,
                "response_summary": response_summary,
                "signals_created": false,
                "orders_created": false
            }),
        }))
        .is_err()
    {
        tracing::warn!("failed to enqueue feishu dry-run runtime event");
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

fn market_label(market_key: MarketKey) -> String {
    match market_key {
        MarketKey::Btc5m => "BTC 5m".to_string(),
        MarketKey::Eth15m => "ETH 15m".to_string(),
    }
}

fn mask_webhook_url(webhook_url: &str) -> String {
    let suffix = webhook_url
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    if let Some((scheme, rest)) = webhook_url.split_once("://") {
        let host = rest.split('/').next().unwrap_or("webhook");
        format!("{scheme}://{host}/.../{suffix}")
    } else {
        format!(".../{suffix}")
    }
}

fn default_source(market_key: MarketKey) -> &'static str {
    match market_key {
        MarketKey::Btc5m | MarketKey::Eth15m => "coinbase",
    }
}
