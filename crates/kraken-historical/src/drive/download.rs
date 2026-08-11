//! Stream Google Drive files to local scratch (never load multi-GB ZIPs into RAM).

use std::path::{Path, PathBuf};

use futures::StreamExt;
use tokio::io::AsyncWriteExt;
use tracing::{info, warn};

use crate::config::Config;
use crate::error::{Error, Result};
use crate::model::DriveArtifact;

/// Local scratch path for a Drive ZIP.
pub fn zip_scratch_path(config: &Config, artifact: &DriveArtifact) -> Result<PathBuf> {
    let safe_name = artifact
        .name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    let label = artifact.source.label();
    Ok(config
        .zips_dir()?
        .join(artifact.data_type.path_segment())
        .join(format!("{label}_{safe_name}")))
}

/// Download artifact to scratch if missing or empty; return path.
pub async fn download_artifact(config: &Config, artifact: &DriveArtifact) -> Result<PathBuf> {
    config.ensure_data_dirs()?;
    let path = zip_scratch_path(config, artifact)?;
    if path.exists() {
        let meta = std::fs::metadata(&path)?;
        if meta.len() > 0 {
            if let Some(hint) = artifact.size_hint {
                if meta.len() == hint {
                    info!(path = %path.display(), "zip already on disk");
                    return Ok(path);
                }
            } else {
                info!(path = %path.display(), bytes = meta.len(), "zip already on disk");
                return Ok(path);
            }
        }
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Long timeout for multi-GB complete archives; cookie jar for virus-scan confirm.
    let client = reqwest::Client::builder()
        .user_agent(&config.user_agent)
        .timeout(std::time::Duration::from_secs(14_400))
        .cookie_store(true)
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()?;

    if let Some(api_key) = config.drive_api_key() {
        match download_via_api(&client, api_key, &artifact.file_id, &path).await {
            Ok(()) => return Ok(path),
            Err(err) => {
                warn!(%err, "Drive API download failed; falling back to public confirm flow");
            }
        }
    }

    // Retry confirm flow on Drive anonymous quota (often clears after a cool-down).
    let mut last_err = None;
    for attempt in 1..=4u32 {
        match download_via_confirm(&client, &artifact.file_id, &path).await {
            Ok(()) => return Ok(path),
            Err(err) => {
                let msg = err.to_string();
                let quota = msg.to_ascii_lowercase().contains("quota");
                last_err = Some(err);
                if quota && attempt < 4 {
                    let delay = 120u64.saturating_mul(attempt as u64);
                    warn!(
                        attempt,
                        delay_secs = delay,
                        file_id = %artifact.file_id,
                        "Drive quota; backing off before retry"
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
                    continue;
                }
                break;
            }
        }
    }
    Err(last_err.unwrap_or_else(|| Error::Drive("download failed".into())))
}

async fn download_via_api(
    client: &reqwest::Client,
    api_key: &str,
    file_id: &str,
    path: &Path,
) -> Result<()> {
    let url =
        format!("https://www.googleapis.com/drive/v3/files/{file_id}?alt=media&key={api_key}");
    stream_to_file(client, &url, path).await
}

async fn download_via_confirm(client: &reqwest::Client, file_id: &str, path: &Path) -> Result<()> {
    // Classic uc endpoint yields the virus-scan / quota HTML + cookies.
    let initial = format!("https://drive.google.com/uc?export=download&id={file_id}");
    let response = client.get(&initial).send().await?;
    let status = response.status();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    if status.is_success() && !content_type.contains("text/html") {
        return write_response_stream(response, path).await;
    }

    let html = response.text().await?;
    if html_indicates_quota(&html) {
        return Err(Error::Drive(format!(
            "Google Drive anonymous download quota exceeded for file {file_id}; \
             retry later or set KRAKEN_GOOGLE_DRIVE_API_KEY"
        )));
    }

    let (confirm, uuid) = parse_confirm_form(&html).ok_or_else(|| {
        Error::Drive(format!(
            "virus-scan confirm page did not yield tokens for {file_id}"
        ))
    })?;

    let mut url = url::Url::parse("https://drive.usercontent.google.com/download")?;
    {
        let mut q = url.query_pairs_mut();
        q.append_pair("id", file_id);
        q.append_pair("export", "download");
        q.append_pair("confirm", &confirm);
        if let Some(uuid) = uuid {
            q.append_pair("uuid", &uuid);
        }
    }

    match stream_to_file(client, url.as_str(), path).await {
        Ok(()) => Ok(()),
        Err(err) => {
            // Second try: classic uc with confirm=t (gdown-style).
            warn!(%err, "usercontent confirm failed; retrying uc?confirm=");
            let fallback = format!(
                "https://drive.google.com/uc?export=download&confirm={confirm}&id={file_id}"
            );
            stream_to_file(client, &fallback, path).await
        }
    }
}

fn html_indicates_quota(html: &str) -> bool {
    let lower = html.to_ascii_lowercase();
    lower.contains("quota exceeded")
        || lower.contains("you can't download this file right now")
        || lower.contains("can&#39;t download this file right now")
        || lower.contains("can't download this file right now")
}

fn parse_confirm_form(html: &str) -> Option<(String, Option<String>)> {
    let confirm = extract_input_value(html, "confirm")
        .or_else(|| extract_query_param(html, "confirm"))
        .unwrap_or_else(|| "t".to_string());
    let uuid = extract_input_value(html, "uuid").or_else(|| extract_query_param(html, "uuid"));
    Some((confirm, uuid))
}

fn extract_input_value(html: &str, name: &str) -> Option<String> {
    if let Some(pos) = html.find(&format!("name=\"{name}\"")) {
        let slice = &html[pos..pos.saturating_add(200).min(html.len())];
        if let Some(vpos) = slice.find("value=\"") {
            let after = &slice[vpos + 7..];
            let val: String = after.chars().take_while(|c| *c != '"').collect();
            if !val.is_empty() {
                return Some(val);
            }
        }
    }
    if let Some(pos) = html.find(&format!("name='{name}'")) {
        let slice = &html[pos..pos.saturating_add(200).min(html.len())];
        if let Some(vpos) = slice.find("value='") {
            let after = &slice[vpos + 7..];
            let val: String = after.chars().take_while(|c| *c != '\'').collect();
            if !val.is_empty() {
                return Some(val);
            }
        }
    }
    None
}

fn extract_query_param(html: &str, key: &str) -> Option<String> {
    let marker = format!("{key}=");
    let pos = html.find(&marker)?;
    let after = &html[pos + marker.len()..];
    let val: String = after
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    if val.is_empty() {
        None
    } else {
        Some(val)
    }
}

async fn stream_to_file(client: &reqwest::Client, url: &str, path: &Path) -> Result<()> {
    let response = client.get(url).send().await?;
    if response.status().as_u16() == 404 {
        return Err(Error::NotFound(url.to_string()));
    }
    if !response.status().is_success() {
        return Err(Error::Drive(format!(
            "download failed HTTP {} for {url}",
            response.status()
        )));
    }
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    if content_type.contains("text/html") {
        let peek = response.text().await.unwrap_or_default();
        if html_indicates_quota(&peek) {
            return Err(Error::Drive(
                "Google Drive anonymous download quota exceeded".into(),
            ));
        }
        return Err(Error::Drive(
            "download returned HTML (likely virus-scan interstitial)".into(),
        ));
    }
    write_response_stream(response, path).await
}

async fn write_response_stream(response: reqwest::Response, path: &Path) -> Result<()> {
    let tmp = path.with_extension("partial");
    let mut file = tokio::fs::File::create(&tmp).await?;
    let mut stream = response.bytes_stream();
    let mut written: u64 = 0;
    let mut header = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        if header.len() < 4 {
            let need = 4 - header.len();
            header.extend_from_slice(&chunk[..chunk.len().min(need)]);
        }
        file.write_all(&chunk).await?;
        written += chunk.len() as u64;
    }
    file.flush().await?;
    drop(file);
    if written < 1024 {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(Error::Drive(format!(
            "download too small ({written} bytes); likely not a ZIP"
        )));
    }
    if header.len() < 2 || header[0] != 0x50 || header[1] != 0x4b {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(Error::Drive(
            "downloaded file is not a ZIP (missing PK header)".into(),
        ));
    }
    tokio::fs::rename(&tmp, path).await?;
    info!(path = %path.display(), bytes = written, "downloaded zip to scratch");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_confirm_from_html() {
        let html = r#"
        <form id="download-form">
          <input type="hidden" name="id" value="abc">
          <input type="hidden" name="export" value="download">
          <input type="hidden" name="confirm" value="t">
          <input type="hidden" name="uuid" value="123e4567-e89b-12d3-a456-426614174000">
        </form>
        "#;
        let (confirm, uuid) = parse_confirm_form(html).unwrap();
        assert_eq!(confirm, "t");
        assert_eq!(
            uuid.as_deref(),
            Some("123e4567-e89b-12d3-a456-426614174000")
        );
    }

    #[test]
    fn detects_quota_page() {
        assert!(html_indicates_quota(
            "<title>Google Drive - Quota exceeded</title>Sorry, you can&#39;t download"
        ));
    }
}
