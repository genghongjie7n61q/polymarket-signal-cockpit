use bigdecimal::BigDecimal;
use polymarket_backend::{
    model::{
        BaselineDirectionConfig, BaselineDirectionModel, ModelAction, ModelAssignment, ModelCandle,
        ModelContext, ModelPolymarket, ModelPortfolio, ModelSide, ModelTickInput, ModelWindow,
        StrategyModel,
    },
    realtime::MarketKey,
};
use serde_json::json;
use std::str::FromStr;
use time::{Duration, OffsetDateTime};

#[test]
fn baseline_returns_no_trade_before_decision_offset() {
    let model = BaselineDirectionModel::btc5m_default();
    let mut context = sample_context(MarketKey::Btc5m);
    context.window.elapsed_ms = 30_000;

    let decision = model.decide(&context);

    assert_eq!(decision.action, ModelAction::NoTrade);
    assert_eq!(decision.reason, "waiting for decision offset");
}

#[test]
fn baseline_returns_no_trade_when_move_is_below_threshold() {
    let model = BaselineDirectionModel::btc5m_default();
    let mut context = sample_context(MarketKey::Btc5m);
    context.latest_tick.price = bd("100.01");

    let decision = model.decide(&context);

    assert_eq!(decision.action, ModelAction::NoTrade);
    assert_eq!(decision.reason, "move below threshold");
}

#[test]
fn baseline_returns_up_candidate_with_ttl_limit_price_size_and_features() {
    let model = BaselineDirectionModel::btc5m_default();
    let mut context = sample_context(MarketKey::Btc5m);
    context.latest_tick.price = bd("110");
    context.polymarket.up_price = Some(bd("0.50"));

    let decision = model.decide(&context);

    assert_eq!(decision.action, ModelAction::Candidate);
    assert_eq!(decision.side, Some(ModelSide::Up));
    assert_eq!(decision.limit_price, Some(bd("0.50")));
    assert_eq!(decision.suggested_size, Some(bd("1.5")));
    assert_eq!(decision.ttl_ms(), Some(15_000));
    assert_eq!(decision.features()["model_key"], json!("baseline_direction"));
    assert_eq!(decision.features()["model_version"], json!("0.1.0"));
    assert_eq!(decision.features()["market_key"], json!("btc5m"));
    assert_eq!(decision.features()["return_bps"], json!("1000"));
    assert_eq!(
        decision.features()["input_snapshot_hash"],
        json!(context.input_snapshot_hash())
    );
}

#[test]
fn baseline_returns_down_candidate_using_down_price() {
    let model = BaselineDirectionModel::eth15m_default();
    let mut context = sample_context(MarketKey::Eth15m);
    context.latest_tick.price = bd("90");
    context.polymarket.down_price = Some(bd("0.48"));

    let decision = model.decide(&context);

    assert_eq!(decision.action, ModelAction::Candidate);
    assert_eq!(decision.side, Some(ModelSide::Down));
    assert_eq!(decision.limit_price, Some(bd("0.48")));
    assert_eq!(decision.ttl_ms(), Some(15_000));
}

#[test]
fn baseline_is_deterministic_for_identical_context() {
    let model = BaselineDirectionModel::new(BaselineDirectionConfig::default());
    let context = sample_context(MarketKey::Btc5m);

    assert_eq!(model.decide(&context), model.decide(&context));
}

fn sample_context(market_key: MarketKey) -> ModelContext {
    let start_ts = OffsetDateTime::UNIX_EPOCH + Duration::seconds(1_779_330_300);
    ModelContext {
        market_key,
        symbol: market_key.symbol().to_string(),
        assignment: ModelAssignment {
            model_key: "baseline_direction".to_string(),
            model_version: "0.1.0".to_string(),
            parameters: json!({"threshold_bps": 4}),
        },
        window: ModelWindow {
            event_slug: format!("{}-window", market_key.as_str()),
            start_ts,
            end_ts: start_ts + Duration::seconds(market_key.interval_seconds()),
            elapsed_ms: 180_000,
        },
        latest_tick: ModelTickInput {
            source: "coinbase".to_string(),
            source_ts: start_ts + Duration::seconds(180),
            received_at: start_ts + Duration::seconds(181),
            price: bd("100"),
            size: Some(bd("0.25")),
            sequence: Some(42),
        },
        candles: vec![ModelCandle {
            start_ts,
            open: bd("100"),
            high: bd("100"),
            low: bd("100"),
            close: bd("100"),
            volume: bd("3.5"),
        }],
        polymarket: ModelPolymarket {
            event_slug: format!("{}-window", market_key.as_str()),
            up_price: Some(bd("0.51")),
            down_price: Some(bd("0.49")),
            spread: Some(bd("0.02")),
            liquidity: Some(bd("12500")),
        },
        portfolio: ModelPortfolio {
            bankroll: bd("15"),
            max_signal_size: bd("1.5"),
        },
    }
}

fn bd(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).expect("decimal literal should parse")
}
