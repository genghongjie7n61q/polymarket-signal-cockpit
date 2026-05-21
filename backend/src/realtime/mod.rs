pub mod bus;
pub mod candle;
pub mod normalize;
pub mod state;
pub mod types;
pub mod window;

pub use bus::{RealtimeBus, RealtimeBusSnapshot, RealtimeEvent};
pub use candle::{CandleAggregator, CandleSnapshot};
pub use normalize::{normalize_coinbase_ticker, normalize_polymarket_snapshot};
pub use state::{
    LiveMarketState, RealtimeStateMetrics, RealtimeStateOwner, RealtimeStateSnapshot,
    SourceSnapshot, StateOwnerConfig,
};
pub use types::{MarketKey, MarketTick, PolymarketSnapshot, RealtimeError};
pub use window::{window_for_tick, MarketWindowState};
