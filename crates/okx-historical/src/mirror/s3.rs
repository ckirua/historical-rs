//! Stream OKX downloads directly to S3-compatible object storage.

use std::collections::HashSet;
use std::sync::Arc;

use futures::stream::{self, StreamExt};
use tracing::{info, warn};

use crate::client::OkxHttpClient;
use crate::error::Result;
use crate::export::dataset_object_key;
use crate::model::{MirrorStatus, OkxFile};

use super::convert::{download_and_convert_file, is_not_found};
use super::local::{iter_files, MirrorOptions};
use super::manifest::{entry_from_file, ManifestEntry};
use super::s3_manifest::{new_s3_manifest_writer, S3_MANIFEST_KEY};
use historical_core::{spawn_progress_reporter, MirrorProgress};

#[derive(Clone)]
pub struct S3Config(historical_core::S3Config);

impl std::fmt::Debug for S3Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl S3Config {
    pub fn from_env() -> Result<Self> {
        Ok(Self(historical_core::S3Config::from_env_keys(
            &["S3_OKX_HISTORICAL_BUCKET", "S3_OKX_BUCKET"],
            &["S3_OKX_HISTORICAL_ENDPOINT", "S3_URL"],
            &["S3_OKX_HISTORICAL_REGION"],
            &["S3_OKX_HISTORICAL_ACCESS_KEY", "S3_ACCESS_KEY"],
            &["S3_OKX_HISTORICAL_SECRET_KEY", "S3_SECRET_KEY"],
            "mercat-okx",
            "fsn1",
        )?))
    }

    pub(crate) fn bucket(&self) -> Result<Box<s3::bucket::Bucket>> {
        Ok(self.0.bucket()?)
    }

    pub(crate) fn core(&self) -> &historical_core::S3Config {
        &self.0
    }
}

async fn load_resume_keys(s3: &S3Config) -> Option<Arc<HashSet<String>>> {
    let (manifest_result, objects_result) = tokio::join!(
        historical_core::s3_manifest::load_manifest_index(s3.core(), S3_MANIFEST_KEY),
        historical_core::s3_manifest::list_s3_object_keys(s3.core(), "historical/data/"),
    );

    let manifest_index = match manifest_result {
        Ok(index) => index,
        Err(err) => {
            warn!(%err, "failed to load S3 manifest resume index; falling back to HEAD checks");
            return None;
        }
    };
    let object_keys = match objects_result {
        Ok(keys) => keys,
        Err(err) => {
            warn!(%err, "failed to list S3 data keys for resume; falling back to HEAD checks");
            return None;
        }
    };

    let mut keys: HashSet<String> = object_keys.into_iter().collect();
    for indexed_entry in manifest_index.successful() {
        let entry: ManifestEntry = match serde_json::from_slice(&indexed_entry.serialized) {
            Ok(entry) => entry,
            Err(err) => {
                warn!(%err, "failed to parse S3 manifest resume entry; falling back to HEAD checks");
                return None;
            }
        };
        let successful =
            entry.status == "ok" || (entry.status == "skipped" && entry.error.is_none());
        if successful {
            if let Some(path) = entry.path {
                keys.insert(path);
            }
        }
    }

    Some(Arc::new(keys))
}

pub async fn run_s3_mirror(
    http: &OkxHttpClient,
    s3: &S3Config,
    options: MirrorOptions,
) -> Result<MirrorStatus> {
    let total = iter_files(&options).count();
    let files = iter_files(&options);
    let http = http.clone();
    let resume = options.resume;
    let resume_keys = if resume {
        load_resume_keys(s3).await
    } else {
        None
    };
    let s3 = Arc::new(s3.clone());
    let manifest = Arc::new(new_s3_manifest_writer(Arc::new(s3.core().clone())));
    let parallel = options.parallel.max(1);
    eprintln!(
        "S3 mirror: planned {total} files (parallel={})",
        options.parallel
    );
    let progress = MirrorProgress::new(total);
    let _reporter = spawn_progress_reporter(progress.clone(), "S3 mirror");

    let status = stream::iter(files)
        .map(|file| {
            let http = http.clone();
            let s3 = s3.clone();
            let manifest = manifest.clone();
            let progress = progress.clone();
            let resume_keys = resume_keys.clone();
            async move {
                let entry = upload_file(&http, &s3, &file, resume, resume_keys.as_deref()).await;
                progress.record(&entry.status);
                let entry_status = entry.status.clone();
                manifest.record(entry).await;
                entry_status
            }
        })
        .buffer_unordered(parallel)
        .fold(
            MirrorStatus::default(),
            |mut status, entry_status| async move {
                status.record(&entry_status);
                status
            },
        )
        .await;

    if let Err(err) = manifest.flush().await {
        warn!(%err, "failed to flush final S3 manifest batch");
    }
    if let Err(err) = manifest.write_compatibility_snapshot().await {
        warn!(%err, "failed to write S3 manifest compatibility snapshot");
    }
    info!(
        total = status.total,
        done = status.done,
        failed = status.failed,
        skipped = status.skipped,
        "S3 mirror finished"
    );
    Ok(status)
}

async fn upload_file(
    http: &OkxHttpClient,
    s3: &S3Config,
    file: &OkxFile,
    resume: bool,
    resume_keys: Option<&HashSet<String>>,
) -> ManifestEntry {
    let source_url = http.url_builder().build(file);
    let key = dataset_object_key(file);
    let mut entry = entry_from_file(
        file.market,
        file.frequency,
        file.data_type,
        &file.symbol,
        &file.period,
        &source_url,
    );
    entry.path = Some(key.clone());
    if resume {
        if let Some(keys) = resume_keys {
            if keys.contains(&key) {
                entry.status = "skipped".into();
                return entry;
            }
        }
    }

    let bucket = match s3.bucket() {
        Ok(bucket) => bucket,
        Err(err) => {
            entry.status = "failed".into();
            entry.error = Some(err.to_string());
            return entry;
        }
    };

    if resume && resume_keys.is_none() {
        match bucket.head_object(&key).await {
            Ok((_, code)) if (200..300).contains(&code) => {
                entry.status = "skipped".into();
                return entry;
            }
            Ok((_, 404)) => {}
            Ok((_, code)) => {
                warn!(%key, code, "unexpected HEAD response during resume check");
            }
            Err(err) => {
                let message = err.to_string();
                if !message.contains("404") && !message.contains("Not Found") {
                    warn!(%key, %message, "HEAD failed during resume check");
                }
            }
        }
    }

    match download_and_convert_file(http, file).await {
        Ok((parquet, rows)) => {
            let upload = async {
                let mut input = tokio::fs::File::open(parquet.path()).await?;
                bucket
                    .put_object_stream_with_content_type(
                        &mut input,
                        &key,
                        "application/vnd.apache.parquet",
                    )
                    .await
                    .map_err(|error| crate::error::Error::S3(error.to_string()))
            }
            .await;
            match upload {
                Ok(response) if (200..300).contains(&response.status_code()) => {
                    entry.status = "ok".into();
                    entry.rows = Some(rows);
                }
                Ok(response) => {
                    entry.status = "failed".into();
                    entry.error = Some(format!(
                        "S3 upload failed with HTTP {}",
                        response.status_code()
                    ));
                }
                Err(err) => {
                    entry.status = "failed".into();
                    entry.error = Some(err.to_string());
                }
            }
        }
        Err(err) if is_not_found(&err) => {
            entry.status = "skipped".into();
            entry.error = Some("not found on OKX CDN".into());
        }
        Err(err) => {
            entry.status = "failed".into();
            entry.error = Some(err.to_string());
        }
    }
    entry
}
