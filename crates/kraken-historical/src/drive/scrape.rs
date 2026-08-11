//! Scrape Kraken support pages and public Drive folders (no API key required).

use crate::error::{Error, Result};
use crate::model::DataType;

use super::ids::{quarterly_folder_id, OHLCVT_SUPPORT_URL, TRADES_SUPPORT_URL};

/// Extract Google Drive file IDs from HTML (`/file/d/{id}/` or `id={id}`).
pub fn extract_drive_file_ids(html: &str) -> Vec<String> {
    let mut ids = Vec::new();

    let mut search_from = 0;
    let marker = "/file/d/";
    while let Some(rel) = html[search_from..].find(marker) {
        let start = search_from + rel + marker.len();
        let after = &html[start..];
        let id: String = after
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
            .collect();
        if id.len() >= 20 && !ids.contains(&id) {
            ids.push(id);
        }
        search_from = start + 1;
        if search_from >= html.len() {
            break;
        }
    }

    let mut search_from = 0;
    let marker = "id=";
    while let Some(rel) = html[search_from..].find(marker) {
        let start = search_from + rel + marker.len();
        let after = &html[start..];
        let id: String = after
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
            .collect();
        if id.len() >= 20 && !ids.contains(&id) {
            ids.push(id);
        }
        search_from = start + 1;
        if search_from >= html.len() {
            break;
        }
    }

    ids
}

/// Fetch support pages and return the first Drive file ID found per product.
pub async fn scrape_complete_file_ids(
    client: &reqwest::Client,
) -> Result<(Option<String>, Option<String>)> {
    let trades = scrape_one(client, TRADES_SUPPORT_URL).await?;
    let ohlcvt = scrape_one(client, OHLCVT_SUPPORT_URL).await?;
    Ok((trades, ohlcvt))
}

async fn scrape_one(client: &reqwest::Client, url: &str) -> Result<Option<String>> {
    let response = client.get(url).send().await?;
    if !response.status().is_success() {
        return Err(Error::Drive(format!(
            "support page fetch failed HTTP {} for {url}",
            response.status()
        )));
    }
    let html = response.text().await?;
    Ok(extract_drive_file_ids(&html).into_iter().next())
}

/// Resolve complete file ID: scraped override or hardcoded default.
#[allow(dead_code)]
pub fn resolve_complete_file_id(data_type: DataType, scraped: Option<&str>) -> String {
    if let Some(id) = scraped.filter(|s| !s.is_empty()) {
        return id.to_string();
    }
    match data_type {
        DataType::Trades => super::ids::TRADES_COMPLETE_FILE_ID.to_string(),
        DataType::Ohlcvt => super::ids::OHLCVT_COMPLETE_FILE_ID.to_string(),
    }
}

/// Extract `(file_id, zip_name)` pairs from a public Drive folder HTML page.
pub fn extract_folder_zip_entries(html: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut seen_names = std::collections::HashSet::new();

    for m in html.match_indices("Kraken_") {
        let rest = &html[m.0..];
        let Some(end) = rest.find(".zip") else {
            continue;
        };
        let name = &rest[..end + 4];
        if !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-')
        {
            continue;
        }
        if !seen_names.insert(name.to_string()) {
            continue;
        }
        // Prefer attrs after the name (same element), then a short look-behind.
        let after_start = m.0 + name.len();
        let after_end = (after_start + 240).min(html.len());
        let before_start = m.0.saturating_sub(160);
        if let Some(file_id) = extract_nearby_file_id(&html[after_start..after_end])
            .or_else(|| extract_nearby_file_id(&html[before_start..m.0]))
        {
            out.push((file_id, name.to_string()));
        }
    }
    out
}

fn extract_nearby_file_id(window: &str) -> Option<String> {
    // Prefer ssk (Drive UI) then data-id.
    for marker in ["ssk='", "data-id=\""] {
        let mut search = 0;
        while let Some(rel) = window[search..].find(marker) {
            let start = search + rel + marker.len();
            let after = &window[start..];
            let raw: String = if marker.starts_with("ssk") {
                let id_part = after.split('\'').next().unwrap_or("");
                id_part
                    .rsplit(':')
                    .next()
                    .unwrap_or("")
                    .split('-')
                    .next()
                    .unwrap_or("")
                    .to_string()
            } else {
                after
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
                    .collect()
            };
            if raw.len() >= 25 && raw.len() <= 60 {
                return Some(raw);
            }
            search = start + 1;
        }
    }
    None
}

/// Fetch public Drive folder HTML and return `(file_id, zip_name)` pairs.
pub async fn fetch_folder_zip_entries(
    client: &reqwest::Client,
    data_type: DataType,
) -> Result<Vec<(String, String)>> {
    let folder_id = quarterly_folder_id(data_type);
    let url = format!("https://drive.google.com/drive/folders/{folder_id}?usp=sharing");
    let response = client.get(&url).send().await?;
    if !response.status().is_success() {
        return Err(Error::Drive(format!(
            "Drive folder scrape failed HTTP {} for {url}",
            response.status()
        )));
    }
    let html = response.text().await?;
    let entries = extract_folder_zip_entries(&html);
    if entries.is_empty() {
        return Err(Error::Drive(format!(
            "no quarterly ZIPs found in public folder {folder_id}"
        )));
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_file_d_links() {
        let html = r#"
            <a href="https://drive.google.com/file/d/10zh3tDpqANYvVtYVgczwVz3UZFRUb1el/view">trades</a>
            <a href="https://drive.google.com/file/d/1ptNqWYidLkhb2VAKuLCxmp2OXEfGO-AP/view">ohlc</a>
        "#;
        let ids = extract_drive_file_ids(html);
        assert!(ids.contains(&"10zh3tDpqANYvVtYVgczwVz3UZFRUb1el".to_string()));
        assert!(ids.contains(&"1ptNqWYidLkhb2VAKuLCxmp2OXEfGO-AP".to_string()));
    }

    #[test]
    fn extract_id_query() {
        let html = r#"href="https://drive.google.com/uc?id=10zh3tDpqANYvVtYVgczwVz3UZFRUb1el&export=download""#;
        let ids = extract_drive_file_ids(html);
        assert_eq!(ids[0], "10zh3tDpqANYvVtYVgczwVz3UZFRUb1el");
    }

    #[test]
    fn extract_folder_entries_from_aria_ssk() {
        let html = r#"
            <div class="JxSEve" aria-label="Kraken_Trading_History_Q1_2026.zip Compressed archive Shared"
                 ssk='5:auSv138:1CdhksOTuo9u1MK66VmKxTF41uUwDmt3B-0-16'></div>
            <div aria-label="Kraken_OHLCVT_Q4_2025.zip" data-id="1QbPHLP0TTGo-lqwKn8M-_Xo_oexXlEnB"></div>
        "#;
        let entries = extract_folder_zip_entries(html);
        assert!(entries.iter().any(|(id, name)| {
            id == "1CdhksOTuo9u1MK66VmKxTF41uUwDmt3B"
                && name == "Kraken_Trading_History_Q1_2026.zip"
        }));
        assert!(entries.iter().any(|(id, name)| {
            id == "1QbPHLP0TTGo-lqwKn8M-_Xo_oexXlEnB" && name == "Kraken_OHLCVT_Q4_2025.zip"
        }));
    }
}
