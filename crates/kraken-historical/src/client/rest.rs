//! Kraken public AssetPairs REST helper.

use serde::Deserialize;
use serde_json::Value;

use crate::client::HttpClient;
use crate::error::{Error, Result};

#[derive(Debug, Clone)]
pub struct AssetPair {
    pub name: String,
    pub altname: String,
    pub wsname: Option<String>,
    pub base: String,
    pub quote: String,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiResponse {
    error: Vec<String>,
    result: Option<Value>,
}

pub async fn fetch_asset_pairs(http: &HttpClient) -> Result<Vec<AssetPair>> {
    let url = format!(
        "{}/0/public/AssetPairs",
        http.config().rest_base_url.trim_end_matches('/')
    );
    let response: ApiResponse = http.get_json(&url).await?;
    if !response.error.is_empty() {
        return Err(Error::Api(response.error.join("; ")));
    }
    let result = response
        .result
        .ok_or_else(|| Error::Api("AssetPairs returned empty result".into()))?;
    let obj = result
        .as_object()
        .ok_or_else(|| Error::Api("AssetPairs result is not an object".into()))?;

    let mut pairs = Vec::new();
    for (name, value) in obj {
        let altname = value
            .get("altname")
            .and_then(|v| v.as_str())
            .unwrap_or(name)
            .to_string();
        let wsname = value
            .get("wsname")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let base = value
            .get("base")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let quote = value
            .get("quote")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let status = value
            .get("status")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        pairs.push(AssetPair {
            name: name.clone(),
            altname,
            wsname,
            base,
            quote,
            status,
        });
    }
    pairs.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(pairs)
}
