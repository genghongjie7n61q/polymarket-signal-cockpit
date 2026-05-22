mod baseline;
mod registry;
mod runtime;
mod sizing;
mod types;

pub use baseline::{
    BaselineDirectionConfig, BaselineDirectionModel, BASELINE_DIRECTION_KEY,
    BASELINE_DIRECTION_VERSION,
};
pub use registry::{built_in_model_version, ModelRegistry};
pub use runtime::{ModelRuntime, ModelRuntimeError, ModelRuntimeSnapshot};
pub use sizing::{SizingConfig, SizingEngine};
pub use types::{
    BacktestResult, ModelAction, ModelAssignment, ModelCandidate, ModelCandle, ModelContext,
    ModelDecision, ModelPolymarket, ModelPortfolio, ModelSide, ModelTickInput, ModelWindow,
    StrategyModel,
};
