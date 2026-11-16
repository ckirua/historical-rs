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
use super::s3_manifest::new_s3_manifest_writer;
use historical_core::{spawn_progress_reporter, MirrorProgress, ResumeHead};

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

/// Build the resume skip set from objects already under this dataset prefix.
///
/// Listing existing parquet keys is the source of truth for S3 resume. The root
/// JSONL manifest is intentionally not required: loading it (plus every segment)
/// regularly times out on Hetzner and previously forced a per-object HEAD storm.
async fn load_resume_keys(s3: &S3Config, options: &MirrorOptions) -> Option<Arc<HashSet<String>>> {
    let data_prefix = format!(
        "historical/data/{}/{}/{}/",
        options.market.path_segment(),
        options.frequency.as_str(),
        options.data_type.path_segment(),
    );
    info!(%data_prefix, "loading S3 resume keys from data prefix");
    match historical_core::s3_manifest::list_s3_object_keys(s3.core(), &data_prefix).await {
        Ok(keys) => {
            info!(resume_keys = keys.len(), "S3 resume key set ready");
            Some(Arc::new(keys.into_iter().collect()))
        }
        Err(err) => {
            warn!(
                %err,
                %data_prefix,
                "failed to list S3 data keys for resume; falling back to HEAD checks"
            );
            None
        }
    }
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
        load_resume_keys(s3, &options).await
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
        match historical_core::head_object_for_resume(&bucket, &key).await {
            ResumeHead::Exists => {
                entry.status = "skipped".into();
                return entry;
            }
            ResumeHead::Missing => {}
            ResumeHead::Unavailable(message) => {
                entry.status = "failed".into();
                entry.error = Some(format!("resume HEAD unavailable: {message}"));
                return entry;
            }
        }
    }

    match download_and_convert_file(http, file).await {
        Ok((parquet, rows)) => {
            match historical_core::put_file_object_with_retries(
                &bucket,
                &key,
                parquet.path(),
                "application/vnd.apache.parquet",
            )
            .await
            {
                Ok(()) => {
                    entry.status = "ok".into();
                    entry.rows = Some(rows);
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
