//! OKX REST helpers: instruments + market-data-history listing.

use serde::Deserialize;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::model::{DataType, Frequency, Market, OkxFile};

use super::http::OkxHttpClient;

#[derive(Debug, Clone)]
pub struct InstrumentInfo {
    pub market: Market,
    pub inst_id: String,
    pub state: String,
    pub base_ccy: String,
    pub quote_ccy: String,
    pub settle_ccy: String,
}

#[derive(Clone)]
pub struct OkxRestClient {
    http: OkxHttpClient,
}

impl OkxRestClient {
    pub fn new(http: OkxHttpClient) -> Self {
        Self { http }
    }

    pub async fn instruments(&self, market: Market) -> Result<Vec<InstrumentInfo>> {
        let url = format!(
            "{}/api/v5/public/instruments?instType={}",
            self.http.config().rest_base_url.trim_end_matches('/'),
            market.inst_type()
        );
        let payload: OkxResponse<InstrumentRaw> = self.http.get_json(&url).await?;
        ensure_ok(&payload)?;
        Ok(payload
            .data
            .into_iter()
            .map(|s| InstrumentInfo {
                market,
                inst_id: s.inst_id,
                state: s.state,
                base_ccy: s.base_ccy,
                quote_ccy: s.quote_ccy,
                settle_ccy: s.settle_ccy,
            })
            .collect())
    }

    /// List downloadable archives via the public market-data-history API.
    ///
    /// OKX rejects windows longer than ~20 days; callers should chunk.
    pub async fn list_history(
        &self,
        market: Market,
        data_type: DataType,
        symbols: &[String],
        frequency: Frequency,
        begin_ms: i64,
        end_ms: i64,
    ) -> Result<Vec<HistoryFile>> {
        let mut url = url::Url::parse(&format!(
            "{}/api/v5/public/market-data-history",
            self.http.config().rest_base_url.trim_end_matches('/')
        ))?;
        {
            let mut q = url.query_pairs_mut();
            q.append_pair("module", data_type.module());
            q.append_pair("instType", market.inst_type());
            q.append_pair("dateAggrType", frequency.as_str());
            q.append_pair("begin", &begin_ms.to_string());
            q.append_pair("end", &end_ms.to_string());
            match data_type {
                DataType::BorrowRates => {
                    for (i, ccy) in symbols.iter().enumerate() {
                        q.append_pair(&format!("ccyList[{i}]"), ccy);
                    }
                }
                _ => {
                    for (i, inst) in symbols.iter().enumerate() {
                        if inst.eq_ignore_ascii_case("ALL") {
                            continue;
                        }
                        q.append_pair(&format!("instIdList[{i}]"), inst);
                    }
                }
            }
        }

        let payload: OkxResponse<HistoryResult> = self.http.get_json(url.as_str()).await?;
        ensure_ok(&payload)?;

        let mut files = Vec::new();
        for result in payload.data {
            for detail in result.details {
                let symbol = if !detail.inst_id.is_empty() {
                    detail.inst_id
                } else if !detail.ccy.is_empty() {
                    detail.ccy
                } else {
                    "ALL".to_string()
                };
                for group in detail.group_details {
                    files.push(HistoryFile {
                        symbol: symbol.clone(),
                        filename: group.filename,
                        url: strip_query(&group.url),
                        size_mb: group.size_mb.parse().unwrap_or(0.0),
                        date_ts_ms: group.date_ts.parse().unwrap_or(0),
                    });
                }
            }
        }
        Ok(files)
    }
}

#[derive(Debug, Clone)]
pub struct HistoryFile {
    pub symbol: String,
    pub filename: String,
    pub url: String,
    pub size_mb: f64,
    pub date_ts_ms: i64,
}

fn strip_query(url: &str) -> String {
    url.split('?').next().unwrap_or(url).to_string()
}

fn ensure_ok<T>(payload: &OkxResponse<T>) -> Result<()> {
    if payload.code == "0" {
        Ok(())
    } else {
        Err(Error::Api {
            code: payload.code.clone(),
            msg: payload.msg.clone(),
        })
    }
}

#[derive(Debug, Deserialize)]
struct OkxResponse<T> {
    code: String,
    #[serde(default)]
    msg: String,
    #[serde(default = "Vec::new")]
    data: Vec<T>,
}

#[derive(Debug, Deserialize)]
struct InstrumentRaw {
    #[serde(rename = "instId")]
    inst_id: String,
    state: String,
    #[serde(rename = "baseCcy", default)]
    base_ccy: String,
    #[serde(rename = "quoteCcy", default)]
    quote_ccy: String,
    #[serde(rename = "settleCcy", default)]
    settle_ccy: String,
}

#[derive(Debug, Deserialize)]
struct HistoryResult {
    #[serde(default)]
    details: Vec<HistoryDetail>,
}

#[derive(Debug, Deserialize)]
struct HistoryDetail {
    #[serde(rename = "instId", default)]
    inst_id: String,
    #[serde(default)]
    ccy: String,
    #[serde(rename = "groupDetails", default)]
    group_details: Vec<HistoryGroup>,
}

#[derive(Debug, Deserialize)]
struct HistoryGroup {
    #[serde(rename = "dateTs")]
    date_ts: String,
    filename: String,
    #[serde(rename = "sizeMB")]
    size_mb: String,
    url: String,
}

/// Helper kept for callers that need config + rest base.
#[allow(dead_code)]
pub fn rest_base(config: &Config) -> &str {
    config.rest_base_url.trim_end_matches('/')
}

/// Convert a history listing entry into an [`OkxFile`] when possible.
pub fn history_to_file(
    market: Market,
    data_type: DataType,
    frequency: Frequency,
    entry: &HistoryFile,
) -> Option<OkxFile> {
    let period = period_from_filename(&entry.filename)?;
    let symbol = if data_type == DataType::Funding && entry.filename.starts_with("allswap-") {
        "ALL".to_string()
    } else {
        entry.symbol.clone()
    };
    Some(OkxFile {
        market,
        frequency,
        data_type,
        symbol,
        period,
    })
}

fn period_from_filename(name: &str) -> Option<String> {
    // ...-2024-01-01.zip or ...-2024-01.zip
    let stem = name.trim_end_matches(".zip");
    let tokens: Vec<&str> = stem.split('-').collect();
    if tokens.len() >= 3 {
        let y = tokens[tokens.len() - 3];
        let m = tokens[tokens.len() - 2];
        let d = tokens[tokens.len() - 1];
        if y.len() == 4
            && y.chars().all(|c| c.is_ascii_digit())
            && m.len() == 2
            && m.chars().all(|c| c.is_ascii_digit())
            && d.len() == 2
            && d.chars().all(|c| c.is_ascii_digit())
        {
            return Some(format!("{y}-{m}-{d}"));
        }
    }
    if tokens.len() >= 2 {
        let y = tokens[tokens.len() - 2];
        let m = tokens[tokens.len() - 1];
        if y.len() == 4
            && y.chars().all(|c| c.is_ascii_digit())
            && m.len() == 2
            && m.chars().all(|c| c.is_ascii_digit())
        {
            return Some(format!("{y}-{m}"));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn period_from_daily_filename() {
        assert_eq!(
            period_from_filename("BTC-USDT-trades-2024-01-01.zip").as_deref(),
            Some("2024-01-01")
        );
    }

    #[test]
    fn period_from_monthly_filename() {
        assert_eq!(
            period_from_filename("BTC-borrowrates-2024-07.zip").as_deref(),
            Some("2024-07")
        );
    }
}
