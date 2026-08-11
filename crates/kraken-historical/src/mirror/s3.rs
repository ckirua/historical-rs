//! Stream Kraken converts directly to S3-compatible object storage.

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use futures::stream::{self, StreamExt};
use tracing::{info, warn};

use crate::config::Config;
use crate::drive::download_artifact;
use crate::error::Result;
use crate::keys::dataset_object_key;
use crate::model::{DriveArtifact, MirrorStatus, OutputUnit};
use crate::parse::list_csv_members;

use super::convert::convert_output_unit_to_file;
use super::local::{plan_units, select_artifacts, MirrorOptions};
use super::manifest::{entry_from_unit, ManifestEntry};
use super::s3_manifest::{new_s3_manifest_writer, S3ManifestWriter, S3_MANIFEST_KEY};
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
            &["S3_KRAKEN_HISTORICAL_BUCKET", "S3_KRAKEN_BUCKET"],
            &["S3_KRAKEN_HISTORICAL_ENDPOINT", "S3_URL"],
            &["S3_KRAKEN_HISTORICAL_REGION"],
            &["S3_KRAKEN_HISTORICAL_ACCESS_KEY", "S3_ACCESS_KEY"],
            &["S3_KRAKEN_HISTORICAL_SECRET_KEY", "S3_SECRET_KEY"],
            "mercat-kraken",
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

async fn build_resume_index(s3: &historical_core::S3Config) -> Result<HashSet<String>> {
    let manifest_index =
        historical_core::s3_manifest::load_manifest_index(s3, S3_MANIFEST_KEY).await?;
    let object_keys =
        historical_core::s3_manifest::list_s3_object_keys(s3, "historical/data/").await?;
    let mut keys = HashSet::with_capacity(object_keys.len() + manifest_index.entries().len());
    keys.extend(object_keys);
    for indexed in manifest_index.successful() {
        let entry: ManifestEntry = serde_json::from_slice(&indexed.serialized)?;
        if let Some(path) = entry.path {
            keys.insert(path);
        }
    }
    Ok(keys)
}

pub async fn run_s3_mirror(
    config: &Config,
    s3: &S3Config,
    options: MirrorOptions,
) -> Result<MirrorStatus> {
    let s3 = Arc::new(s3.clone());
    let manifest = Arc::new(new_s3_manifest_writer(Arc::new(s3.core().clone())));
    let resume_index = if options.resume {
        match build_resume_index(s3.core()).await {
            Ok(keys) => Some(Arc::new(keys)),
            Err(err) => {
                warn!(%err, "failed to build S3 resume index; falling back to HEAD checks");
                None
            }
        }
    } else {
        None
    };
    let artifacts = select_artifacts(config, &options).await?;
    let mut status = MirrorStatus::default();

    for artifact in artifacts {
        match mirror_artifact_s3(config, &s3, &manifest, &resume_index, &options, &artifact).await {
            Ok(artifact_status) => status.merge(artifact_status),
            Err(err) => {
                warn!(
                    artifact = %artifact.name,
                    source = %artifact.source.label(),
                    %err,
                    "skipping artifact after download/mirror failure"
                );
                let mut entry = entry_from_unit(
                    &OutputUnit::trades(artifact.source.clone(), "_ARTIFACT_"),
                    &artifact.file_id,
                );
                entry.status = "failed".into();
                entry.error = Some(err.to_string());
                manifest.record(entry).await;
                status.record("failed");
            }
        }
    }

    if let Err(err) = manifest.write_compatibility_snapshot().await {
        warn!(%err, "failed to write final S3 manifest compatibility snapshot");
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

async fn mirror_artifact_s3(
    config: &Config,
    s3: &Arc<S3Config>,
    manifest: &Arc<S3ManifestWriter>,
    resume_index: &Option<Arc<HashSet<String>>>,
    options: &MirrorOptions,
    artifact: &DriveArtifact,
) -> Result<MirrorStatus> {
    let zip_path = download_artifact(config, artifact).await?;
    let members = list_csv_members(&zip_path, artifact.data_type)?;
    let planned = plan_units(artifact, &members, options);

    let total = planned.len();
    eprintln!(
        "S3 mirror {}:{} planned {total} parquet outputs (parallel={})",
        artifact.data_type.as_str(),
        artifact.source.label(),
        options.parallel
    );
    let progress = MirrorProgress::new(total);
    let _reporter = spawn_progress_reporter(progress.clone(), "S3 mirror");

    let zip_path = Arc::new(zip_path);
    let file_id = Arc::new(artifact.file_id.clone());
    let resume = options.resume;

    let status = stream::iter(planned)
        .map(|(unit, member)| {
            let s3 = s3.clone();
            let manifest = manifest.clone();
            let resume_index = resume_index.clone();
            let progress = progress.clone();
            let zip_path = zip_path.clone();
            let file_id = file_id.clone();
            async move {
                let entry = upload_one(
                    &s3,
                    &zip_path,
                    &unit,
                    &member,
                    &file_id,
                    resume,
                    resume_index.as_deref(),
                )
                .await;
                progress.record(&entry.status);
                let entry_status = entry.status.clone();
                manifest.record(entry).await;
                entry_status
            }
        })
        .buffer_unordered(options.parallel.max(1))
        .fold(
            MirrorStatus::default(),
            |mut status, entry_status| async move {
                status.record(&entry_status);
                status
            },
        )
        .await;

    Ok(status)
}

async fn upload_one(
    s3: &S3Config,
    zip_path: &Path,
    unit: &OutputUnit,
    member: &crate::parse::ZipMember,
    file_id: &str,
    resume: bool,
    resume_index: Option<&HashSet<String>>,
) -> ManifestEntry {
    let key = dataset_object_key(unit);
    let mut entry = entry_from_unit(unit, file_id);
    entry.path = Some(key.clone());

    if resume {
        if let Some(index) = resume_index {
            if index.contains(&key) {
                entry.status = "skipped".into();
                return entry;
            }
        } else {
            let bucket = match s3.bucket() {
                Ok(bucket) => bucket,
                Err(err) => {
                    entry.status = "failed".into();
                    entry.error = Some(err.to_string());
                    return entry;
                }
            };

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
    }
    let bucket = match s3.bucket() {
        Ok(bucket) => bucket,
        Err(err) => {
            entry.status = "failed".into();
            entry.error = Some(err.to_string());
            return entry;
        }
    };

    let converted = {
        let zip_path = zip_path.to_path_buf();
        let member = member.clone();
        let unit = unit.clone();
        tokio::task::spawn_blocking(move || -> Result<_> {
            let parquet = tempfile::NamedTempFile::new()?;
            let rows = convert_output_unit_to_file(&zip_path, &member, &unit, parquet.path())?;
            Ok((parquet, rows))
        })
        .await
    };

    match converted {
        Ok(Ok((_parquet, 0))) => {
            entry.status = "skipped".into();
            entry.rows = Some(0);
            entry.error = Some("empty csv".into());
        }
        Ok(Ok((parquet, rows))) => {
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
        Ok(Err(err)) => {
            entry.status = "failed".into();
            entry.error = Some(err.to_string());
        }
        Err(err) => {
            entry.status = "failed".into();
            entry.error = Some(format!("task join error: {err}"));
        }
    }
    entry
}
