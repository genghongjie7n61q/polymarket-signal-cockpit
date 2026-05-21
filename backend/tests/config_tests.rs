use std::collections::HashMap;

use polymarket_backend::config::{AppConfig, ConfigError, Environment};

fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

#[test]
fn local_config_uses_safe_defaults_when_environment_is_empty() {
    let config = AppConfig::from_env_map(env(&[])).expect("local defaults should be valid");

    assert_eq!(config.environment, Environment::Local);
    assert_eq!(config.rust_log, "info");
    assert_eq!(config.app_version, env!("CARGO_PKG_VERSION"));
    assert_eq!(config.host, "127.0.0.1");
    assert_eq!(config.port, 8080);
    assert_eq!(config.database_url, None);
    assert_eq!(config.supported_markets, vec!["btc5m", "eth15m"]);
    assert!(!config.database_configured());
}

#[test]
fn production_config_requires_database_url() {
    let error = AppConfig::from_env_map(env(&[("POLY_ENV", "production")]))
        .expect_err("production should reject a missing database url");

    assert!(matches!(error, ConfigError::MissingDatabaseUrl));
}

#[test]
fn config_parses_explicit_values_and_supported_markets() {
    let config = AppConfig::from_env_map(env(&[
        ("POLY_ENV", "production"),
        ("DATABASE_URL", "postgres://example"),
        ("RUST_LOG", "debug,polymarket_backend=trace"),
        ("APP_VERSION", "2026.05.21"),
        ("APP_HOST", "0.0.0.0"),
        ("APP_PORT", "9090"),
        ("SUPPORTED_MARKETS", " btc5m, eth15m ,, sol15m "),
    ]))
    .expect("explicit production config should be valid");

    assert_eq!(config.environment, Environment::Production);
    assert_eq!(config.database_url.as_deref(), Some("postgres://example"));
    assert_eq!(config.rust_log, "debug,polymarket_backend=trace");
    assert_eq!(config.app_version, "2026.05.21");
    assert_eq!(config.host, "0.0.0.0");
    assert_eq!(config.port, 9090);
    assert_eq!(config.supported_markets, vec!["btc5m", "eth15m", "sol15m"]);
    assert!(config.database_configured());
}

#[test]
fn config_rejects_invalid_port() {
    let error = AppConfig::from_env_map(env(&[("APP_PORT", "not-a-port")]))
        .expect_err("invalid port should be rejected");

    assert!(matches!(error, ConfigError::InvalidPort(_)));
}
