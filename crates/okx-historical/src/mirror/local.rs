//! Local disk mirror pipeline.

use std::collections::HashSet;
use std::sync::Arc;

use chrono::{Datelike, NaiveDate};
use futures::stream::{self, StreamExt};
use tracing::{info, warn};

use crate::client::OkxHttpClient;
use crate::config::Config;
use crate::model::{DataType, Frequency, Market, MirrorStatus, OkxFile};

use super::convert::{download_and_convert_file, is_not_found, output_path_for_file};
use super::manifest::{
    append_manifest, entry_from_file, load_completed_keys, manifest_key, ManifestEntry,
};

#[derive(Debug, Clone)]
pub struct MirrorOptions {
    pub market: Market,
    pub frequency: Frequency,
    pub data_type: DataType,
    pub symbols: Vec<String>,
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub resume: bool,
    pub parallel: usize,
}

pub async fn run_mirror(
    http: &OkxHttpClient,
    config: &Config,
    options: MirrorOptions,
) -> crate::error::Result<MirrorStatus> {
    config.ensure_data_dirs()?;
    let total = iter_files(&options).count();
    let files = iter_files(&options);
    let parallel = options.parallel.max(1);
    let completed = Arc::new(if options.resume {
        load_completed_keys(config.manifest_path()?)?
    } else {
        HashSet::new()
    });

    eprintln!(
        "local mirror: planned {total} files (parallel={})",
        options.parallel
    );
    let progress = historical_core::MirrorProgress::new(total);
    let _reporter = historical_core::spawn_progress_reporter(progress.clone(), "local mirror");

    let config = config.clone();
    let http = http.clone();
    let status = stream::iter(files)
        .map(|file| {
            let http = http.clone();
            let config = config.clone();
            let completed = completed.clone();
            let progress = progress.clone();
            async move {
                let entry = mirror_one(&http, &config, &file, &completed).await;
                progress.record(&entry.status);
                entry.status
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
    info!(
        total = status.total,
        done = status.done,
        failed = status.failed,
        skipped = status.skipped,
        "mirror run finished"
    );
    Ok(status)
}

pub fn plan_files(options: &MirrorOptions) -> Vec<OkxFile> {
    iter_files(options).collect()
}

pub(crate) fn iter_files(options: &MirrorOptions) -> impl Iterator<Item = OkxFile> + '_ {
    let mut next = Some(options.from);
    std::iter::from_fn(move || {
        let date = next?;
        if date > options.to {
            return None;
        }
        next = Some(next_date(date, options.frequency));
        Some(date)
    })
    .flat_map(move |date| {
        options.symbols.iter().map(move |symbol| OkxFile {
            market: options.market,
            frequency: options.frequency,
            data_type: options.data_type,
            symbol: symbol.clone(),
            period: match options.frequency {
                Frequency::Daily => date.format("%Y-%m-%d").to_string(),
                Frequency::Monthly => date.format("%Y-%m").to_string(),
            },
        })
    })
}

fn next_date(date: NaiveDate, frequency: Frequency) -> NaiveDate {
    match frequency {
        Frequency::Daily => date + chrono::Duration::days(1),
        Frequency::Monthly => if date.month() == 12 {
            NaiveDate::from_ymd_opt(date.year() + 1, 1, 1)
        } else {
            NaiveDate::from_ymd_opt(date.year(), date.month() + 1, 1)
        }
        .unwrap_or(date + chrono::Duration::days(31)),
    }
}

async fn mirror_one(
    http: &OkxHttpClient,
    config: &Config,
    file: &OkxFile,
    completed: &HashSet<String>,
) -> ManifestEntry {
    let source_url = http.url_builder().build(file);
    let mut entry = entry_from_file(
        file.market,
        file.frequency,
        file.data_type,
        &file.symbol,
        &file.period,
        &source_url,
    );
    let key = manifest_key(
        &entry.market,
        &entry.frequency,
        &entry.data_type,
        &entry.symbol,
        &entry.period,
        &entry.source_url,
    );
    let output = match output_path_for_file(config, file) {
        Ok(path) => path,
        Err(err) => {
            entry.status = "failed".into();
            entry.error = Some(err.to_string());
            let _ = append_manifest(config, &entry);
            return entry;
        }
    };

    if completed.contains(&key)
        || (output.exists() && output.metadata().map(|m| m.len() > 0).unwrap_or(false))
    {
        entry.status = "skipped".into();
        entry.path = Some(output.display().to_string());
        let _ = append_manifest(config, &entry);
        return entry;
    }

    match download_and_convert_file(http, file).await {
        Ok((parquet, rows)) => {
            let persisted = output
                .parent()
                .map(std::fs::create_dir_all)
                .transpose()
                .and_then(|_| {
                    parquet
                        .persist(&output)
                        .map(|_| ())
                        .map_err(|error| error.error)
                });
            match persisted {
                Ok(()) => {
                    entry.status = "ok".into();
                    entry.path = Some(output.display().to_string());
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
    if let Err(err) = append_manifest(config, &entry) {
        warn!(%err, "failed to append manifest");
    }
    entry
}
