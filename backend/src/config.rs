use std::{collections::HashMap, env, str::FromStr};

use thiserror::Error;

const DEFAULT_RUST_LOG: &str = "info";
const DEFAULT_APP_HOST: &str = "127.0.0.1";
const DEFAULT_APP_PORT: u16 = 8080;
const DEFAULT_SUPPORTED_MARKETS: &[&str] = &["btc5m", "eth15m"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    pub database_url: Option<String>,
    pub environment: Environment,
    pub rust_log: String,
    pub app_version: String,
    pub host: String,
    pub port: u16,
    pub supported_markets: Vec<String>,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_env_map(env::vars())
    }

    pub fn from_env_map<I, K, V>(vars: I) -> Result<Self, ConfigError>
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        let vars = vars
            .into_iter()
            .map(|(key, value)| (key.into(), value.into()))
            .collect::<HashMap<String, String>>();

        let environment = get_non_empty(&vars, "POLY_ENV")
            .unwrap_or_else(|| "local".to_string())
            .parse()?;
        let database_url = get_non_empty(&vars, "DATABASE_URL");

        if environment == Environment::Production && database_url.is_none() {
            return Err(ConfigError::MissingDatabaseUrl);
        }

        Ok(Self {
            database_url,
            environment,
            rust_log: get_non_empty(&vars, "RUST_LOG")
                .unwrap_or_else(|| DEFAULT_RUST_LOG.to_string()),
            app_version: get_non_empty(&vars, "APP_VERSION")
                .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string()),
            host: get_non_empty(&vars, "APP_HOST").unwrap_or_else(|| DEFAULT_APP_HOST.to_string()),
            port: parse_port(get_non_empty(&vars, "APP_PORT"))?,
            supported_markets: parse_supported_markets(get_non_empty(&vars, "SUPPORTED_MARKETS")),
        })
    }

    pub fn database_configured(&self) -> bool {
        self.database_url.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Environment {
    Local,
    Production,
}

impl Environment {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Production => "production",
        }
    }
}

impl FromStr for Environment {
    type Err = ConfigError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "local" => Ok(Self::Local),
            "production" => Ok(Self::Production),
            other => Err(ConfigError::InvalidEnvironment(other.to_string())),
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("DATABASE_URL is required when POLY_ENV=production")]
    MissingDatabaseUrl,
    #[error("POLY_ENV must be either local or production, got {0}")]
    InvalidEnvironment(String),
    #[error("APP_PORT must be a TCP port number between 1 and 65535, got {0}")]
    InvalidPort(String),
}

fn get_non_empty(vars: &HashMap<String, String>, key: &str) -> Option<String> {
    vars.get(key)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn parse_supported_markets(value: Option<String>) -> Vec<String> {
    let markets = value
        .as_deref()
        .unwrap_or("")
        .split(',')
        .map(str::trim)
        .filter(|market| !market.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    if markets.is_empty() {
        DEFAULT_SUPPORTED_MARKETS
            .iter()
            .map(|market| (*market).to_string())
            .collect()
    } else {
        markets
    }
}

fn parse_port(value: Option<String>) -> Result<u16, ConfigError> {
    match value {
        Some(port) => port
            .parse::<u16>()
            .ok()
            .filter(|port| *port > 0)
            .ok_or(ConfigError::InvalidPort(port)),
        None => Ok(DEFAULT_APP_PORT),
    }
}
