use sqlx::{postgres::PgPoolOptions, PgPool};

#[derive(Debug, Clone, Copy)]
pub struct PgPoolOptionsConfig {
    pub max_connections: u32,
}

impl Default for PgPoolOptionsConfig {
    fn default() -> Self {
        Self { max_connections: 5 }
    }
}

pub async fn connect_pool(
    database_url: &str,
    options: PgPoolOptionsConfig,
) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(options.max_connections)
        .connect(database_url)
        .await
}

pub async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}
