//! List quarterly ZIP artifacts via Google Drive API.

use serde::Deserialize;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::model::{DataType, DriveArtifact, Source};

use super::ids::{complete_artifact, quarterly_folder_id};

#[derive(Debug, Deserialize)]
struct DriveListResponse {
    files: Option<Vec<DriveFileMeta>>,
    #[serde(rename = "nextPageToken")]
    next_page_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DriveFileMeta {
    id: String,
    name: String,
    size: Option<String>,
}

/// Parse quarter label from Kraken ZIP filenames.
///
/// Accepts:
/// - `Kraken_Trades_Q3_2024.zip` → `2024Q3`
/// - `Kraken_Trading_History_Q1_2023.zip` → `2023Q1`
/// - `Kraken_OHLCVT_Q4_2022.zip` → `2022Q4`
pub fn parse_quarter_filename(name: &str) -> Option<(DataType, String)> {
    let lower = name.to_ascii_lowercase();
    if !lower.ends_with(".zip") {
        return None;
    }
    let stem = &name[..name.len().saturating_sub(4)];

    let data_type = if stem.contains("OHLCVT") || lower.contains("ohlcvt") {
        DataType::Ohlcvt
    } else if lower.contains("trades") || lower.contains("trading") || lower.contains("trade") {
        DataType::Trades
    } else {
        return None;
    };

    // Look for Q{n}_{yyyy} anywhere in the stem
    let bytes = stem.as_bytes();
    for i in 0..bytes.len().saturating_sub(6) {
        if (bytes[i] == b'Q' || bytes[i] == b'q')
            && bytes[i + 1].is_ascii_digit()
            && bytes[i + 2] == b'_'
            && bytes[i + 3].is_ascii_digit()
            && bytes[i + 4].is_ascii_digit()
            && bytes[i + 5].is_ascii_digit()
            && bytes[i + 6].is_ascii_digit()
        {
            let qn = bytes[i + 1] - b'0';
            if (1..=4).contains(&qn) {
                let year = &stem[i + 3..i + 7];
                return Some((data_type, format!("{year}Q{qn}")));
            }
        }
    }
    None
}

pub async fn list_quarterly_artifacts(
    config: &Config,
    data_type: DataType,
) -> Result<Vec<DriveArtifact>> {
    let client = reqwest::Client::builder()
        .user_agent(&config.user_agent)
        .timeout(config.timeout())
        .build()?;

    // Default: scrape public folder HTML (no API key).
    match super::scrape::fetch_folder_zip_entries(&client, data_type).await {
        Ok(entries) => {
            let mut artifacts = Vec::new();
            for (file_id, name) in entries {
                let Some((dt, quarter)) = parse_quarter_filename(&name) else {
                    continue;
                };
                if dt != data_type {
                    continue;
                }
                artifacts.push(DriveArtifact {
                    data_type,
                    source: Source::Quarterly { quarter },
                    file_id,
                    name,
                    size_hint: None,
                });
            }
            if !artifacts.is_empty() {
                artifacts.sort_by_key(|a| a.source.label());
                return Ok(artifacts);
            }
        }
        Err(err) => {
            tracing::warn!(
                data_type = data_type.as_str(),
                %err,
                "public Drive folder scrape failed; trying API key if present"
            );
        }
    }

    // Optional: Drive API when KRAKEN_GOOGLE_DRIVE_API_KEY is set.
    let Some(api_key) = config.drive_api_key() else {
        return Err(Error::Config(
            "quarterly discovery failed via public folder scrape; set KRAKEN_GOOGLE_DRIVE_API_KEY as fallback".into(),
        ));
    };
    list_quarterly_via_api(&client, api_key, data_type).await
}

async fn list_quarterly_via_api(
    client: &reqwest::Client,
    api_key: &str,
    data_type: DataType,
) -> Result<Vec<DriveArtifact>> {
    let folder_id = quarterly_folder_id(data_type);
    let mut artifacts = Vec::new();
    let mut page_token: Option<String> = None;
    loop {
        let mut url = url::Url::parse("https://www.googleapis.com/drive/v3/files")?;
        {
            let mut q = url.query_pairs_mut();
            q.append_pair("q", &format!("'{folder_id}' in parents and trashed=false"));
            q.append_pair("fields", "nextPageToken,files(id,name,size)");
            q.append_pair("pageSize", "1000");
            q.append_pair("key", api_key);
            if let Some(token) = &page_token {
                q.append_pair("pageToken", token);
            }
        }

        let response = client.get(url).send().await?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::Drive(format!(
                "Drive list failed HTTP {status}: {body}"
            )));
        }
        let body: DriveListResponse = response.json().await?;
        for file in body.files.unwrap_or_default() {
            if let Some((dt, quarter)) = parse_quarter_filename(&file.name) {
                if dt != data_type {
                    continue;
                }
                let size_hint = file.size.and_then(|s| s.parse().ok());
                artifacts.push(DriveArtifact {
                    data_type,
                    source: Source::Quarterly { quarter },
                    file_id: file.id,
                    name: file.name,
                    size_hint,
                });
            }
        }
        page_token = body.next_page_token;
        if page_token.is_none() {
            break;
        }
    }

    artifacts.sort_by_key(|a| a.source.label());
    Ok(artifacts)
}

pub async fn discover_artifacts(
    config: &Config,
    data_type: Option<DataType>,
) -> Result<Vec<DriveArtifact>> {
    let types: Vec<DataType> = match data_type {
        Some(dt) => vec![dt],
        None => vec![DataType::Trades, DataType::Ohlcvt],
    };

    let mut out = Vec::new();
    for dt in types {
        out.push(complete_artifact(dt));
        match list_quarterly_artifacts(config, dt).await {
            Ok(mut quarters) => out.append(&mut quarters),
            Err(err) => {
                tracing::warn!(
                    data_type = dt.as_str(),
                    %err,
                    "quarterly discovery skipped (public scrape/API failed)"
                );
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_trades_quarter_filenames() {
        assert_eq!(
            parse_quarter_filename("Kraken_Trades_Q3_2024.zip"),
            Some((DataType::Trades, "2024Q3".into()))
        );
        assert_eq!(
            parse_quarter_filename("Kraken_Trading_History_Q1_2023.zip"),
            Some((DataType::Trades, "2023Q1".into()))
        );
    }

    #[test]
    fn parse_ohlcvt_quarter_filename() {
        assert_eq!(
            parse_quarter_filename("Kraken_OHLCVT_Q4_2022.zip"),
            Some((DataType::Ohlcvt, "2022Q4".into()))
        );
    }

    #[test]
    fn reject_non_quarter() {
        assert!(parse_quarter_filename("Kraken_Trading_History.zip").is_none());
        assert!(parse_quarter_filename("readme.txt").is_none());
    }
}
