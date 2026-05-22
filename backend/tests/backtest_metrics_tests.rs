use polymarket_backend::backtest::{
    compute_backtest_metrics, BacktestReplayOutput, FrozenActionableAlert,
};
use serde_json::json;
use time::OffsetDateTime;

#[test]
fn metrics_compute_wilson_drawdown_ev_and_calibration() {
    let outputs = vec![
        output("btc5m", "Up", true, "0.62", "0.50", "2.0"),
        output("btc5m", "Down", false, "0.58", "0.50", "2.0"),
        output("btc5m", "Up", true, "0.72", "0.55", "2.0"),
    ];

    let metrics = compute_backtest_metrics(
        "baseline_direction",
        "0.1.0",
        "btc5m:3",
        5,
        &outputs,
        json!({"threshold_bps": 4}),
    );

    assert_eq!(metrics.model_key, "baseline_direction");
    assert_eq!(metrics.model_version, "0.1.0");
    assert_eq!(metrics.market_key, "btc5m");
    assert_eq!(metrics.dataset, "btc5m:3");
    assert_eq!(metrics.trades, 3);
    assert_eq!(metrics.wins, 2);
    assert_eq!(metrics.coverage.to_string(), "0.6");
    assert_eq!(metrics.accuracy.to_string(), "0.666666667");
    assert_eq!(metrics.expected_value.to_string(), "0.3");
    assert_eq!(metrics.max_drawdown.to_string(), "1");
    assert_eq!(metrics.max_consecutive_losses, 1);
    assert_eq!(metrics.average_price_paid.to_string(), "0.516666667");
    assert_eq!(metrics.calibration_buckets.len(), 3);
    assert!(metrics.wilson_lower_bound.to_string().starts_with("0.20"));
    assert_eq!(metrics.parameters, json!({"threshold_bps": 4}));
}

#[test]
fn metrics_return_zero_values_for_no_covered_windows() {
    let metrics = compute_backtest_metrics(
        "baseline_direction",
        "0.1.0",
        "btc5m:empty",
        0,
        &[],
        json!({}),
    );

    assert_eq!(metrics.trades, 0);
    assert_eq!(metrics.wins, 0);
    assert_eq!(metrics.accuracy.to_string(), "0");
    assert_eq!(metrics.coverage.to_string(), "0");
    assert_eq!(metrics.wilson_lower_bound.to_string(), "0");
    assert_eq!(metrics.max_drawdown.to_string(), "0");
    assert_eq!(metrics.expected_value.to_string(), "0");
    assert_eq!(metrics.average_price_paid.to_string(), "0");
    assert!(metrics.calibration_buckets.is_empty());
}

#[test]
fn metrics_keep_btc_and_eth_parameter_sets_independent() {
    let btc = compute_backtest_metrics(
        "baseline_direction",
        "0.1.0",
        "btc5m:1",
        1,
        &[output("btc5m", "Up", true, "0.70", "0.50", "1.0")],
        json!({"threshold_bps": 3}),
    );
    let eth = compute_backtest_metrics(
        "baseline_direction",
        "0.1.0",
        "eth15m:1",
        1,
        &[output("eth15m", "Down", true, "0.80", "0.55", "1.0")],
        json!({"threshold_bps": 8}),
    );

    assert_eq!(btc.market_key, "btc5m");
    assert_eq!(eth.market_key, "eth15m");
    assert_eq!(btc.parameters, json!({"threshold_bps": 3}));
    assert_eq!(eth.parameters, json!({"threshold_bps": 8}));
}

fn output(
    market_key: &str,
    side: &str,
    won: bool,
    confidence: &str,
    price_paid: &str,
    size: &str,
) -> BacktestReplayOutput {
    BacktestReplayOutput {
        market_key: market_key.to_string(),
        window_start: OffsetDateTime::UNIX_EPOCH,
        window_end: OffsetDateTime::UNIX_EPOCH + time::Duration::minutes(5),
        alert: FrozenActionableAlert {
            created_at: OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(90),
            side: side.to_string(),
            confidence: confidence.parse().unwrap(),
            limit_price: price_paid.parse().unwrap(),
            suggested_size: size.parse().unwrap(),
            ttl_ms: 15_000,
            reason: "test alert".to_string(),
            features: json!({"fixture": true}),
            input_snapshot_hash: format!("{market_key}-{side}-{won}"),
        },
        open_price: "100".parse().unwrap(),
        final_price: if won && side == "Up" || !won && side == "Down" {
            "101".parse().unwrap()
        } else {
            "99".parse().unwrap()
        },
        won,
        profit_loss: "0".parse().unwrap(),
    }
}
