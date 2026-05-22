use bigdecimal::BigDecimal;
use polymarket_backend::{
    model::{
        ModelAction, ModelAssignment, ModelCandle, ModelContext, ModelPolymarket, ModelPortfolio,
        ModelRegistry, ModelRuntime, ModelRuntimeError, ModelTickInput, ModelWindow,
        BASELINE_DIRECTION_KEY, BASELINE_DIRECTION_VERSION,
    },
    realtime::MarketKey,
};
use serde_json::json;
use std::str::FromStr;
use time::{Duration, OffsetDateTime};
use tokio::time::{sleep, timeout};

#[tokio::test]
async fn model_runtime_processes_context_and_records_candidate() {
    let runtime = ModelRuntime::spawn(ModelRegistry::built_ins(), 8);
    let mut context = sample_context(MarketKey::Btc5m);
    context.latest_tick.price = bd("110");
    context.polymarket.up_price = Some(bd("0.50"));

    runtime
        .try_enqueue(context)
        .expect("context should enqueue");

    timeout(Duration::seconds(2).unsigned_abs(), async {
        loop {
            let snapshot = runtime.snapshot();
            if snapshot.processed == 1 {
                assert_eq!(snapshot.accepted, 1);
                assert_eq!(snapshot.candidates, 1);
                assert_eq!(snapshot.no_trades, 0);
                assert_eq!(snapshot.failed, 0);
                assert_eq!(snapshot.last_action, Some(ModelAction::Candidate));
                assert_eq!(
                    snapshot
                        .last_decision
                        .as_ref()
                        .expect("decision should be retained")
                        .features()["model_version"],
                    json!(BASELINE_DIRECTION_VERSION)
                );
                break;
            }
            sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("runtime should process the context");
}

#[tokio::test]
async fn model_runtime_records_unknown_assignment_as_failed() {
    let runtime = ModelRuntime::spawn(ModelRegistry::built_ins(), 8);
    let mut context = sample_context(MarketKey::Btc5m);
    context.assignment.model_key = "missing_model".to_string();

    runtime
        .try_enqueue(context)
        .expect("context should enqueue");

    timeout(Duration::seconds(2).unsigned_abs(), async {
        loop {
            let snapshot = runtime.snapshot();
            if snapshot.processed == 1 {
                assert_eq!(snapshot.accepted, 1);
                assert_eq!(snapshot.failed, 1);
                assert_eq!(snapshot.candidates, 0);
                break;
            }
            sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("runtime should process the context");
}

#[tokio::test]
async fn model_runtime_reports_queue_full_without_blocking() {
    let runtime = ModelRuntime::spawn_paused_for_tests(1);
    let context = sample_context_for_model("blocking_model", "0.1.0");

    runtime
        .try_enqueue(context.clone())
        .expect("first context should enqueue");

    let error = runtime
        .try_enqueue(context)
        .expect_err("second context should see full queue");

    assert_eq!(error, ModelRuntimeError::QueueFull);
    assert_eq!(runtime.snapshot().accepted, 1);
    assert_eq!(runtime.snapshot().dropped, 1);
}

fn sample_context(market_key: MarketKey) -> ModelContext {
    let start_ts = OffsetDateTime::UNIX_EPOCH + Duration::seconds(1_779_330_300);
    ModelContext {
        market_key,
        symbol: market_key.symbol().to_string(),
        assignment: ModelAssignment {
            model_key: BASELINE_DIRECTION_KEY.to_string(),
            model_version: BASELINE_DIRECTION_VERSION.to_string(),
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

fn sample_context_for_model(model_key: &str, model_version: &str) -> ModelContext {
    let mut context = sample_context(MarketKey::Btc5m);
    context.assignment.model_key = model_key.to_string();
    context.assignment.model_version = model_version.to_string();
    context
}

fn bd(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).expect("decimal literal should parse")
}
