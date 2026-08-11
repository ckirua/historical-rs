//! Binance REST exchangeInfo client for symbol discovery.

use serde::Deserialize;

use crate::config::{CM_API_URL, SPOT_API_URL, UM_API_URL};
use crate::error::{Error, Result};
use crate::model::Market;

use super::http::VisionHttpClient;

#[derive(Debug, Clone)]
pub struct ExchangeInfoSymbol {
    pub market: Market,
    pub symbol: String,
    pub status: String,
    pub base_asset: String,
    pub quote_asset: String,
}

#[derive(Clone)]
pub struct BinanceRestClient {
    http: VisionHttpClient,
}

impl BinanceRestClient {
    pub fn new(http: VisionHttpClient) -> Self {
        Self { http }
    }

    pub async fn exchange_info(&self, market: Market) -> Result<Vec<ExchangeInfoSymbol>> {
        let url = match market {
            Market::Spot => SPOT_API_URL,
            Market::Um => UM_API_URL,
            Market::Cm => CM_API_URL,
            Market::Option => {
                return Err(Error::Config(
                    "options exchangeInfo not supported via REST in this client".into(),
                ));
            }
        };
        let bytes = self.http.get_bytes(url).await?;
        let payload: ExchangeInfoResponse = serde_json::from_slice(&bytes)?;
        Ok(payload
            .symbols
            .into_iter()
            .map(|s| ExchangeInfoSymbol {
                market,
                symbol: s.symbol,
                status: s.status,
                base_asset: s.base_asset,
                quote_asset: s.quote_asset,
            })
            .collect())
    }
}

#[derive(Debug, Deserialize)]
struct ExchangeInfoResponse {
    symbols: Vec<ExchangeInfoSymbolRaw>,
}

#[derive(Debug, Deserialize)]
struct ExchangeInfoSymbolRaw {
    symbol: String,
    /// Spot/UM use `status`; CM delivery uses `contractStatus`.
    #[serde(alias = "contractStatus")]
    status: String,
    #[serde(rename = "baseAsset")]
    base_asset: String,
    #[serde(rename = "quoteAsset")]
    quote_asset: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_um_style_status_field() {
        let raw = r#"{
            "symbols": [{
                "symbol": "BTCUSDT",
                "status": "TRADING",
                "baseAsset": "BTC",
                "quoteAsset": "USDT"
            }]
        }"#;
        let payload: ExchangeInfoResponse = serde_json::from_str(raw).unwrap();
        assert_eq!(payload.symbols[0].status, "TRADING");
        assert_eq!(payload.symbols[0].symbol, "BTCUSDT");
    }

    #[test]
    fn parses_cm_contract_status_field() {
        let raw = r#"{
            "symbols": [{
                "symbol": "BTCUSD_PERP",
                "contractStatus": "TRADING",
                "baseAsset": "BTC",
                "quoteAsset": "USD"
            }]
        }"#;
        let payload: ExchangeInfoResponse = serde_json::from_str(raw).unwrap();
        assert_eq!(payload.symbols[0].status, "TRADING");
        assert_eq!(payload.symbols[0].symbol, "BTCUSD_PERP");
    }
}
