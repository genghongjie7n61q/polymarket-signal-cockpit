use polymarket_backend::storage::{connect_pool, run_migrations, PgPoolOptionsConfig};

fn database_url() -> Option<String> {
    std::env::var("DATABASE_URL")
        .ok()
        .filter(|value| !value.is_empty())
}

#[tokio::test]
async fn migrations_apply_and_core_tables_exist() {
    let Some(database_url) = database_url() else {
        eprintln!("DATABASE_URL not set; skipping PostgreSQL migration test");
        return;
    };

    let pool = connect_pool(&database_url, PgPoolOptionsConfig { max_connections: 2 })
        .await
        .expect("test database should connect");

    run_migrations(&pool)
        .await
        .expect("migrations should apply");
    run_migrations(&pool)
        .await
        .expect("migrations should be repeatable");

    let table_count: i64 = sqlx::query_scalar(
        r#"
        SELECT count(*)
        FROM information_schema.tables
        WHERE table_schema = 'public'
          AND table_name IN (
            'assets',
            'markets',
            'market_windows',
            'ticks',
            'signals',
            'notification_deliveries',
            'runtime_events'
          )
        "#,
    )
    .fetch_one(&pool)
    .await
    .expect("table count query should work");

    assert_eq!(table_count, 7);
}
