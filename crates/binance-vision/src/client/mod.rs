//! HTTP client for Binance Vision and REST APIs.

mod http;
#[cfg(feature = "cache")]
mod rest;

pub use http::VisionHttpClient;
#[cfg(feature = "cache")]
pub use rest::{BinanceRestClient, ExchangeInfoSymbol};
