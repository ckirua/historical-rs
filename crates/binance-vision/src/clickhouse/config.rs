//! ClickHouse connection configuration.

#[derive(Debug, Clone)]
pub struct ClickHouseConfig {
    pub host: String,
    pub port: u16,
    pub http_port: u16,
    pub user: String,
    pub password: String,
    pub database: String,
}

impl ClickHouseConfig {
    /// Target database for Vision product tables.
    ///
    /// Prefers `CLICKHOUSE_VISION_DATABASE`, then `CLICKHOUSE_BINANCE_DATABASE`,
    /// defaulting to **`binance_vision`** (not `binance`).
    pub fn from_env() -> crate::error::Result<Self> {
        let host = std::env::var("CLICKHOUSE_HOST")
            .unwrap_or_else(|_| "127.0.0.1".to_string())
            .trim()
            .to_string();
        let port = std::env::var("CLICKHOUSE_PORT")
            .unwrap_or_else(|_| "9000".to_string())
            .trim()
            .parse()
            .map_err(|_| crate::error::Error::Config("invalid CLICKHOUSE_PORT".into()))?;
        let http_port = std::env::var("CLICKHOUSE_HTTP_PORT")
            .unwrap_or_else(|_| "8123".to_string())
            .trim()
            .parse()
            .map_err(|_| crate::error::Error::Config("invalid CLICKHOUSE_HTTP_PORT".into()))?;
        let user = std::env::var("CLICKHOUSE_USER")
            .unwrap_or_else(|_| "default".to_string())
            .trim()
            .to_string();
        let password = std::env::var("CLICKHOUSE_PASSWORD")
            .map_err(|_| crate::error::Error::Config("CLICKHOUSE_PASSWORD is not set".into()))?
            .trim()
            .to_string();
        let database = std::env::var("CLICKHOUSE_VISION_DATABASE")
            .or_else(|_| std::env::var("CLICKHOUSE_BINANCE_DATABASE"))
            .unwrap_or_else(|_| "binance_vision".to_string())
            .trim()
            .to_string();
        Ok(Self {
            host,
            port,
            http_port,
            user,
            password,
            database,
        })
    }

    pub fn url(&self) -> String {
        format!("http://{}:{}", self.host, self.http_port)
    }
}
