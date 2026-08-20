//! Process configuration loaded from the environment and an optional
//! `.env` file.
//!
//! This is the only module permitted to call [`std::env::var`]. Every other
//! module receives an already-parsed, validated [`Config`] value — nothing
//! is hardcoded at the call site.

use std::{env, net::IpAddr, str::FromStr};

use crate::domain::Symbol;

const DEFAULT_HOST: &str = "0.0.0.0";
const DEFAULT_PORT: &str = "8080";
const DEFAULT_SERVICE_NAME: &str = "ob";
const DEFAULT_LOG_LEVEL: &str = "info";
const DEFAULT_BOOK_CAPACITY: &str = "100000";
const DEFAULT_RING_CAPACITY: &str = "1024";
const DEFAULT_SYMBOLS: &str = "BTC-USD";

/// Runtime configuration for the matching engine service.
#[derive(Debug, Clone)]
pub struct Config {
    pub host: IpAddr,
    pub port: u16,
    pub service_name: String,
    pub log_level: String,
    /// Resting-order capacity reserved per configured symbol.
    pub book_capacity: usize,
    /// Per-symbol Disruptor command ring capacity. Must be a power of two.
    pub ring_capacity: usize,
    /// Symbols each engine is prepared to trade at startup.
    pub symbols: Vec<Symbol>,
}

/// A single environment variable that is present but fails to parse or
/// validate.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("environment variable {name}={value:?} is invalid: {reason}")]
pub struct ConfigError {
    name: &'static str,
    value: String,
    reason: String,
}

impl Config {
    /// Loads configuration from the process environment.
    ///
    /// Reads an optional `.env` file in the working directory first; a
    /// missing or unreadable file is ignored, and explicit process
    /// environment variables always take precedence over `.env` entries.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] when a variable is present but fails to parse
    /// or validate.
    pub fn load() -> Result<Self, ConfigError> {
        let _ = dotenvy::dotenv();
        Self::from_lookup(|name| env::var(name).ok())
    }

    /// Builds configuration from an arbitrary variable lookup.
    ///
    /// Split out from [`Config::load`] so tests can supply an in-memory
    /// source instead of mutating the process environment.
    fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let raw =
            |name: &'static str, default: &str| lookup(name).unwrap_or_else(|| default.to_owned());
        Ok(Self {
            host: parse("HOST", raw("HOST", DEFAULT_HOST))?,
            port: parse("PORT", raw("PORT", DEFAULT_PORT))?,
            service_name: non_empty("SERVICE_NAME", raw("SERVICE_NAME", DEFAULT_SERVICE_NAME))?,
            log_level: non_empty("LOG_LEVEL", raw("LOG_LEVEL", DEFAULT_LOG_LEVEL))?,
            book_capacity: parse("BOOK_CAPACITY", raw("BOOK_CAPACITY", DEFAULT_BOOK_CAPACITY))?,
            ring_capacity: parse("RING_CAPACITY", raw("RING_CAPACITY", DEFAULT_RING_CAPACITY))?,
            symbols: symbols("SYMBOLS", raw("SYMBOLS", DEFAULT_SYMBOLS))?,
        })
    }
}

fn parse<T>(name: &'static str, value: String) -> Result<T, ConfigError>
where
    T: FromStr,
    T::Err: std::fmt::Display,
{
    value.parse().map_err(|error: T::Err| ConfigError {
        name,
        value,
        reason: error.to_string(),
    })
}

fn non_empty(name: &'static str, value: String) -> Result<String, ConfigError> {
    if value.trim().is_empty() {
        return Err(ConfigError {
            name,
            value,
            reason: "must not be empty".to_owned(),
        });
    }
    Ok(value)
}

fn symbols(name: &'static str, value: String) -> Result<Vec<Symbol>, ConfigError> {
    let parsed = value
        .split(',')
        .map(str::trim)
        .filter(|symbol| !symbol.is_empty())
        .map(|symbol| {
            Symbol::parse(symbol).map_err(|_| ConfigError {
                name,
                value: value.clone(),
                reason: format!("{symbol:?} is not a valid symbol"),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if parsed.is_empty() {
        return Err(ConfigError {
            name,
            value,
            reason: "must list at least one symbol".to_owned(),
        });
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn load(vars: &[(&str, &str)]) -> Result<Config, ConfigError> {
        let vars: HashMap<&str, &str> = vars.iter().copied().collect();
        Config::from_lookup(|name| vars.get(name).map(|value| (*value).to_owned()))
    }

    #[test]
    fn defaults_apply_when_unset() {
        let config = load(&[]).unwrap();
        assert_eq!(config.port, 8080);
        assert_eq!(config.book_capacity, 100_000);
        assert_eq!(config.ring_capacity, 1_024);
        assert_eq!(config.symbols, vec![Symbol::parse("BTC-USD").unwrap()]);
    }

    #[test]
    fn explicit_values_override_defaults() {
        let config = load(&[
            ("PORT", "9090"),
            ("BOOK_CAPACITY", "10"),
            ("SYMBOLS", "BTC-USD, ETH-USD"),
        ])
        .unwrap();
        assert_eq!(config.port, 9090);
        assert_eq!(config.book_capacity, 10);
        assert_eq!(
            config.symbols,
            vec![
                Symbol::parse("BTC-USD").unwrap(),
                Symbol::parse("ETH-USD").unwrap()
            ]
        );
    }

    #[test]
    fn invalid_port_is_rejected() {
        let error = load(&[("PORT", "not-a-port")]).unwrap_err();
        assert_eq!(error.name, "PORT");
    }

    #[test]
    fn empty_symbols_is_rejected() {
        let error = load(&[("SYMBOLS", "  ")]).unwrap_err();
        assert_eq!(error.name, "SYMBOLS");
    }
}
