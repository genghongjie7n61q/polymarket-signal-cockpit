use polymarket_backend::backtest::{
    replay_backtest, BacktestEligibilityPolicy, ReplayWindow, ReplayWindowCandidate,
};
use serde_json::json;
use time::{Duration, OffsetDateTime};

#[test]
fn replay_freezes_first_actionable_alert_per_window() {
    let start = OffsetDateTime::UNIX_EPOCH;
    let windows = vec![ReplayWindow {
        market_key: "btc5m".to_string(),
        window_start: start,
        window_end: start + Duration::minutes(5),
        open_price: "100".parse().unwrap(),
        final_price: "101".parse().unwrap(),
        candidates: vec![
            candidate(
                start + Duration::seconds(90),
                "Down",
                "0.57",
                "0.50",
                "first",
            ),
            candidate(
                start + Duration::seconds(180),
                "Up",
                "0.90",
                "0.50",
                "later-better",
            ),
        ],
    }];

    let result = replay_backtest(
        "baseline_direction",
        "0.1.0",
        "btc5m:test",
        json!({"threshold_bps": 4}),
        &windows,
    );

    assert_eq!(result.replay_outputs.len(), 1);
    assert_eq!(result.replay_outputs[0].alert.side, "Down");
    assert_eq!(result.replay_outputs[0].alert.reason, "first");
    assert!(!result.replay_outputs[0].won);
    assert_eq!(result.replay_outputs[0].profit_loss.to_string(), "-0.5");
    assert_eq!(result.metrics.trades, 1);
    assert_eq!(result.metrics.wins, 0);
}

#[test]
fn replay_ignores_windows_without_actionable_candidates_for_coverage() {
    let start = OffsetDateTime::UNIX_EPOCH;
    let windows = vec![
        ReplayWindow {
            market_key: "btc5m".to_string(),
            window_start: start,
            window_end: start + Duration::minutes(5),
            open_price: "100".parse().unwrap(),
            final_price: "101".parse().unwrap(),
            candidates: Vec::new(),
        },
        ReplayWindow {
            market_key: "btc5m".to_string(),
            window_start: start + Duration::minutes(5),
            window_end: start + Duration::minutes(10),
            open_price: "101".parse().unwrap(),
            final_price: "99".parse().unwrap(),
            candidates: vec![candidate(
                start + Duration::minutes(6),
                "Down",
                "0.65",
                "0.52",
                "short",
            )],
        },
    ];

    let result = replay_backtest(
        "baseline_direction",
        "0.1.0",
        "btc5m:test",
        json!({}),
        &windows,
    );

    assert_eq!(result.metrics.windows, 2);
    assert_eq!(result.metrics.trades, 1);
    assert_eq!(result.metrics.wins, 1);
    assert_eq!(result.metrics.coverage.to_string(), "0.5");
}

#[test]
fn replay_is_deterministic_for_same_inputs() {
    let windows = sample_windows();
    let first = replay_backtest(
        "baseline_direction",
        "0.1.0",
        "btc5m:test",
        json!({"threshold_bps": 4}),
        &windows,
    );
    let second = replay_backtest(
        "baseline_direction",
        "0.1.0",
        "btc5m:test",
        json!({"threshold_bps": 4}),
        &windows,
    );

    assert_eq!(first, second);
}

#[test]
fn eligibility_requires_enough_recent_trades_wilson_floor_and_coverage() {
    let policy = BacktestEligibilityPolicy {
        min_trades: 20,
        min_wilson_lower_bound: "0.55".parse().unwrap(),
        min_coverage: "0.20".parse().unwrap(),
        min_expected_value: "0".parse().unwrap(),
        max_drawdown: "5".parse().unwrap(),
        max_consecutive_losses: 5,
        max_average_price_paid: "0.70".parse().unwrap(),
    };
    let result = replay_backtest(
        "baseline_direction",
        "0.1.0",
        "btc5m:test",
        json!({}),
        &sample_windows(),
    );

    let eligibility = result.metrics.production_eligibility(&policy);

    assert!(!eligibility.eligible);
    assert!(eligibility
        .reasons
        .iter()
        .any(|reason| reason.contains("min_trades")));
}

#[test]
fn eligibility_rejects_negative_ev_and_excessive_risk_even_when_stats_pass() {
    let mut result = replay_backtest(
        "baseline_direction",
        "0.1.0",
        "btc5m:test",
        json!({}),
        &sample_windows(),
    );
    result.metrics.trades = 30;
    result.metrics.wilson_lower_bound = "0.60".parse().unwrap();
    result.metrics.coverage = "0.50".parse().unwrap();
    result.metrics.expected_value = "-0.01".parse().unwrap();
    result.metrics.max_drawdown = "8".parse().unwrap();
    result.metrics.max_consecutive_losses = 6;
    result.metrics.average_price_paid = "0.72".parse().unwrap();

    let policy = BacktestEligibilityPolicy {
        min_trades: 20,
        min_wilson_lower_bound: "0.55".parse().unwrap(),
        min_coverage: "0.20".parse().unwrap(),
        min_expected_value: "0".parse().unwrap(),
        max_drawdown: "5".parse().unwrap(),
        max_consecutive_losses: 5,
        max_average_price_paid: "0.70".parse().unwrap(),
    };

    let eligibility = result.metrics.production_eligibility(&policy);

    assert!(!eligibility.eligible);
    assert!(eligibility
        .reasons
        .iter()
        .any(|reason| reason.contains("min_expected_value")));
    assert!(eligibility
        .reasons
        .iter()
        .any(|reason| reason.contains("max_drawdown")));
    assert!(eligibility
        .reasons
        .iter()
        .any(|reason| reason.contains("max_average_price_paid")));
    assert!(eligibility
        .reasons
        .iter()
        .any(|reason| reason.contains("max_consecutive_losses")));
}

fn sample_windows() -> Vec<ReplayWindow> {
    let start = OffsetDateTime::UNIX_EPOCH;
    vec![
        ReplayWindow {
            market_key: "btc5m".to_string(),
            window_start: start,
            window_end: start + Duration::minutes(5),
            open_price: "100".parse().unwrap(),
            final_price: "101".parse().unwrap(),
            candidates: vec![candidate(
                start + Duration::seconds(90),
                "Up",
                "0.70",
                "0.50",
                "long",
            )],
        },
        ReplayWindow {
            market_key: "btc5m".to_string(),
            window_start: start + Duration::minutes(5),
            window_end: start + Duration::minutes(10),
            open_price: "101".parse().unwrap(),
            final_price: "99".parse().unwrap(),
            candidates: vec![candidate(
                start + Duration::minutes(6),
                "Down",
                "0.65",
                "0.52",
                "short",
            )],
        },
    ]
}

fn candidate(
    created_at: OffsetDateTime,
    side: &str,
    confidence: &str,
    limit_price: &str,
    reason: &str,
) -> ReplayWindowCandidate {
    ReplayWindowCandidate {
        created_at,
        side: side.to_string(),
        confidence: confidence.parse().unwrap(),
        limit_price: limit_price.parse().unwrap(),
        suggested_size: "1.0".parse().unwrap(),
        ttl_ms: 15_000,
        reason: reason.to_string(),
        features: json!({"source": "test"}),
        input_snapshot_hash: format!("snapshot-{side}-{reason}"),
    }
}
