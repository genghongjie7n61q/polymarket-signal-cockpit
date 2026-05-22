use serde::Serialize;

use crate::notification::NotificationRuntimeSnapshot;
use crate::realtime::{
    CandleSnapshot, LiveMarketSummary, MarketTick, MarketWindowState, PolymarketSnapshot,
    RealtimeRuntimeSnapshot,
};
use crate::storage::{
    BacktestRunRecord, CandleRecord, ModelAssignmentRecord, NotificationChannelRecord,
    NotificationDeliveryRecord, SignalWithMarketRecord, StorageWriterSnapshot,
};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MarketsResponseDto {
    pub markets: Vec<MarketSummaryDto>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CockpitBootstrapDto {
    pub generated_at: time::OffsetDateTime,
    pub markets: Vec<CockpitMarketDto>,
    pub runtime: RuntimeHealthDto,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CockpitMarketDto {
    pub summary: MarketSummaryDto,
    pub recent_candles: Vec<CandleDto>,
    pub active_model: Option<ModelAssignmentDto>,
    pub latest_signal: Option<SignalDto>,
    pub latest_actionable_alert: Option<SignalDto>,
    pub latest_backtest: Option<BacktestRunDto>,
    pub notification_channels: Vec<NotificationChannelDto>,
    pub notification_deliveries: Vec<NotificationDeliveryDto>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MarketSummaryDto {
    pub market_key: String,
    pub symbol: String,
    pub interval_seconds: i64,
    pub source_status: Option<String>,
    pub current_window: Option<MarketWindowState>,
    pub latest_tick: Option<MarketTickDto>,
    pub latest_snapshot: Option<PolymarketSnapshotDto>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MarketStateDto {
    pub market_key: String,
    pub latest_tick: Option<MarketTickDto>,
    pub current_window: Option<MarketWindowState>,
    pub latest_snapshot: Option<PolymarketSnapshotDto>,
    pub recent_candles: Vec<CandleSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CandlesResponseDto {
    pub market_key: String,
    pub candles: Vec<CandleDto>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CandleDto {
    pub start_ts: time::OffsetDateTime,
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
    pub volume: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SignalsResponseDto {
    pub market_key: String,
    pub signals: Vec<SignalDto>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SignalDto {
    pub id: uuid::Uuid,
    pub market_window_id: uuid::Uuid,
    pub model_version_id: uuid::Uuid,
    pub signal_type: String,
    pub side: Option<String>,
    pub confidence: Option<String>,
    pub limit_price: Option<String>,
    pub suggested_size: Option<String>,
    pub ttl_ms: Option<i32>,
    pub reason: String,
    pub features: serde_json::Value,
    pub input_snapshot_hash: String,
    pub created_at: time::OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MarketTickDto {
    pub symbol: String,
    pub source: String,
    pub source_ts: time::OffsetDateTime,
    pub received_at: time::OffsetDateTime,
    pub price: String,
    pub size: Option<String>,
    pub sequence: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PolymarketSnapshotDto {
    pub event_slug: String,
    pub captured_at: time::OffsetDateTime,
    pub up_price: Option<String>,
    pub down_price: Option<String>,
    pub spread: Option<String>,
    pub liquidity: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RuntimeHealthDto {
    pub storage_writer: Option<StorageWriterSnapshot>,
    pub realtime: Option<RealtimeRuntimeSnapshot>,
    pub notification: Option<NotificationRuntimeSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ModelAssignmentsResponseDto {
    pub assignments: Vec<ModelAssignmentDto>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ModelAssignmentDto {
    pub market_key: String,
    pub model_key: String,
    pub display_name: String,
    pub version: String,
    pub parameters: serde_json::Value,
    pub status: String,
    pub created_at: time::OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NotificationChannelsResponseDto {
    pub market_key: String,
    pub channels: Vec<NotificationChannelDto>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NotificationChannelDto {
    pub id: uuid::Uuid,
    pub market_key: String,
    pub channel_type: String,
    pub name: String,
    pub webhook_url: Option<String>,
    pub webhook_url_masked: String,
    pub enabled: bool,
    pub created_at: time::OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NotificationDeliveriesResponseDto {
    pub market_key: String,
    pub deliveries: Vec<NotificationDeliveryDto>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NotificationDeliveryDto {
    pub id: uuid::Uuid,
    pub market_key: String,
    pub signal_id: uuid::Uuid,
    pub channel_id: uuid::Uuid,
    pub channel_type: String,
    pub channel_name: String,
    pub status: String,
    pub attempt_count: i32,
    pub response_summary: Option<String>,
    pub created_at: time::OffsetDateTime,
    pub updated_at: time::OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BacktestsResponseDto {
    pub runs: Vec<BacktestRunDto>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FeishuDryRunResponseDto {
    pub market_key: String,
    pub card_summary: FeishuDryRunCardSummaryDto,
    pub sent: Vec<FeishuDryRunDeliveryDto>,
    pub notification: Option<NotificationRuntimeSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FeishuDryRunCardSummaryDto {
    pub title: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FeishuDryRunDeliveryDto {
    pub channel: NotificationChannelDto,
    pub status: String,
    pub response_summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BacktestRunDto {
    pub id: uuid::Uuid,
    pub market_key: String,
    pub model_key: String,
    pub display_name: String,
    pub model_version: String,
    pub parameters: serde_json::Value,
    pub started_at: time::OffsetDateTime,
    pub finished_at: Option<time::OffsetDateTime>,
    pub window_start: time::OffsetDateTime,
    pub window_end: time::OffsetDateTime,
    pub metrics: serde_json::Value,
    pub status: String,
}

impl MarketTickDto {
    pub fn from_tick(tick: &MarketTick) -> Self {
        Self {
            symbol: tick.symbol.clone(),
            source: tick.source.clone(),
            source_ts: tick.source_ts,
            received_at: tick.received_at,
            price: tick.price.to_string(),
            size: tick.size.as_ref().map(ToString::to_string),
            sequence: tick.sequence,
        }
    }
}

impl MarketSummaryDto {
    pub fn from_live_summary(summary: LiveMarketSummary) -> Self {
        Self {
            market_key: summary.market_key.as_str().to_string(),
            symbol: summary.market_key.symbol().to_string(),
            interval_seconds: summary.market_key.interval_seconds(),
            source_status: summary.source_status,
            current_window: summary.current_window,
            latest_tick: summary.latest_tick.as_ref().map(MarketTickDto::from_tick),
            latest_snapshot: summary
                .latest_snapshot
                .as_ref()
                .map(PolymarketSnapshotDto::from_snapshot),
        }
    }
}

impl PolymarketSnapshotDto {
    pub fn from_snapshot(snapshot: &PolymarketSnapshot) -> Self {
        Self {
            event_slug: snapshot.event_slug.clone(),
            captured_at: snapshot.captured_at,
            up_price: snapshot.up_price.as_ref().map(ToString::to_string),
            down_price: snapshot.down_price.as_ref().map(ToString::to_string),
            spread: snapshot.spread.as_ref().map(ToString::to_string),
            liquidity: snapshot.liquidity.as_ref().map(ToString::to_string),
        }
    }
}

impl CandleDto {
    pub fn from_record(record: CandleRecord) -> Self {
        Self {
            start_ts: record.start_ts,
            open: record.open.to_string(),
            high: record.high.to_string(),
            low: record.low.to_string(),
            close: record.close.to_string(),
            volume: record.volume.to_string(),
        }
    }
}

impl SignalDto {
    pub fn from_record(record: SignalWithMarketRecord) -> Self {
        Self {
            id: record.id,
            market_window_id: record.market_window_id,
            model_version_id: record.model_version_id,
            signal_type: record.signal_type,
            side: record.side,
            confidence: record.confidence.map(|value| value.to_string()),
            limit_price: record.limit_price.map(|value| value.to_string()),
            suggested_size: record.suggested_size.map(|value| value.to_string()),
            ttl_ms: record.ttl_ms,
            reason: record.reason,
            features: record.features,
            input_snapshot_hash: record.input_snapshot_hash,
            created_at: record.created_at,
        }
    }
}

impl ModelAssignmentDto {
    pub fn from_record(record: ModelAssignmentRecord) -> Self {
        Self {
            market_key: record.market_key,
            model_key: record.model_key,
            display_name: record.display_name,
            version: record.version,
            parameters: record.parameters,
            status: record.status,
            created_at: record.created_at,
        }
    }
}

impl NotificationChannelDto {
    pub fn from_record(record: NotificationChannelRecord) -> Self {
        Self {
            id: record.id,
            market_key: record.market_key,
            channel_type: record.channel_type,
            name: record.name,
            webhook_url: None,
            webhook_url_masked: mask_webhook_url(&record.webhook_url),
            enabled: record.enabled,
            created_at: record.created_at,
        }
    }
}

impl NotificationDeliveryDto {
    pub fn from_record(record: NotificationDeliveryRecord) -> Self {
        Self {
            id: record.id,
            market_key: record.market_key,
            signal_id: record.signal_id,
            channel_id: record.channel_id,
            channel_type: record.channel_type,
            channel_name: record.channel_name,
            status: record.status,
            attempt_count: record.attempt_count,
            response_summary: record.response_summary,
            created_at: record.created_at,
            updated_at: record.updated_at,
        }
    }
}

impl BacktestRunDto {
    pub fn from_record(record: BacktestRunRecord) -> Self {
        Self {
            id: record.id,
            market_key: record.market_key,
            model_key: record.model_key,
            display_name: record.display_name,
            model_version: record.model_version,
            parameters: record.parameters,
            started_at: record.started_at,
            finished_at: record.finished_at,
            window_start: record.window_start,
            window_end: record.window_end,
            metrics: record.metrics,
            status: record.status,
        }
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
