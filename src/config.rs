use std::{env, net::IpAddr};

use thiserror::Error;

#[derive(Debug, Clone)]
pub struct Config {
    pub host: IpAddr,
    pub port: u16,
    pub service_name: String,
    pub log_filter: String,
    pub book_capacity: usize,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("HOST must be a valid IP address: {0}")]
    Host(#[from] std::net::AddrParseError),
    #[error("PORT must be an integer between 1 and 65535")]
    Port,
    #[error("BOOK_CAPACITY must be a positive integer")]
    Capacity,
    #[error("SERVICE_NAME must be set")]
    ServiceName,
}

impl Config {
    /// Loads the service configuration from the process environment.
    ///
    /// # Errors
    ///
    /// Returns an error when a required value is missing or an address, port,
    /// or book capacity is invalid.
    pub fn load() -> Result<Self, ConfigError> {
        if env::var("ENV").as_deref() == Ok("local") {
            dotenvy::dotenv().ok();
        }
        let host = value("HOST", "0.0.0.0").parse()?;
        let port = value("PORT", "8080")
            .parse()
            .ok()
            .filter(|port: &u16| *port > 0)
            .ok_or(ConfigError::Port)?;
        let service_name = env::var("SERVICE_NAME").map_err(|_| ConfigError::ServiceName)?;
        if service_name.is_empty() {
            return Err(ConfigError::ServiceName);
        }
        let book_capacity = value("BOOK_CAPACITY", "100000")
            .parse()
            .ok()
            .filter(|capacity: &usize| *capacity > 0)
            .ok_or(ConfigError::Capacity)?;
        Ok(Self {
            host,
            port,
            service_name,
            log_filter: value("RUST_LOG", &value("LOG_LEVEL", "info")),
            book_capacity,
        })
    }
}

fn value(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_owned())
}
