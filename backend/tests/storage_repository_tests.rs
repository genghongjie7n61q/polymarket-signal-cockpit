use bigdecimal::BigDecimal;
use polymarket_backend::{
    realtime::MarketKey,
    storage::{
        connect_pool, run_migrations, NewBacktestRun, NewModelAssignment, NewNotificationChannel,
        NewNotificationDelivery, NewRawMarketEvent, NewSignal, NewTick, PgPoolOptionsConfig,
        PostgresStorage, StorageRepository,
    },
};
use serde_json::json;
use sqlx::{PgPool, Row};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

fn database_url() -> String {
    std::env::var("DATABASE_URL")
        .ok()
        .filter(|value| !value.is_empty())
        .expect("DATABASE_URL is required for PostgreSQL repository tests")
}

#[test]
fn new_tick_type_is_serializable() {
    let tick = NewTick {
        market_id: Uuid::nil(),
        source: "coinbase".to_string(),
        source_ts: OffsetDateTime::UNIX_EPOCH,
        received_at: OffsetDateTime::UNIX_EPOCH,
        price: BigDecimal::from(100),
        size: Some(BigDecimal::from(2)),
        sequence: Some(1),
    };

    let encoded = serde_json::to_value(&tick).expect("tick should serialize");
    assert_eq!(encoded["source"], "coinbase");
}

#[tokio::test]
async fn repository_inserts_tick_idempotently() {
    let ctx = TestContext::create().await;

    let storage = PostgresStorage::new(ctx.pool.clone());
    let tick = NewTick {
        market_id: ctx.market_id,
        source: "coinbase".to_string(),
        source_ts: ctx.window_start + Duration::seconds(10),
        received_at: ctx.window_start + Duration::seconds(11),
        price: BigDecimal::from(77998),
        size: Some(BigDecimal::from(2)),
        sequence: Some(42),
    };

    let first = storage.insert_tick(&tick).await.expect("first insert");
    let second = storage.insert_tick(&tick).await.expect("second insert");

    assert_eq!(first.id, second.id);
    assert_eq!(second.price, BigDecimal::from(77998));

    ctx.cleanup().await;
}

#[tokio::test]
async fn repository_loads_seeded_market_ids_for_realtime_bridge() {
    let database_url = database_url();
    let pool = connect_pool(&database_url, PgPoolOptionsConfig { max_connections: 3 })
        .await
        .expect("test database should connect");
    run_migrations(&pool)
        .await
        .expect("migrations should apply");
    let storage = PostgresStorage::new(pool);

    let market_ids = storage
        .load_realtime_market_ids()
        .await
        .expect("market ids should load");

    assert!(market_ids.contains_key(&MarketKey::Btc5m));
    assert!(market_ids.contains_key(&MarketKey::Eth15m));
}

#[tokio::test]
async fn repository_preserves_unsequenced_ticks_that_share_timestamp_and_price() {
    let ctx = TestContext::create().await;

    let storage = PostgresStorage::new(ctx.pool.clone());
    let base_tick = NewTick {
        market_id: ctx.market_id,
        source: "coinbase".to_string(),
        source_ts: ctx.window_start + Duration::seconds(12),
        received_at: ctx.window_start + Duration::seconds(13),
        price: BigDecimal::from(78001),
        size: Some(BigDecimal::from(1)),
        sequence: None,
    };
    let mut second_tick = base_tick.clone();
    second_tick.received_at = ctx.window_start + Duration::seconds(14);
    second_tick.size = Some(BigDecimal::from(3));

    let first = storage
        .insert_tick(&base_tick)
        .await
        .expect("first unsequenced insert");
    let second = storage
        .insert_tick(&second_tick)
        .await
        .expect("second unsequenced insert");

    assert_ne!(first.id, second.id);

    let matching_count: i64 = sqlx::query_scalar(
        r#"
        SELECT count(*)
        FROM ticks
        WHERE market_id = $1
          AND source = $2
          AND source_ts = $3
          AND price = $4
        "#,
    )
    .bind(ctx.market_id)
    .bind("coinbase")
    .bind(base_tick.source_ts)
    .bind(&base_tick.price)
    .fetch_one(&ctx.pool)
    .await
    .expect("tick count query");

    assert_eq!(matching_count, 2);

    ctx.cleanup().await;
}

#[tokio::test]
async fn repository_updates_sequenced_tick_corrections() {
    let ctx = TestContext::create().await;

    let storage = PostgresStorage::new(ctx.pool.clone());
    let first_tick = NewTick {
        market_id: ctx.market_id,
        source: "coinbase".to_string(),
        source_ts: ctx.window_start + Duration::seconds(20),
        received_at: ctx.window_start + Duration::seconds(21),
        price: BigDecimal::from(78000),
        size: Some(BigDecimal::from(1)),
        sequence: Some(77),
    };
    let mut corrected_tick = first_tick.clone();
    corrected_tick.received_at = ctx.window_start + Duration::seconds(22);
    corrected_tick.price = BigDecimal::from(78002);
    corrected_tick.size = Some(BigDecimal::from(5));

    let first = storage
        .insert_tick(&first_tick)
        .await
        .expect("first sequenced insert");
    let corrected = storage
        .insert_tick(&corrected_tick)
        .await
        .expect("corrected sequenced insert");

    assert_eq!(first.id, corrected.id);
    assert_eq!(corrected.price, BigDecimal::from(78002));
    assert_eq!(corrected.size, Some(BigDecimal::from(5)));

    ctx.cleanup().await;
}

#[tokio::test]
async fn repository_preserves_raw_events_without_source_event_id() {
    let ctx = TestContext::create().await;

    let storage = PostgresStorage::new(ctx.pool.clone());
    let source = format!("polymarket_ws_{}", ctx.suffix);
    let event = NewRawMarketEvent {
        source: source.clone(),
        source_event_id: None,
        received_at: ctx.window_start + Duration::seconds(1),
        source_ts: Some(ctx.window_start),
        payload: json!({"price": "0.51"}),
    };
    let mut second_event = event.clone();
    second_event.received_at = ctx.window_start + Duration::seconds(2);
    second_event.payload = json!({"price": "0.52"});

    let first = storage
        .insert_raw_market_event(&event)
        .await
        .expect("first raw event");
    let second = storage
        .insert_raw_market_event(&second_event)
        .await
        .expect("second raw event");

    assert_ne!(first.id, second.id);

    let event_count: i64 = sqlx::query_scalar(
        r#"
        SELECT count(*)
        FROM raw_market_events
        WHERE source = $1
          AND source_event_id IS NULL
          AND received_at IN ($2, $3)
        "#,
    )
    .bind(&source)
    .bind(event.received_at)
    .bind(second_event.received_at)
    .fetch_one(&ctx.pool)
    .await
    .expect("raw event count");

    assert_eq!(event_count, 2);

    ctx.cleanup().await;
}

#[tokio::test]
async fn repository_idempotently_updates_raw_events_with_source_event_id() {
    let ctx = TestContext::create().await;

    let storage = PostgresStorage::new(ctx.pool.clone());
    let source = format!("polymarket_ws_{}", ctx.suffix);
    let first_event = NewRawMarketEvent {
        source,
        source_event_id: Some(format!("event-{}", ctx.suffix)),
        received_at: ctx.window_start + Duration::seconds(3),
        source_ts: Some(ctx.window_start),
        payload: json!({"price": "0.51"}),
    };
    let mut corrected_event = first_event.clone();
    corrected_event.received_at = ctx.window_start + Duration::seconds(4);
    corrected_event.payload = json!({"price": "0.53"});

    let first = storage
        .insert_raw_market_event(&first_event)
        .await
        .expect("first raw event");
    let corrected = storage
        .insert_raw_market_event(&corrected_event)
        .await
        .expect("corrected raw event");

    assert_eq!(first.id, corrected.id);
    assert_eq!(corrected.payload, json!({"price": "0.53"}));

    ctx.cleanup().await;
}

#[tokio::test]
async fn repository_inserts_signal_idempotently() {
    let ctx = TestContext::create().await;

    let storage = PostgresStorage::new(ctx.pool.clone());
    let signal = NewSignal {
        market_window_id: ctx.window_id,
        model_version_id: ctx.model_version_id,
        signal_type: "actionable_alert".to_string(),
        side: Some("Up".to_string()),
        confidence: Some(BigDecimal::from(90) / BigDecimal::from(100)),
        limit_price: Some(BigDecimal::from(51) / BigDecimal::from(100)),
        suggested_size: Some(BigDecimal::from(1)),
        ttl_ms: Some(15_000),
        reason: "test signal".to_string(),
        features: json!({"threshold_bps": 4}),
        input_snapshot_hash: format!("snapshot-{}", ctx.suffix),
    };

    let first = storage.insert_signal(&signal).await.expect("first insert");
    let second = storage.insert_signal(&signal).await.expect("second insert");

    assert_eq!(first.id, second.id);
    assert_eq!(second.side.as_deref(), Some("Up"));

    ctx.cleanup().await;
}

#[tokio::test]
async fn repository_inserts_notification_delivery_idempotently() {
    let ctx = TestContext::create().await;

    let storage = PostgresStorage::new(ctx.pool.clone());
    let signal = storage
        .insert_signal(&NewSignal {
            market_window_id: ctx.window_id,
            model_version_id: ctx.model_version_id,
            signal_type: "actionable_alert".to_string(),
            side: Some("Down".to_string()),
            confidence: Some(BigDecimal::from(88) / BigDecimal::from(100)),
            limit_price: Some(BigDecimal::from(49) / BigDecimal::from(100)),
            suggested_size: Some(BigDecimal::from(1)),
            ttl_ms: Some(15_000),
            reason: "test notification".to_string(),
            features: json!({"threshold_bps": 5}),
            input_snapshot_hash: format!("notify-{}", ctx.suffix),
        })
        .await
        .expect("signal insert");

    let delivery = NewNotificationDelivery {
        signal_id: signal.id,
        channel_id: ctx.channel_id,
        dedupe_key: format!("dedupe-{}", ctx.suffix),
        status: "sent".to_string(),
        attempt_count: 1,
        response_summary: Some("ok".to_string()),
    };

    let first_id = storage
        .insert_notification_delivery(&delivery)
        .await
        .expect("first delivery");
    let second_id = storage
        .insert_notification_delivery(&delivery)
        .await
        .expect("second delivery");

    assert_eq!(first_id, second_id);

    ctx.cleanup().await;
}

#[tokio::test]
async fn repository_queries_replay_ticks_by_market_window() {
    let ctx = TestContext::create().await;

    let storage = PostgresStorage::new(ctx.pool.clone());
    for offset in [5, 15, 25] {
        storage
            .insert_tick(&NewTick {
                market_id: ctx.market_id,
                source: "coinbase".to_string(),
                source_ts: ctx.window_start + Duration::seconds(offset),
                received_at: ctx.window_start + Duration::seconds(offset + 1),
                price: BigDecimal::from(78000 + offset),
                size: Some(BigDecimal::from(1)),
                sequence: Some(offset),
            })
            .await
            .expect("tick insert");
    }

    storage
        .insert_tick(&NewTick {
            market_id: ctx.market_id,
            source: "coinbase".to_string(),
            source_ts: ctx.window_start - Duration::seconds(5),
            received_at: ctx.window_start,
            price: BigDecimal::from(1),
            size: None,
            sequence: Some(99),
        })
        .await
        .expect("outside tick insert");

    let replay = storage
        .replay_ticks_for_window(&ctx.market_key, ctx.window_start)
        .await
        .expect("replay query");

    assert_eq!(replay.len(), 3);
    assert!(replay
        .windows(2)
        .all(|pair| pair[0].source_ts <= pair[1].source_ts));

    ctx.cleanup().await;
}

#[tokio::test]
async fn repository_queries_recent_candles_by_market() {
    let ctx = TestContext::create().await;

    let storage = PostgresStorage::new(ctx.pool.clone());
    for offset in [0, 60, 120] {
        sqlx::query(
            r#"
            INSERT INTO candles_1m (market_id, start_ts, open, high, low, close, volume)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(ctx.market_id)
        .bind(ctx.window_start + Duration::seconds(offset))
        .bind(BigDecimal::from(78_000 + offset))
        .bind(BigDecimal::from(78_010 + offset))
        .bind(BigDecimal::from(77_990 + offset))
        .bind(BigDecimal::from(78_005 + offset))
        .bind(BigDecimal::from(offset + 1))
        .execute(&ctx.pool)
        .await
        .expect("candle insert");
    }

    let candles = storage
        .recent_candles(&ctx.market_key, 2)
        .await
        .expect("recent candle query");

    assert_eq!(candles.len(), 2);
    assert_eq!(candles[0].market_key, ctx.market_key);
    assert_eq!(
        candles[0].start_ts,
        ctx.window_start + Duration::seconds(60)
    );
    assert_eq!(candles[0].close, BigDecimal::from(78_065));
    assert_eq!(
        candles[1].start_ts,
        ctx.window_start + Duration::seconds(120)
    );

    ctx.cleanup().await;
}

#[tokio::test]
async fn repository_queries_latest_signals_by_market() {
    let ctx = TestContext::create().await;

    let storage = PostgresStorage::new(ctx.pool.clone());
    for (index, side) in [(1, "Up"), (2, "Down"), (3, "Up")] {
        storage
            .insert_signal(&NewSignal {
                market_window_id: ctx.window_id,
                model_version_id: ctx.model_version_id,
                signal_type: "actionable_alert".to_string(),
                side: Some(side.to_string()),
                confidence: Some(BigDecimal::from(80 + index) / BigDecimal::from(100)),
                limit_price: Some(BigDecimal::from(50 + index) / BigDecimal::from(100)),
                suggested_size: Some(BigDecimal::from(index)),
                ttl_ms: Some(15_000),
                reason: format!("signal {index}"),
                features: json!({"index": index}),
                input_snapshot_hash: format!("latest-{index}-{}", ctx.suffix),
            })
            .await
            .expect("signal insert");
    }

    let signals = storage
        .latest_signals(&ctx.market_key, 2)
        .await
        .expect("latest signal query");

    assert_eq!(signals.len(), 2);
    assert!(signals[0].created_at >= signals[1].created_at);
    assert_eq!(signals[0].market_key, ctx.market_key);
    assert_eq!(signals[0].reason, "signal 3");
    assert_eq!(signals[1].reason, "signal 2");

    ctx.cleanup().await;
}

#[tokio::test]
async fn repository_sets_one_active_model_assignment_per_market() {
    let ctx = TestContext::create().await;

    let storage = PostgresStorage::new(ctx.pool.clone());
    let first = storage
        .set_active_model_assignment(&NewModelAssignment {
            market_key: ctx.market_key.clone(),
            model_key: format!("momentum-{}", ctx.suffix),
            display_name: "Momentum Test".to_string(),
            version: "0.1.0".to_string(),
            parameters: json!({"threshold_bps": 4}),
        })
        .await
        .expect("first assignment");
    let second = storage
        .set_active_model_assignment(&NewModelAssignment {
            market_key: ctx.market_key.clone(),
            model_key: format!("mean-reversion-{}", ctx.suffix),
            display_name: "Mean Reversion Test".to_string(),
            version: "0.2.0".to_string(),
            parameters: json!({"threshold_bps": 7}),
        })
        .await
        .expect("second assignment");

    assert_eq!(first.status, "active");
    assert_eq!(second.status, "active");
    assert_eq!(second.model_key, format!("mean-reversion-{}", ctx.suffix));

    let assignments = storage
        .list_model_assignments()
        .await
        .expect("assignment list");
    let active = assignments
        .into_iter()
        .filter(|assignment| assignment.market_key == ctx.market_key)
        .collect::<Vec<_>>();

    assert_eq!(active.len(), 1);
    assert_eq!(
        active[0].model_key,
        format!("mean-reversion-{}", ctx.suffix)
    );
    assert_eq!(active[0].parameters, json!({"threshold_bps": 7}));

    ctx.cleanup().await;
}

#[tokio::test]
async fn repository_lists_and_upserts_notification_channels_by_market() {
    let ctx = TestContext::create().await;

    let storage = PostgresStorage::new(ctx.pool.clone());
    let first = storage
        .upsert_notification_channel(&NewNotificationChannel {
            market_key: ctx.market_key.clone(),
            channel_type: "feishu".to_string(),
            name: "primary".to_string(),
            webhook_url: "https://open.feishu.cn/open-apis/bot/v2/hook/original".to_string(),
            enabled: true,
        })
        .await
        .expect("first channel");
    let updated = storage
        .upsert_notification_channel(&NewNotificationChannel {
            market_key: ctx.market_key.clone(),
            channel_type: "feishu".to_string(),
            name: "primary".to_string(),
            webhook_url: "https://open.feishu.cn/open-apis/bot/v2/hook/updated".to_string(),
            enabled: false,
        })
        .await
        .expect("updated channel");

    assert_eq!(first.id, updated.id);
    assert!(!updated.enabled);

    let channels = storage
        .list_notification_channels(&ctx.market_key)
        .await
        .expect("channel list");

    let matching = channels
        .into_iter()
        .filter(|channel| channel.name == "primary")
        .collect::<Vec<_>>();
    assert_eq!(matching.len(), 1);
    assert_eq!(matching[0].market_key, ctx.market_key);
    assert_eq!(matching[0].webhook_url, updated.webhook_url);
    assert!(!matching[0].enabled);

    ctx.cleanup().await;
}

#[tokio::test]
async fn repository_inserts_and_lists_backtest_runs() {
    let ctx = TestContext::create().await;
    let storage = PostgresStorage::new(ctx.pool.clone());
    let model_key = format!("backtest-baseline-{}", ctx.suffix);

    let inserted = storage
        .insert_backtest_run(&NewBacktestRun {
            market_key: ctx.market_key.clone(),
            model_key: model_key.clone(),
            display_name: "Backtest Baseline".to_string(),
            model_version: "0.1.0".to_string(),
            parameters: json!({"threshold_bps": 4}),
            window_start: ctx.window_start,
            window_end: ctx.window_start + Duration::minutes(5),
            metrics: json!({"trades": 3, "wins": 2, "eligible": false}),
            status: "completed".to_string(),
        })
        .await
        .expect("insert backtest run");

    let runs = storage
        .latest_backtest_runs(Some(&ctx.market_key), Some(&model_key), 10)
        .await
        .expect("latest runs");

    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].id, inserted.id);
    assert_eq!(runs[0].market_key, ctx.market_key);
    assert_eq!(runs[0].model_key, model_key);
    assert_eq!(runs[0].metrics["trades"], 3);
    assert!(runs[0].finished_at.is_some());

    ctx.cleanup().await;
}

#[tokio::test]
async fn repository_preserves_backtest_run_parameter_snapshots() {
    let ctx = TestContext::create().await;
    let storage = PostgresStorage::new(ctx.pool.clone());
    let model_key = format!("snapshot-baseline-{}", ctx.suffix);

    let first = storage
        .insert_backtest_run(&NewBacktestRun {
            market_key: ctx.market_key.clone(),
            model_key: model_key.clone(),
            display_name: "Snapshot Baseline".to_string(),
            model_version: "0.1.0".to_string(),
            parameters: json!({"threshold_bps": 4}),
            window_start: ctx.window_start,
            window_end: ctx.window_start + Duration::minutes(5),
            metrics: json!({"trades": 4, "wins": 3}),
            status: "completed".to_string(),
        })
        .await
        .expect("first backtest run");
    let second = storage
        .insert_backtest_run(&NewBacktestRun {
            market_key: ctx.market_key.clone(),
            model_key: model_key.clone(),
            display_name: "Snapshot Baseline".to_string(),
            model_version: "0.1.0".to_string(),
            parameters: json!({"threshold_bps": 8}),
            window_start: ctx.window_start + Duration::minutes(5),
            window_end: ctx.window_start + Duration::minutes(10),
            metrics: json!({"trades": 5, "wins": 4}),
            status: "completed".to_string(),
        })
        .await
        .expect("second backtest run");

    let runs = storage
        .latest_backtest_runs(Some(&ctx.market_key), Some(&model_key), 10)
        .await
        .expect("latest runs");
    let first_record = runs
        .iter()
        .find(|run| run.id == first.id)
        .expect("first run should still be listed");
    let second_record = runs
        .iter()
        .find(|run| run.id == second.id)
        .expect("second run should be listed");

    assert_eq!(first_record.parameters, json!({"threshold_bps": 4}));
    assert_eq!(second_record.parameters, json!({"threshold_bps": 8}));

    ctx.cleanup().await;
}

#[tokio::test]
async fn repository_backtest_run_does_not_mutate_active_assignment_parameters() {
    let ctx = TestContext::create().await;
    let storage = PostgresStorage::new(ctx.pool.clone());
    let model_key = format!("baseline-{}", ctx.suffix);

    storage
        .insert_backtest_run(&NewBacktestRun {
            market_key: ctx.market_key.clone(),
            model_key: model_key.clone(),
            display_name: "Baseline Test".to_string(),
            model_version: "0.1.0".to_string(),
            parameters: json!({"threshold_bps": 99}),
            window_start: ctx.window_start,
            window_end: ctx.window_start + Duration::minutes(5),
            metrics: json!({"trades": 1, "wins": 1}),
            status: "completed".to_string(),
        })
        .await
        .expect("backtest run insert");

    let assignments = storage
        .list_model_assignments()
        .await
        .expect("assignment list");
    let active = assignments
        .into_iter()
        .find(|assignment| assignment.market_key == ctx.market_key)
        .expect("active assignment should exist");

    assert_eq!(active.model_key, model_key);
    assert_eq!(active.parameters, json!({"threshold_bps": 4}));

    ctx.cleanup().await;
}

struct TestContext {
    pool: PgPool,
    suffix: String,
    market_key: String,
    asset_id: Uuid,
    market_id: Uuid,
    model_id: Uuid,
    window_id: Uuid,
    window_start: OffsetDateTime,
    model_version_id: Uuid,
    channel_id: Uuid,
}

impl TestContext {
    async fn create() -> Self {
        let database_url = database_url();

        let pool = connect_pool(&database_url, PgPoolOptionsConfig { max_connections: 3 })
            .await
            .expect("test database should connect");
        run_migrations(&pool)
            .await
            .expect("migrations should apply");

        let suffix = Uuid::new_v4().to_string();
        let market_key = format!("test-btc5m-{suffix}");
        let asset_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO assets (symbol, name)
            VALUES ($1, $2)
            ON CONFLICT (symbol) DO UPDATE SET name = EXCLUDED.name
            RETURNING id
            "#,
        )
        .bind(format!("BTC-{suffix}"))
        .bind("Bitcoin Test")
        .fetch_one(&pool)
        .await
        .expect("asset insert");

        let market_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO markets (market_key, asset_id, symbol, interval_seconds, source)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id
            "#,
        )
        .bind(&market_key)
        .bind(asset_id)
        .bind("BTC-USD")
        .bind(300_i32)
        .bind("polymarket")
        .fetch_one(&pool)
        .await
        .expect("market insert");

        let window_start = OffsetDateTime::UNIX_EPOCH + Duration::seconds(1_000_000);
        let window_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO market_windows (market_id, event_slug, start_ts, end_ts)
            VALUES ($1, $2, $3, $4)
            RETURNING id
            "#,
        )
        .bind(market_id)
        .bind(format!("{market_key}-window"))
        .bind(window_start)
        .bind(window_start + Duration::seconds(300))
        .fetch_one(&pool)
        .await
        .expect("window insert");

        let model_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO models (model_key, display_name)
            VALUES ($1, $2)
            RETURNING id
            "#,
        )
        .bind(format!("baseline-{suffix}"))
        .bind("Baseline Test")
        .fetch_one(&pool)
        .await
        .expect("model insert");

        let model_version_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO model_versions (model_id, version, parameters)
            VALUES ($1, $2, $3)
            RETURNING id
            "#,
        )
        .bind(model_id)
        .bind("0.1.0")
        .bind(json!({"threshold_bps": 4}))
        .fetch_one(&pool)
        .await
        .expect("model version insert");

        let channel_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO notification_channels (market_id, channel_type, name, webhook_url)
            VALUES ($1, $2, $3, $4)
            RETURNING id
            "#,
        )
        .bind(market_id)
        .bind("feishu")
        .bind(format!("test-{suffix}"))
        .bind("https://example.invalid/webhook")
        .fetch_one(&pool)
        .await
        .expect("channel insert");

        let assignment_count: i64 = sqlx::query(
            r#"
            INSERT INTO model_assignments (market_id, model_version_id)
            VALUES ($1, $2)
            ON CONFLICT DO NOTHING
            RETURNING 1
            "#,
        )
        .bind(market_id)
        .bind(model_version_id)
        .fetch_all(&pool)
        .await
        .expect("assignment insert")
        .iter()
        .map(|row| row.get::<i32, _>(0) as i64)
        .sum();
        assert_eq!(assignment_count, 1);

        Self {
            pool,
            suffix,
            market_key,
            asset_id,
            market_id,
            model_id,
            window_id,
            window_start,
            model_version_id,
            channel_id,
        }
    }

    async fn cleanup(&self) {
        sqlx::query("DELETE FROM backtest_runs WHERE market_id = $1")
            .bind(self.market_id)
            .execute(&self.pool)
            .await
            .expect("backtest run cleanup");

        sqlx::query(
            r#"
            DELETE FROM notification_deliveries
            WHERE channel_id IN (
                SELECT id FROM notification_channels WHERE market_id = $1
            )
            "#,
        )
        .bind(self.market_id)
        .execute(&self.pool)
        .await
        .expect("notification delivery cleanup");

        sqlx::query(
            r#"
            DELETE FROM paper_orders
            WHERE signal_id IN (
                SELECT s.id
                FROM signals s
                JOIN market_windows w ON w.id = s.market_window_id
                WHERE w.market_id = $1
            )
            "#,
        )
        .bind(self.market_id)
        .execute(&self.pool)
        .await
        .expect("paper order cleanup");

        sqlx::query(
            r#"
            DELETE FROM signals
            WHERE market_window_id IN (
                SELECT id FROM market_windows WHERE market_id = $1
            )
            "#,
        )
        .bind(self.market_id)
        .execute(&self.pool)
        .await
        .expect("signal cleanup");

        sqlx::query("DELETE FROM model_assignments WHERE market_id = $1 OR model_version_id = $2")
            .bind(self.market_id)
            .bind(self.model_version_id)
            .execute(&self.pool)
            .await
            .expect("model assignment cleanup");

        sqlx::query("DELETE FROM notification_channels WHERE market_id = $1")
            .bind(self.market_id)
            .execute(&self.pool)
            .await
            .expect("notification channel cleanup");

        sqlx::query(
            r#"
            DELETE FROM model_versions
            WHERE model_id IN (
                SELECT id FROM models WHERE model_key LIKE $1
            )
            "#,
        )
        .bind(format!("%{}%", self.suffix))
        .execute(&self.pool)
        .await
        .expect("suffixed model version cleanup");

        sqlx::query("DELETE FROM models WHERE model_key LIKE $1")
            .bind(format!("%{}%", self.suffix))
            .execute(&self.pool)
            .await
            .expect("suffixed model cleanup");

        sqlx::query("DELETE FROM ticks WHERE market_id = $1")
            .bind(self.market_id)
            .execute(&self.pool)
            .await
            .expect("tick cleanup");

        sqlx::query("DELETE FROM candles_1m WHERE market_id = $1")
            .bind(self.market_id)
            .execute(&self.pool)
            .await
            .expect("candle cleanup");

        sqlx::query(
            r#"
            DELETE FROM polymarket_snapshots
            WHERE market_window_id IN (
                SELECT id FROM market_windows WHERE market_id = $1
            )
            "#,
        )
        .bind(self.market_id)
        .execute(&self.pool)
        .await
        .expect("snapshot cleanup");

        sqlx::query("DELETE FROM market_windows WHERE market_id = $1")
            .bind(self.market_id)
            .execute(&self.pool)
            .await
            .expect("market window cleanup");

        sqlx::query("DELETE FROM markets WHERE id = $1")
            .bind(self.market_id)
            .execute(&self.pool)
            .await
            .expect("market cleanup");

        sqlx::query("DELETE FROM model_versions WHERE model_id = $1")
            .bind(self.model_id)
            .execute(&self.pool)
            .await
            .expect("model version cleanup");

        sqlx::query("DELETE FROM models WHERE id = $1")
            .bind(self.model_id)
            .execute(&self.pool)
            .await
            .expect("model cleanup");

        sqlx::query("DELETE FROM assets WHERE id = $1")
            .bind(self.asset_id)
            .execute(&self.pool)
            .await
            .expect("asset cleanup");

        sqlx::query(
            r#"
            DELETE FROM raw_market_events
            WHERE source LIKE $1
               OR source_event_id LIKE $2
            "#,
        )
        .bind(format!("%{}%", self.suffix))
        .bind(format!("%{}%", self.suffix))
        .execute(&self.pool)
        .await
        .expect("raw event cleanup");
    }
}
