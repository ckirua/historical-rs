//! Local disk mirror pipeline.

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use futures::stream::{self, StreamExt};
use tracing::info;

use crate::config::Config;
use crate::drive::{discover_artifacts, download_artifact};
use crate::error::Result;
use crate::model::{DataType, DriveArtifact, MirrorStatus, OutputUnit, Source};
use crate::parse::{list_csv_members, ZipMember};

use super::convert::{convert_output_unit_to_file, output_path_for_unit};
use super::manifest::{append_manifest, entry_from_unit, load_completed_keys, ManifestEntry};

#[derive(Debug, Clone)]
pub struct MirrorOptions {
    pub data_type: DataType,
    pub source_mode: SourceMode,
    pub from_quarter: Option<String>,
    pub to_quarter: Option<String>,
    pub pairs: Option<Vec<String>>,
    pub resume: bool,
    pub parallel: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceMode {
    Complete,
    Quarterly,
    Both,
}

impl MirrorOptions {
    pub fn complete(data_type: DataType) -> Self {
        Self {
            data_type,
            source_mode: SourceMode::Complete,
            from_quarter: None,
            to_quarter: None,
            pairs: None,
            resume: true,
            parallel: crate::config::default_parallel(),
        }
    }
}

pub async fn run_mirror(config: &Config, options: MirrorOptions) -> Result<MirrorStatus> {
    config.ensure_data_dirs()?;
    let artifacts = select_artifacts(config, &options).await?;
    let completed = Arc::new(if options.resume {
        load_completed_keys(config.manifest_path()?)?
    } else {
        HashSet::new()
    });

    let mut status = MirrorStatus::default();
    for artifact in artifacts {
        match mirror_artifact(config, &options, &artifact, completed.clone()).await {
            Ok(artifact_status) => status.merge(artifact_status),
            Err(err) => {
                tracing::warn!(
                    artifact = %artifact.name,
                    %err,
                    "skipping artifact after download/mirror failure"
                );
                let mut entry = entry_from_unit(
                    &OutputUnit::trades(artifact.source.clone(), "_ARTIFACT_"),
                    &artifact.file_id,
                );
                entry.status = "failed".into();
                entry.error = Some(err.to_string());
                if let Err(manifest_err) = append_manifest(config, &entry) {
                    tracing::warn!(%manifest_err, "failed to append artifact failure");
                }
                status.record("failed");
            }
        }
    }

    Ok(status)
}

pub(crate) async fn select_artifacts(
    config: &Config,
    options: &MirrorOptions,
) -> Result<Vec<DriveArtifact>> {
    let discovered = discover_artifacts(config, Some(options.data_type)).await?;
    let mut selected = Vec::new();
    let mut complete = Vec::new();
    for artifact in discovered {
        match (&options.source_mode, &artifact.source) {
            (SourceMode::Complete | SourceMode::Both, Source::Complete) => {
                complete.push(artifact);
            }
            (SourceMode::Quarterly | SourceMode::Both, Source::Quarterly { quarter })
                if quarter_in_range(quarter, &options.from_quarter, &options.to_quarter) =>
            {
                selected.push(artifact);
            }
            _ => {}
        }
    }
    // Prefer quarterly first (anonymous Drive quota often blocks huge complete ZIPs).
    selected.extend(complete);
    Ok(selected)
}

pub(crate) fn quarter_in_range(quarter: &str, from: &Option<String>, to: &Option<String>) -> bool {
    if let Some(from) = from {
        if quarter < from.as_str() {
            return false;
        }
    }
    if let Some(to) = to {
        if quarter > to.as_str() {
            return false;
        }
    }
    true
}

async fn mirror_artifact(
    config: &Config,
    options: &MirrorOptions,
    artifact: &DriveArtifact,
    completed: Arc<HashSet<String>>,
) -> Result<MirrorStatus> {
    let zip_path = download_artifact(config, artifact).await?;
    let members = list_csv_members(&zip_path, artifact.data_type)?;
    let planned = plan_units(artifact, &members, options);

    let total = planned.len();
    eprintln!(
        "local mirror {}:{} planned {total} parquet outputs (parallel={})",
        artifact.data_type.as_str(),
        artifact.source.label(),
        options.parallel
    );
    let progress = historical_core::MirrorProgress::new(total);
    let _reporter = historical_core::spawn_progress_reporter(progress.clone(), "local mirror");

    let config = Arc::new(config.clone());
    let zip_path = Arc::new(zip_path);
    let file_id = Arc::new(artifact.file_id.clone());

    let status = stream::iter(planned)
        .map(|(unit, member)| {
            let config = config.clone();
            let zip_path = zip_path.clone();
            let completed = completed.clone();
            let progress = progress.clone();
            let file_id = file_id.clone();
            async move {
                let config_for_task = config.clone();
                let file_id_for_task = file_id.clone();
                let entry = tokio::task::spawn_blocking(move || {
                    mirror_one(
                        &config_for_task,
                        &zip_path,
                        &unit,
                        &member,
                        &file_id_for_task,
                        &completed,
                    )
                })
                .await
                .unwrap_or_else(|err| {
                    let mut entry =
                        entry_from_unit(&OutputUnit::trades(Source::Complete, "UNKNOWN"), &file_id);
                    entry.status = "failed".into();
                    entry.error = Some(format!("task join error: {err}"));
                    entry
                });
                progress.record(&entry.status);
                if let Err(err) = append_manifest(&config, &entry) {
                    tracing::warn!(%err, "failed to append manifest");
                }
                entry.status
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

    info!(
        artifact = %artifact.name,
        total = status.total,
        "artifact mirror finished"
    );
    Ok(status)
}

pub(crate) fn plan_units(
    artifact: &DriveArtifact,
    members: &[ZipMember],
    options: &MirrorOptions,
) -> Vec<(OutputUnit, ZipMember)> {
    let pair_filter: Option<HashSet<String>> = options.pairs.as_ref().map(|p| {
        p.iter()
            .map(|s| s.to_ascii_uppercase())
            .collect::<HashSet<_>>()
    });

    let mut out = Vec::new();
    for member in members {
        if let Some(filter) = &pair_filter {
            if !filter.contains(&member.pair.to_ascii_uppercase()) {
                continue;
            }
        }
        let unit = match artifact.data_type {
            DataType::Trades => OutputUnit::trades(artifact.source.clone(), member.pair.clone()),
            DataType::Ohlcvt => OutputUnit::ohlcvt(
                artifact.source.clone(),
                member.pair.clone(),
                member.interval.clone().unwrap_or_default(),
            ),
        };
        out.push((unit, member.clone()));
    }
    out
}

fn mirror_one(
    config: &Config,
    zip_path: &Path,
    unit: &OutputUnit,
    member: &ZipMember,
    file_id: &str,
    completed: &HashSet<String>,
) -> ManifestEntry {
    let mut entry = entry_from_unit(unit, file_id);
    let key = entry.key();
    let output = match output_path_for_unit(config, unit) {
        Ok(path) => path,
        Err(err) => {
            entry.status = "failed".into();
            entry.error = Some(err.to_string());
            return entry;
        }
    };

    if completed.contains(&key)
        || (output.exists() && output.metadata().map(|m| m.len() > 0).unwrap_or(false))
    {
        entry.status = "skipped".into();
        entry.path = Some(output.display().to_string());
        return entry;
    }

    let converted = (|| -> Result<(tempfile::NamedTempFile, usize)> {
        let parent = output.parent().ok_or_else(|| {
            crate::error::Error::Io(std::io::Error::other("output path has no parent"))
        })?;
        std::fs::create_dir_all(parent)?;
        let parquet = tempfile::NamedTempFile::new_in(parent)?;
        let rows = convert_output_unit_to_file(zip_path, member, unit, parquet.path())?;
        Ok((parquet, rows))
    })();

    match converted {
        Ok((_parquet, 0)) => {
            entry.status = "skipped".into();
            entry.rows = Some(0);
            entry.error = Some("empty csv".into());
        }
        Ok((parquet, rows)) => match parquet.persist(&output) {
            Ok(_) => {
                entry.status = "ok".into();
                entry.path = Some(output.display().to_string());
                entry.rows = Some(rows);
            }
            Err(err) => {
                entry.status = "failed".into();
                entry.error = Some(err.error.to_string());
            }
        },
        Err(err) => {
            entry.status = "failed".into();
            entry.error = Some(err.to_string());
        }
    }
    entry
}
