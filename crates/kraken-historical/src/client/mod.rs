//! HTTP clients (Kraken REST + generic).

mod http;
mod rest;

pub use http::HttpClient;
pub use rest::{fetch_asset_pairs, AssetPair};
