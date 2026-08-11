use std::collections::HashSet;
use std::sync::Arc;

use chrono::NaiveDate;
use futures::stream::{self, StreamExt};
use tracing::{info, warn};

use crate::client::DeribitHttpClient;
use crate::error::{Error, Result};
use crate::export::{write_trades_parquet, write_volatility_index_parquet};
use crate::instruments::{filter_kind, load_dvol_instruments, load_future_instruments, InstrumentsCache};
use crate::model::{
    currency_from_instrument_name, manifest_key, volatility_manifest_key, DayJob, ManifestEntry,
    MirrorStatus, VolDayJob, S3_DATA_PREFIX, S3_MANIFEST_KEY, S3_VOL_DATA_PREFIX,
    S3_VOL_MANIFEST_KEY,
};
use crate::trades::{expected_dates, fetch_trades_for_day};
use crate::volatility::fetch_volatility_index_for_day;

pub struct S3Config(historical_core::S3Config);

impl std::fmt::Debug for S3Config {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

impl S3Config {
    pub fn from_env() -> Result<Self> {
        Ok(Self(historical_core::S3Config::from_env_keys(
            &[
                "S3_DERIBIT_HISTORICAL_BUCKET",
                "S3_DERIBIT_BUCKET",
                "S3_BUCKET",
            ],
            &["S3_DERIBIT_HISTORICAL_ENDPOINT", "S3_URL"],
            &["S3_DERIBIT_HISTORICAL_REGION", "S3_REGION"],
            &[
                "S3_DERIBIT_HISTORICAL_ACCESS_KEY",
                "S3_DERIBIT_ACCESS_KEY",
                "S3_ACCESS_KEY",
            ],
            &[
                "S3_DERIBIT_HISTORICAL_SECRET_KEY",
                "S3_DERIBIT_SECRET_KEY",
                "S3_SECRET_KEY",
            ],
            "mercat-deribit",
            "fsn1",
        )?))
    }

    pub(crate) fn core(&self) -> &historical_core::S3Config {
        &self.0
    }
}

#[derive(Debug, Clone)]
pub struct MirrorOptions {
    pub currencies: Vec<String>,
    pub instruments: Option<Vec<String>>,
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub resume: bool,
    pub parallel: usize,
    pub include_expired: bool,
}

#[derive(Debug, Clone)]
pub struct VolatilityMirrorOptions {
    pub currencies: Vec<String>,
    pub resolution: String,
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub resume: bool,
    pub parallel: usize,
}

#[derive(Debug, Clone)]
pub struct MirrorRunReport {
    pub status: MirrorStatus,
}

pub async fn run_s3_mirror(
    client: &DeribitHttpClient,
    s3: &S3Config,
    cache: &InstrumentsCache,
    options: MirrorOptions,
) -> Result<MirrorRunReport> {
    if options.to < options.from {
        return Err(Error::Config("--to must be >= --from".into()));
    }

    let instruments = if let Some(list) = &options.instruments {
        list.iter()
            .filter_map(|name| {
                let name = name.trim();
                if name.is_empty() {
                    return None;
                }
                // Prefer explicit --currency when a single value is given (ops case:
                // `--currency USDC --instruments BTC_USDC-PERPETUAL,...`). Otherwise
                // infer settlement from the name (`BTC_USDC-PERPETUAL` → USDC).
                let currency = if options.currencies.len() == 1 {
                    options.currencies[0].clone()
                } else {
                    let inferred = currency_from_instrument_name(name);
                    options
                        .currencies
                        .iter()
                        .find(|c| c.eq_ignore_ascii_case(&inferred))
                        .cloned()
                        .unwrap_or(inferred)
                };
                Some(crate::model::Instrument {
                    instrument_name: name.to_string(),
                    currency,
                    kind: "future".into(),
                    is_active: true,
                    settlement_period: None,
                    creation_timestamp: None,
                    expiration_timestamp: None,
                })
            })
            .collect::<Vec<_>>()
    } else {
        let loaded =
            load_future_instruments(client, cache, &options.currencies, options.include_expired)
                .await?;
        filter_kind(loaded, "future")?
    };

    if instruments.is_empty() {
        return Err(Error::Config("no instruments to mirror".into()));
    }

    let dates: Vec<NaiveDate> = expected_dates(options.from, options.to).collect();
    let jobs: Vec<DayJob> = instruments
        .into_iter()
        .flat_map(|instrument| {
            dates.iter().copied().map(move |date| DayJob {
                currency: instrument.currency.clone(),
                instrument_name: instrument.instrument_name.clone(),
                date,
            })
        })
        .collect();

    let resume_keys = if options.resume {
        load_resume_keys(s3).await
    } else {
        None
    };

    let manifest = Arc::new(historical_core::S3ManifestWriter::<ManifestEntry>::new(
        Arc::new(s3.core().clone()),
        manifest_key(),
    ));

    info!(
        jobs = jobs.len(),
        parallel = options.parallel,
        resume = options.resume,
        "starting Deribit trade S3 mirror"
    );

    let s3_core = Arc::new(s3.core().clone());
    let client = client.clone();
    let resume = options.resume;
    // Keep history-host credit pressure low (plan: 2–4 workers).
    let parallel = options.parallel.clamp(1, 4);

    let statuses = stream::iter(jobs)
        .map(|job| {
            let client = client.clone();
            let s3_core = s3_core.clone();
            let manifest = manifest.clone();
            let resume_keys = resume_keys.clone();
            async move {
                let entry =
                    mirror_one(&client, &s3_core, &job, resume, resume_keys.as_ref()).await;
                let status = entry.status.clone();
                manifest.record(entry).await;
                status
            }
        })
        .buffer_unordered(parallel)
        .collect::<Vec<_>>()
        .await;

    manifest.flush().await?;
    if let Err(error) = manifest.write_compatibility_snapshot().await {
        warn!(
            %error,
            "Deribit compatibility snapshot failed; segment manifests remain authoritative"
        );
    }

    Ok(MirrorRunReport {
        status: MirrorStatus::from_statuses(statuses.iter().map(String::as_str)),
    })
}

async fn load_resume_keys(s3: &S3Config) -> Option<Arc<HashSet<String>>> {
    info!(prefix = S3_DATA_PREFIX, "loading S3 resume keys from data prefix");
    match historical_core::s3_manifest::list_s3_object_keys(s3.core(), S3_DATA_PREFIX).await {
        Ok(keys) => {
            info!(resume_keys = keys.len(), "S3 resume key set ready");
            Some(Arc::new(keys.into_iter().collect()))
        }
        Err(error) => {
            warn!(%error, "unable to list Deribit S3 data keys; using HEAD resume checks");
            None
        }
    }
}

async fn mirror_one(
    client: &DeribitHttpClient,
    s3: &historical_core::S3Config,
    job: &DayJob,
    resume: bool,
    resume_keys: Option<&Arc<HashSet<String>>>,
) -> ManifestEntry {
    let source_url = format!(
        "{}/public/get_last_trades_by_instrument_and_time",
        client.config().history_url.trim_end_matches('/')
    );
    let mut entry = ManifestEntry::pending(job, source_url);
    let key = job.object_key();

    if resume {
        if let Some(keys) = resume_keys {
            if keys.contains(&key) {
                entry.status = "skipped".into();
                return entry;
            }
        } else if let Ok(bucket) = s3.bucket() {
            match bucket.head_object(&key).await {
                Ok((_, code)) if (200..300).contains(&code) => {
                    entry.status = "skipped".into();
                    return entry;
                }
                Ok((_, 404)) => {}
                Ok((_, code)) => warn!(%key, code, "unexpected Deribit resume HEAD status"),
                Err(error) => warn!(%key, %error, "Deribit resume HEAD failed"),
            }
        }
    }

    let trades = match fetch_trades_for_day(
        client,
        &job.instrument_name,
        &job.currency,
        job.date,
    )
    .await
    {
        Ok(trades) => trades,
        Err(error) => {
            entry.status = "failed".into();
            entry.error = Some(error.to_string());
            return entry;
        }
    };

    let output = match tempfile::NamedTempFile::new() {
        Ok(file) => file,
        Err(error) => {
            entry.status = "failed".into();
            entry.error = Some(error.to_string());
            return entry;
        }
    };
    if let Err(error) = write_trades_parquet(&trades, output.path()) {
        entry.status = "failed".into();
        entry.error = Some(error.to_string());
        return entry;
    }

    let bucket = match s3.bucket() {
        Ok(bucket) => bucket,
        Err(error) => {
            entry.status = "failed".into();
            entry.error = Some(error.to_string());
            return entry;
        }
    };
    let mut parquet = match tokio::fs::File::open(output.path()).await {
        Ok(file) => file,
        Err(error) => {
            entry.status = "failed".into();
            entry.error = Some(error.to_string());
            return entry;
        }
    };
    match bucket
        .put_object_stream_with_content_type(&mut parquet, &key, "application/vnd.apache.parquet")
        .await
    {
        Ok(response) if (200..300).contains(&response.status_code()) => {
            entry.status = "ok".into();
            entry.rows = Some(trades.len());
        }
        Ok(response) => {
            entry.status = "failed".into();
            entry.error = Some(format!(
                "S3 upload failed with HTTP {}",
                response.status_code()
            ));
        }
        Err(error) => {
            entry.status = "failed".into();
            entry.error = Some(error.to_string());
        }
    }
    entry
}

pub async fn mirror_status_s3(s3: &S3Config) -> Result<MirrorStatus> {
    Ok(historical_core::s3_manifest::mirror_status_s3(s3.core(), S3_MANIFEST_KEY).await?)
}

/// Mirror DVOL futures public trades (instruments whose name contains `DVOL`).
///
/// Listed under settlement currency `USDC` (e.g. `BTCDVOL_USDC-26AUG26`). There is
/// currently no active ETHDVOL future; empty discovery returns a config error.
pub async fn run_dvol_trades_s3_mirror(
    client: &DeribitHttpClient,
    s3: &S3Config,
    cache: &InstrumentsCache,
    from: NaiveDate,
    to: NaiveDate,
    resume: bool,
    parallel: usize,
    include_expired: bool,
) -> Result<MirrorRunReport> {
    let instruments = load_dvol_instruments(client, cache, include_expired).await?;
    if instruments.is_empty() {
        return Err(Error::Config(
            "no DVOL futures found via get_instruments (currency=USDC, kind=future); \
             volatility index candles still available via mirror volatility-index"
                .into(),
        ));
    }
    let names = instruments
        .iter()
        .map(|instrument| instrument.instrument_name.clone())
        .collect::<Vec<_>>();
    info!(?names, "discovered DVOL trade instruments");
    run_s3_mirror(
        client,
        s3,
        cache,
        MirrorOptions {
            currencies: vec!["USDC".into()],
            instruments: Some(names),
            from,
            to,
            resume,
            parallel,
            include_expired,
        },
    )
    .await
}

pub async fn run_volatility_index_s3_mirror(
    client: &DeribitHttpClient,
    s3: &S3Config,
    options: VolatilityMirrorOptions,
) -> Result<MirrorRunReport> {
    if options.to < options.from {
        return Err(Error::Config("--to must be >= --from".into()));
    }
    if options.currencies.is_empty() {
        return Err(Error::Config("no volatility currencies to mirror".into()));
    }

    let dates: Vec<NaiveDate> = expected_dates(options.from, options.to).collect();
    let resolution = options.resolution.clone();
    let jobs: Vec<VolDayJob> = options
        .currencies
        .iter()
        .flat_map(|currency| {
            let resolution = resolution.clone();
            dates.iter().copied().map(move |date| VolDayJob {
                currency: currency.clone(),
                resolution: resolution.clone(),
                date,
            })
        })
        .collect();

    let resume_keys = if options.resume {
        load_vol_resume_keys(s3).await
    } else {
        None
    };

    let manifest = Arc::new(historical_core::S3ManifestWriter::<ManifestEntry>::new(
        Arc::new(s3.core().clone()),
        volatility_manifest_key(),
    ));

    info!(
        jobs = jobs.len(),
        parallel = options.parallel,
        resume = options.resume,
        resolution = %options.resolution,
        "starting Deribit volatility-index S3 mirror"
    );

    let s3_core = Arc::new(s3.core().clone());
    let client = client.clone();
    let resume = options.resume;
    let parallel = options.parallel.clamp(1, 4);

    let statuses = stream::iter(jobs)
        .map(|job| {
            let client = client.clone();
            let s3_core = s3_core.clone();
            let manifest = manifest.clone();
            let resume_keys = resume_keys.clone();
            async move {
                let entry =
                    mirror_vol_one(&client, &s3_core, &job, resume, resume_keys.as_ref()).await;
                let status = entry.status.clone();
                manifest.record(entry).await;
                status
            }
        })
        .buffer_unordered(parallel)
        .collect::<Vec<_>>()
        .await;

    manifest.flush().await?;
    if let Err(error) = manifest.write_compatibility_snapshot().await {
        warn!(
            %error,
            "Deribit volatility-index compatibility snapshot failed; segment manifests remain authoritative"
        );
    }

    Ok(MirrorRunReport {
        status: MirrorStatus::from_statuses(statuses.iter().map(String::as_str)),
    })
}

async fn load_vol_resume_keys(s3: &S3Config) -> Option<Arc<HashSet<String>>> {
    info!(
        prefix = S3_VOL_DATA_PREFIX,
        "loading S3 resume keys from volatility_index data prefix"
    );
    match historical_core::s3_manifest::list_s3_object_keys(s3.core(), S3_VOL_DATA_PREFIX).await {
        Ok(keys) => {
            info!(resume_keys = keys.len(), "S3 volatility resume key set ready");
            Some(Arc::new(keys.into_iter().collect()))
        }
        Err(error) => {
            warn!(%error, "unable to list Deribit volatility S3 data keys; using HEAD resume checks");
            None
        }
    }
}

async fn mirror_vol_one(
    client: &DeribitHttpClient,
    s3: &historical_core::S3Config,
    job: &VolDayJob,
    resume: bool,
    resume_keys: Option<&Arc<HashSet<String>>>,
) -> ManifestEntry {
    let source_url = format!(
        "{}/public/get_volatility_index_data",
        client.config().auth_url.trim_end_matches('/')
    );
    let mut entry = ManifestEntry::pending_volatility(job, source_url);
    let key = job.object_key();

    if resume {
        if let Some(keys) = resume_keys {
            if keys.contains(&key) {
                entry.status = "skipped".into();
                return entry;
            }
        } else if let Ok(bucket) = s3.bucket() {
            match bucket.head_object(&key).await {
                Ok((_, code)) if (200..300).contains(&code) => {
                    entry.status = "skipped".into();
                    return entry;
                }
                Ok((_, 404)) => {}
                Ok((_, code)) => warn!(%key, code, "unexpected Deribit vol resume HEAD status"),
                Err(error) => warn!(%key, %error, "Deribit vol resume HEAD failed"),
            }
        }
    }

    let candles = match fetch_volatility_index_for_day(
        client,
        &job.currency,
        job.date,
        &job.resolution,
    )
    .await
    {
        Ok(candles) => candles,
        Err(error) => {
            entry.status = "failed".into();
            entry.error = Some(error.to_string());
            return entry;
        }
    };

    let output = match tempfile::NamedTempFile::new() {
        Ok(file) => file,
        Err(error) => {
            entry.status = "failed".into();
            entry.error = Some(error.to_string());
            return entry;
        }
    };
    if let Err(error) = write_volatility_index_parquet(&candles, output.path()) {
        entry.status = "failed".into();
        entry.error = Some(error.to_string());
        return entry;
    }

    let bucket = match s3.bucket() {
        Ok(bucket) => bucket,
        Err(error) => {
            entry.status = "failed".into();
            entry.error = Some(error.to_string());
            return entry;
        }
    };
    let mut parquet = match tokio::fs::File::open(output.path()).await {
        Ok(file) => file,
        Err(error) => {
            entry.status = "failed".into();
            entry.error = Some(error.to_string());
            return entry;
        }
    };
    match bucket
        .put_object_stream_with_content_type(&mut parquet, &key, "application/vnd.apache.parquet")
        .await
    {
        Ok(response) if (200..300).contains(&response.status_code()) => {
            entry.status = "ok".into();
            entry.rows = Some(candles.len());
        }
        Ok(response) => {
            entry.status = "failed".into();
            entry.error = Some(format!(
                "S3 upload failed with HTTP {}",
                response.status_code()
            ));
        }
        Err(error) => {
            entry.status = "failed".into();
            entry.error = Some(error.to_string());
        }
    }
    entry
}

pub async fn mirror_volatility_status_s3(s3: &S3Config) -> Result<MirrorStatus> {
    Ok(
        historical_core::s3_manifest::mirror_status_s3(s3.core(), S3_VOL_MANIFEST_KEY).await?,
    )
}
