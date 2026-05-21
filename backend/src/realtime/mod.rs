pub mod bus;
pub mod candle;
pub mod normalize;
pub mod runtime;
pub mod state;
pub mod types;
pub mod window;

pub use bus::{RealtimeBus, RealtimeBusSnapshot, RealtimeEvent};
pub use candle::{CandleAggregator, CandleSnapshot};
pub use normalize::{normalize_coinbase_ticker, normalize_polymarket_snapshot};
pub use runtime::{
    RealtimeRuntime, RealtimeRuntimeSnapshot, RealtimeStateHealthSnapshot,
    DEFAULT_REALTIME_QUEUE_CAPACITY,
};
pub use state::{
    LiveMarketState, RealtimeStateMetrics, RealtimeStateOwner, RealtimeStateSnapshot,
    SourceSnapshot, StateOwnerConfig,
};
pub use types::{MarketKey, MarketTick, PolymarketSnapshot, RealtimeError};
pub use window::{window_for_tick, MarketWindowState};
