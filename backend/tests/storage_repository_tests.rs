use bigdecimal::BigDecimal;
use polymarket_backend::storage::{
    connect_pool, run_migrations, NewNotificationDelivery, NewSignal, NewTick, PgPoolOptionsConfig,
    PostgresStorage, StorageRepository,
};
use serde_json::json;
use sqlx::{PgPool, Row};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

fn database_url() -> Option<String> {
    std::env::var("DATABASE_URL")
        .ok()
        .filter(|value| !value.is_empty())
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
    let Some(ctx) = TestContext::create().await else {
        return;
    };

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
}

#[tokio::test]
async fn repository_inserts_signal_idempotently() {
    let Some(ctx) = TestContext::create().await else {
        return;
    };

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
}

#[tokio::test]
async fn repository_inserts_notification_delivery_idempotently() {
    let Some(ctx) = TestContext::create().await else {
        return;
    };

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
}

#[tokio::test]
async fn repository_queries_replay_ticks_by_market_window() {
    let Some(ctx) = TestContext::create().await else {
        return;
    };

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
    assert!(replay.windows(2).all(|pair| pair[0].source_ts <= pair[1].source_ts));
}

struct TestContext {
    pool: PgPool,
    suffix: String,
    market_key: String,
    market_id: Uuid,
    window_id: Uuid,
    window_start: OffsetDateTime,
    model_version_id: Uuid,
    channel_id: Uuid,
}

impl TestContext {
    async fn create() -> Option<Self> {
        let Some(database_url) = database_url() else {
            eprintln!("DATABASE_URL not set; skipping PostgreSQL repository test");
            return None;
        };

        let pool = connect_pool(
            &database_url,
            PgPoolOptionsConfig { max_connections: 3 },
        )
        .await
        .expect("test database should connect");
        run_migrations(&pool).await.expect("migrations should apply");

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

        Some(Self {
            pool,
            suffix,
            market_key,
            market_id,
            window_id,
            window_start,
            model_version_id,
            channel_id,
        })
    }
}
