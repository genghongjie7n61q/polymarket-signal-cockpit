use polymarket_backend::storage::{connect_pool, run_migrations, PgPoolOptionsConfig};

fn database_url() -> String {
    std::env::var("DATABASE_URL")
        .ok()
        .filter(|value| !value.is_empty())
        .expect("DATABASE_URL is required for PostgreSQL migration tests")
}

#[tokio::test]
async fn migrations_apply_and_core_tables_exist() {
    let database_url = database_url();

    let pool = connect_pool(&database_url, PgPoolOptionsConfig { max_connections: 2 })
        .await
        .expect("test database should connect");

    run_migrations(&pool)
        .await
        .expect("migrations should apply");
    run_migrations(&pool)
        .await
        .expect("migrations should be repeatable");

    let expected_tables = [
        "assets",
        "markets",
        "market_windows",
        "raw_market_events",
        "ticks",
        "candles_1m",
        "polymarket_snapshots",
        "models",
        "model_versions",
        "model_assignments",
        "backtest_runs",
        "signals",
        "paper_orders",
        "notification_channels",
        "notification_deliveries",
        "runtime_events",
    ];

    let table_count: i64 = sqlx::query_scalar(
        r#"
        SELECT count(*)
        FROM information_schema.tables
        WHERE table_schema = 'public'
          AND table_name = ANY($1)
        "#,
    )
    .bind(&expected_tables)
    .fetch_one(&pool)
    .await
    .expect("table count query should work");

    assert_eq!(table_count, expected_tables.len() as i64);

    let raw_event_index_predicate: Option<String> = sqlx::query_scalar(
        r#"
        SELECT pg_get_expr(i.indpred, i.indrelid)
        FROM pg_class c
        JOIN pg_index i ON i.indexrelid = c.oid
        WHERE c.relname = 'idx_raw_market_events_source_event_id'
        "#,
    )
    .fetch_optional(&pool)
    .await
    .expect("raw event index predicate query");
    assert_eq!(
        raw_event_index_predicate.as_deref(),
        Some("(source_event_id IS NOT NULL)")
    );

    let tick_index_predicate: Option<String> = sqlx::query_scalar(
        r#"
        SELECT pg_get_expr(i.indpred, i.indrelid)
        FROM pg_class c
        JOIN pg_index i ON i.indexrelid = c.oid
        WHERE c.relname = 'idx_ticks_sequence_idempotency'
        "#,
    )
    .fetch_optional(&pool)
    .await
    .expect("tick index predicate query");
    assert_eq!(
        tick_index_predicate.as_deref(),
        Some("(sequence IS NOT NULL)")
    );
}
