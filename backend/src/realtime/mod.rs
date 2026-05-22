pub mod bus;
pub mod candle;
pub mod collector;
pub mod normalize;
pub mod runtime;
pub mod state;
pub mod storage_bridge;
pub mod types;
pub mod window;

pub use bus::{RealtimeBus, RealtimeBusSnapshot, RealtimeEvent};
pub use candle::{CandleAggregator, CandleSnapshot};
pub use collector::{
    build_coinbase_heartbeat_subscribe_message, build_coinbase_subscribe_message,
    build_polymarket_prices_request, extract_polymarket_market, handle_coinbase_ws_message,
    merge_polymarket_snapshot_payload, polymarket_event_slug_at, run_coinbase_ws_collector_until,
    run_polymarket_snapshot_refresher_until, CoinbaseCollectorConfig, CollectorEvent,
    CollectorEventSink, CollectorIngress, CollectorRunError, PolymarketMarketMetadata,
    PolymarketSnapshotRefresherConfig, COINBASE_WS_ENDPOINT, POLYMARKET_MARKET_WS_ENDPOINT,
};
pub use normalize::{normalize_coinbase_ticker, normalize_polymarket_snapshot};
pub use runtime::{
    RealtimeRuntime, RealtimeRuntimeSnapshot, RealtimeStateHealthSnapshot,
    DEFAULT_REALTIME_QUEUE_CAPACITY,
};
pub use state::{
    LiveMarketState, RealtimeStateMetrics, RealtimeStateOwner, RealtimeStateSnapshot,
    SourceSnapshot, StateOwnerConfig,
};
pub use storage_bridge::{RealtimeStorageBridge, RealtimeStorageBridgeSnapshot};
pub use types::{MarketKey, MarketTick, PolymarketSnapshot, RealtimeError};
pub use window::{window_for_tick, MarketWindowState};
