use bigdecimal::BigDecimal;
use polymarket_backend::{
    model::{
        BacktestResult, ModelAction, ModelAssignment, ModelCandidate, ModelCandle, ModelContext,
        ModelDecision, ModelPolymarket, ModelPortfolio, ModelSide, ModelTickInput, ModelWindow,
    },
    realtime::MarketKey,
};
use serde_json::json;
use std::str::FromStr;
use time::{Duration, OffsetDateTime};

#[test]
fn model_context_hash_is_stable_for_identical_input_snapshot() {
    let context = sample_context(MarketKey::Btc5m);

    assert_eq!(context.input_snapshot_hash(), context.input_snapshot_hash());
    assert_eq!(
        context.input_snapshot_hash(),
        sample_context(MarketKey::Btc5m).input_snapshot_hash()
    );
}

#[test]
fn model_context_hash_changes_when_input_changes() {
    let mut context = sample_context(MarketKey::Btc5m);
    let original_hash = context.input_snapshot_hash();

    context.latest_tick.price = bd("101.25");

    assert_ne!(original_hash, context.input_snapshot_hash());
}

#[test]
fn candidate_decision_exposes_replay_fields() {
    let decision = ModelDecision::candidate(ModelCandidate {
        side: ModelSide::Up,
        confidence: bd("0.62"),
        limit_price: bd("0.51"),
        suggested_size: bd("1.25"),
        ttl_ms: 15_000,
        reason: "threshold crossed".to_string(),
        features: json!({"return_bps": 8.1}),
    });

    assert_eq!(decision.action, ModelAction::Candidate);
    assert_eq!(decision.ttl_ms(), Some(15_000));
    assert_eq!(decision.features()["return_bps"], json!(8.1));
    assert_eq!(decision.side, Some(ModelSide::Up));
}

#[test]
fn no_trade_decision_uses_empty_feature_snapshot() {
    let decision = ModelDecision::no_trade("waiting for decision offset");

    assert_eq!(decision.action, ModelAction::NoTrade);
    assert_eq!(decision.reason, "waiting for decision offset");
    assert_eq!(decision.ttl_ms(), None);
    assert_eq!(decision.features(), &json!({}));
}

#[test]
fn backtest_result_carries_model_version_and_metrics() {
    let result = BacktestResult {
        model_key: "baseline_direction".to_string(),
        model_version: "0.1.0".to_string(),
        dataset: "btc5m_recent_576".to_string(),
        trades: 10,
        wins: 7,
        accuracy: bd("0.70"),
        wilson_lower_bound: bd("0.40"),
        coverage: bd("0.25"),
        max_drawdown: bd("0.10"),
        parameters: json!({"threshold_bps": 4}),
    };

    assert_eq!(result.model_key, "baseline_direction");
    assert_eq!(result.model_version, "0.1.0");
    assert_eq!(result.parameters["threshold_bps"], json!(4));
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
            event_slug: "btc-updown-5m-1779330300".to_string(),
            start_ts,
            end_ts: start_ts + Duration::seconds(market_key.interval_seconds()),
            elapsed_ms: 180_000,
        },
        latest_tick: ModelTickInput {
            source: "coinbase".to_string(),
            source_ts: start_ts + Duration::seconds(180),
            received_at: start_ts + Duration::seconds(181),
            price: bd("101"),
            size: Some(bd("0.25")),
            sequence: Some(42),
        },
        candles: vec![ModelCandle {
            start_ts,
            open: bd("100"),
            high: bd("101"),
            low: bd("99"),
            close: bd("101"),
            volume: bd("3.5"),
        }],
        polymarket: ModelPolymarket {
            event_slug: "btc-updown-5m-1779330300".to_string(),
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
