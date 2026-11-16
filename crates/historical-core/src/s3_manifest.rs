//! Parameterized S3 JSONL manifest writer.

use s3::bucket::Bucket;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;
use tracing::warn;

use crate::error::{Error, Result};
use crate::s3::S3Config;
use crate::status::{mirror_status_from_jsonl, MirrorStatus};

const FLUSH_BATCH: usize = 32;

/// Hard ceiling for merging root + segments into the compatibility snapshot.
///
/// rust-s3's tokio backend only applies connect timeouts, so a flaky Hetzner
/// endpoint (504 / stalled body / EOF) can otherwise block mirror exit for hours.
const COMPATIBILITY_SNAPSHOT_DEADLINE: Duration = Duration::from_secs(10 * 60);

/// Per GET / PUT / LIST attempt budget.
const S3_OP_TIMEOUT: Duration = Duration::from_secs(30);

/// Bounded retries for transient S3 failures (504, timeout, EOF, …).
const S3_OP_MAX_ATTEMPTS: u32 = 3;

/// One JSONL entry retained by [`ManifestIndex`].
///
/// The complete serialized entry is retained so callers can inspect their own
/// fields without this crate knowing an exchange-specific manifest schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestIndexEntry {
    pub status: String,
    pub serialized: Vec<u8>,
}

impl ManifestIndexEntry {
    /// Whether this entry records an object that can be treated as complete.
    ///
    /// Failed, source-not-found, and pending entries deliberately return false
    /// and remain available to retrying workers.
    pub fn is_successful(&self) -> bool {
        matches!(self.status.as_str(), "ok" | "skipped")
    }
}

/// Lightweight status index built from the merged root and immutable segments.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ManifestIndex {
    entries: Vec<ManifestIndexEntry>,
}

impl ManifestIndex {
    /// Parse JSONL while retaining only each line and its status field.
    pub fn from_jsonl(bytes: &[u8]) -> Result<Self> {
        let mut entries = Vec::new();
        for line in jsonl_lines(bytes) {
            let parsed: StatusLine = serde_json::from_slice(line)?;
            entries.push(ManifestIndexEntry {
                status: parsed.status,
                serialized: line.to_vec(),
            });
        }
        Ok(Self { entries })
    }

    pub fn entries(&self) -> &[ManifestIndexEntry] {
        &self.entries
    }

    pub fn successful(&self) -> impl Iterator<Item = &ManifestIndexEntry> {
        self.entries.iter().filter(|entry| entry.is_successful())
    }

    pub fn retryable(&self) -> impl Iterator<Item = &ManifestIndexEntry> {
        self.entries.iter().filter(|entry| !entry.is_successful())
    }
}

#[derive(Debug, Deserialize)]
struct StatusLine {
    status: String,
}

/// Batched append writer for a fixed S3 manifest object key.
pub struct S3ManifestWriter<E> {
    config: Arc<S3Config>,
    manifest_key: String,
    writer_id: String,
    next_segment: AtomicU64,
    buffer: Mutex<Vec<E>>,
    flush_lock: Mutex<()>,
}

impl<E: Serialize> S3ManifestWriter<E> {
    pub fn new(config: Arc<S3Config>, manifest_key: impl Into<String>) -> Self {
        Self {
            config,
            manifest_key: manifest_key.into(),
            writer_id: new_writer_id(),
            next_segment: AtomicU64::new(0),
            buffer: Mutex::new(Vec::new()),
            flush_lock: Mutex::new(()),
        }
    }

    pub fn manifest_key(&self) -> &str {
        &self.manifest_key
    }

    pub async fn record(&self, entry: E) {
        let should_flush = {
            let mut buffer = self.buffer.lock().await;
            buffer.push(entry);
            buffer.len() >= FLUSH_BATCH
        };
        if should_flush {
            if let Err(err) = self.flush().await {
                warn!(%err, "failed to flush S3 manifest batch");
            }
        }
    }

    /// Append the buffered entries as one immutable segment.
    ///
    /// Normal flushes intentionally do not read or rewrite the root object.
    /// This keeps each batch O(batch size) and lets concurrent writers append
    /// under independent writer/sequence keys.
    pub async fn flush(&self) -> Result<()> {
        let _flush_guard = self.flush_lock.lock().await;
        let entries = {
            let mut buffer = self.buffer.lock().await;
            if buffer.is_empty() {
                return Ok(());
            }
            std::mem::take(&mut *buffer)
        };

        let batch = match entries_to_jsonl(&entries) {
            Ok(batch) => batch,
            Err(err) => {
                self.requeue(entries).await;
                return Err(err);
            }
        };
        let bucket = match self.config.bucket() {
            Ok(bucket) => bucket,
            Err(err) => {
                self.requeue(entries).await;
                return Err(err);
            }
        };
        let sequence = self.next_segment.fetch_add(1, Ordering::Relaxed);
        let segment_key = format!(
            "{}.segments/{}/{sequence:020}.jsonl",
            self.manifest_key, self.writer_id
        );
        if let Err(err) = put_manifest(&bucket, &segment_key, &batch).await {
            self.requeue(entries).await;
            return Err(err);
        }
        Ok(())
    }

    /// Write the merged state to the fixed root key for legacy readers.
    ///
    /// This is explicit because it performs a full read and rewrite. Existing
    /// root and segment objects remain readable whether or not this is called.
    ///
    /// Fail-open: on overall deadline or transient S3 errors (504 / timeout /
    /// EOF), logs a WARN and returns `Ok(())` so mirrors can exit. Immutable
    /// segment objects remain authoritative for resume.
    pub async fn write_compatibility_snapshot(&self) -> Result<()> {
        self.flush().await?;
        let _flush_guard = self.flush_lock.lock().await;
        match tokio::time::timeout(
            COMPATIBILITY_SNAPSHOT_DEADLINE,
            self.write_compatibility_snapshot_inner(),
        )
        .await
        {
            Ok(Ok(())) => Ok(()),
            Ok(Err(err)) if is_transient_s3_error(&err) => {
                warn!(
                    %err,
                    key = %self.manifest_key,
                    "compatibility snapshot failed (transient); segment manifests remain authoritative"
                );
                Ok(())
            }
            Ok(Err(err)) => Err(err),
            Err(_elapsed) => {
                warn!(
                    key = %self.manifest_key,
                    deadline_secs = COMPATIBILITY_SNAPSHOT_DEADLINE.as_secs(),
                    "compatibility snapshot exceeded deadline; segment manifests remain authoritative"
                );
                Ok(())
            }
        }
    }

    async fn write_compatibility_snapshot_inner(&self) -> Result<()> {
        let bucket = self.config.bucket()?;
        let body = fetch_complete_manifest_bytes(&bucket, &self.manifest_key).await?;
        put_manifest(&bucket, &self.manifest_key, &body).await
    }

    async fn requeue(&self, mut entries: Vec<E>) {
        let mut buffer = self.buffer.lock().await;
        entries.append(&mut *buffer);
        *buffer = entries;
    }
}

/// Load the merged root and immutable segment JSONL bytes.
///
/// The fixed root object and every `.segments/*.jsonl` object are read. The
/// returned bytes preserve entry multiplicity while removing only entries
/// already represented in the root compatibility snapshot.
pub async fn load_manifest_bytes(s3: &S3Config, manifest_key: &str) -> Result<Vec<u8>> {
    let bucket = s3.bucket()?;
    fetch_complete_manifest_bytes(&bucket, manifest_key).await
}

/// Load a lightweight status index from the merged manifest.
///
/// Each entry retains its serialized JSON and status, allowing exchange
/// workers to inspect their own fields. Only `ok` and `skipped` entries are
/// successful; failed/source-not-found entries are returned by `retryable` and
/// are never treated as complete.
pub async fn load_manifest_index(s3: &S3Config, manifest_key: &str) -> Result<ManifestIndex> {
    let bytes = load_manifest_bytes(s3, manifest_key).await?;
    ManifestIndex::from_jsonl(&bytes)
}

/// List object keys under a bounded prefix using rust-s3's paginated `list`.
///
/// Returns every object key under the prefix (parquet, JSONL, and any other
/// suffix). Callers should use a bounded, exchange-specific prefix so the key
/// set remains memory-bounded for the resume operation.
pub async fn list_s3_object_keys(s3: &S3Config, prefix: &str) -> Result<Vec<String>> {
    let bucket = s3.bucket()?;
    list_object_keys(&bucket, prefix).await
}

/// Load and count an S3 JSONL manifest.
pub async fn mirror_status_s3(s3: &S3Config, manifest_key: &str) -> Result<MirrorStatus> {
    let bytes = load_manifest_bytes(s3, manifest_key).await?;
    if bytes.is_empty() {
        return Ok(MirrorStatus::default());
    }
    let content = String::from_utf8(bytes)
        .map_err(|e| Error::S3(format!("manifest is not valid UTF-8: {e}")))?;
    mirror_status_from_jsonl(&content)
}

fn entries_to_jsonl<E: Serialize>(entries: &[E]) -> Result<Vec<u8>> {
    let mut body = Vec::new();
    for entry in entries {
        serde_json::to_writer(&mut body, entry)?;
        body.push(b'\n');
    }
    Ok(body)
}

fn new_writer_id() -> String {
    static NEXT_WRITER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let sequence = NEXT_WRITER.fetch_add(1, Ordering::Relaxed);
    format!("{nanos:032x}-{:08x}-{sequence:016x}", std::process::id())
}

async fn fetch_complete_manifest_bytes(bucket: &Bucket, key: &str) -> Result<Vec<u8>> {
    let root = fetch_manifest_bytes(bucket, key).await?;
    let segments = fetch_manifest_segments(bucket, key).await?;
    Ok(merge_manifest_sources(root, &segments))
}

async fn fetch_manifest_segments(bucket: &Bucket, key: &str) -> Result<Vec<Vec<u8>>> {
    let prefix = format!("{key}.segments/");
    let keys = list_object_keys(bucket, &prefix)
        .await?
        .into_iter()
        .filter(|object_key| object_key.ends_with(".jsonl"))
        .collect::<Vec<_>>();

    let mut segments = Vec::with_capacity(keys.len());
    for segment_key in keys {
        segments.push(fetch_manifest_bytes(bucket, &segment_key).await?);
    }
    Ok(segments)
}

async fn list_object_keys(bucket: &Bucket, prefix: &str) -> Result<Vec<String>> {
    with_s3_retries("LIST", prefix, || async {
        let pages = bucket
            .list(prefix.to_string(), None)
            .await
            .map_err(|err| Error::S3(err.to_string()))?;
        let mut keys = pages
            .into_iter()
            .flat_map(|page| page.contents)
            .map(|object| object.key)
            .collect::<Vec<_>>();
        keys.sort();
        Ok(keys)
    })
    .await
}

fn merge_manifest_sources(mut root: Vec<u8>, segments: &[Vec<u8>]) -> Vec<u8> {
    let mut root_lines = HashMap::<Vec<u8>, usize>::new();
    for line in jsonl_lines(&root) {
        *root_lines.entry(line.to_vec()).or_default() += 1;
    }

    for segment in segments {
        for line in jsonl_lines(segment) {
            if let Some(remaining) = root_lines.get_mut(line) {
                if *remaining > 0 {
                    *remaining -= 1;
                    continue;
                }
            }
            if !root.is_empty() && !root.ends_with(b"\n") {
                root.push(b'\n');
            }
            root.extend_from_slice(line);
            root.push(b'\n');
        }
    }
    root
}

fn jsonl_lines(bytes: &[u8]) -> impl Iterator<Item = &[u8]> {
    bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.iter().all(u8::is_ascii_whitespace))
}

async fn fetch_manifest_bytes(bucket: &Bucket, key: &str) -> Result<Vec<u8>> {
    with_s3_retries("GET", key, || async {
        match bucket.get_object(key).await {
            Ok(response) if (200..300).contains(&response.status_code()) => Ok(response.to_vec()),
            Ok(response) if response.status_code() == 404 => Ok(Vec::new()),
            Ok(response) => Err(Error::S3(format!(
                "manifest GET returned HTTP {}",
                response.status_code()
            ))),
            Err(err) => {
                let message = err.to_string();
                if message.contains("404") || message.contains("Not Found") {
                    Ok(Vec::new())
                } else {
                    Err(Error::S3(message))
                }
            }
        }
    })
    .await
}

async fn put_manifest(bucket: &Bucket, key: &str, body: &[u8]) -> Result<()> {
    with_s3_retries("PUT", key, || async {
        let response = bucket
            .put_object_with_content_type(key, body, "application/x-ndjson")
            .await
            .map_err(|e| Error::S3(e.to_string()))?;
        if (200..300).contains(&response.status_code()) {
            Ok(())
        } else {
            Err(Error::S3(format!(
                "manifest PUT returned HTTP {}",
                response.status_code()
            )))
        }
    })
    .await
}

/// Whether an HTTP status looks like a transient S3 / gateway failure.
pub fn is_transient_s3_status(code: u16) -> bool {
    matches!(code, 500 | 502 | 503 | 504)
}

/// Whether an S3 error string looks like a transient gateway / transport failure.
pub fn is_transient_s3_message(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("504")
        || lower.contains("503")
        || lower.contains("502")
        || lower.contains("500")
        || lower.contains("timeout")
        || lower.contains("timed out")
        || lower.contains("eof")
        || lower.contains("connection reset")
        || lower.contains("broken pipe")
        || lower.contains("connection refused")
        || lower.contains("temporarily unavailable")
}

fn is_transient_s3_error(err: &Error) -> bool {
    match err {
        Error::S3(message) => is_transient_s3_message(message),
        _ => false,
    }
}

/// Outcome of a resume-time object existence check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResumeHead {
    /// Object is present (HTTP 2xx).
    Exists,
    /// Object is absent (HTTP 404 / Not Found).
    Missing,
    /// HEAD kept failing with transient gateway/transport errors after retries.
    ///
    /// Callers must **not** treat this as missing: re-downloading and re-uploading
    /// during an S3 outage amplifies load and produces large false `failed` counts.
    Unavailable(String),
}

/// HEAD an object for `--resume`, retrying transient 5xx / transport failures.
///
/// On exhausted transient failures returns [`ResumeHead::Unavailable`] so the
/// mirror can mark the entry failed and retry on a later run instead of
/// hammering CDN + S3 uploads.
pub async fn head_object_for_resume(bucket: &Bucket, key: &str) -> ResumeHead {
    match with_s3_retries("HEAD", key, || async {
        match bucket.head_object(key).await {
            Ok((_, code)) if (200..300).contains(&code) => Ok(ResumeHead::Exists),
            Ok((_, 404)) => Ok(ResumeHead::Missing),
            Ok((_, code)) if is_transient_s3_status(code) => {
                Err(Error::S3(format!("HEAD returned HTTP {code}")))
            }
            Ok((_, code)) => {
                warn!(%key, code, "unexpected HEAD response during resume check");
                Ok(ResumeHead::Missing)
            }
            Err(err) => {
                let message = err.to_string();
                if message.contains("404") || message.contains("Not Found") {
                    Ok(ResumeHead::Missing)
                } else if is_transient_s3_message(&message) {
                    Err(Error::S3(message))
                } else {
                    warn!(%key, %message, "HEAD failed during resume check");
                    Ok(ResumeHead::Missing)
                }
            }
        }
    })
    .await
    {
        Ok(result) => result,
        Err(err) => ResumeHead::Unavailable(err.to_string()),
    }
}

/// Stream a local file to S3 with the same transient retry policy as manifest ops.
pub async fn put_file_object_with_retries(
    bucket: &Bucket,
    key: &str,
    path: &Path,
    content_type: &str,
) -> Result<()> {
    with_s3_retries("PUT", key, || async {
        let mut input = tokio::fs::File::open(path).await?;
        let response = bucket
            .put_object_stream_with_content_type(&mut input, key, content_type)
            .await
            .map_err(|error| Error::S3(error.to_string()))?;
        let code = response.status_code();
        if (200..300).contains(&code) {
            Ok(())
        } else {
            Err(Error::S3(format!("S3 upload failed with HTTP {code}")))
        }
    })
    .await
}

async fn with_s3_retries<T, F, Fut>(op: &str, key: &str, mut attempt_fn: F) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut attempt = 0u32;
    loop {
        attempt += 1;
        match tokio::time::timeout(S3_OP_TIMEOUT, attempt_fn()).await {
            Ok(Ok(value)) => return Ok(value),
            Ok(Err(err))
                if attempt < S3_OP_MAX_ATTEMPTS && is_transient_s3_error(&err) =>
            {
                warn!(%err, op, key, attempt, "retrying transient S3 error");
                tokio::time::sleep(Duration::from_millis(250 * u64::from(attempt))).await;
            }
            Ok(Err(err)) => return Err(err),
            Err(_elapsed) if attempt < S3_OP_MAX_ATTEMPTS => {
                warn!(op, key, attempt, "S3 op timed out; retrying");
                tokio::time::sleep(Duration::from_millis(250 * u64::from(attempt))).await;
            }
            Err(_elapsed) => {
                return Err(Error::S3(format!(
                    "{op} {key} timed out after {S3_OP_MAX_ATTEMPTS} attempts ({}s each)",
                    S3_OP_TIMEOUT.as_secs()
                )));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovers_entries_lost_by_racing_root_rewrites() {
        let first = br#"{"status":"ok","path":"first"}"#;
        let second = br#"{"status":"failed","path":"second"}"#;
        let root = [first.as_slice(), b"\n"].concat();
        let segments = vec![
            [first.as_slice(), b"\n"].concat(),
            [second.as_slice(), b"\n"].concat(),
        ];

        let merged = merge_manifest_sources(root, &segments);
        let status = mirror_status_from_jsonl(std::str::from_utf8(&merged).unwrap()).unwrap();
        assert_eq!(status.total, 2);
        assert_eq!(status.done, 1);
        assert_eq!(status.failed, 1);
    }

    #[test]
    fn preserves_duplicate_entries_from_distinct_segments() {
        let entry = b"{\"status\":\"skipped\",\"path\":\"same\"}\n";
        let segments = vec![entry.to_vec(), entry.to_vec()];

        let merged = merge_manifest_sources(Vec::new(), &segments);
        let status = mirror_status_from_jsonl(std::str::from_utf8(&merged).unwrap()).unwrap();
        assert_eq!(status.total, 2);
        assert_eq!(status.skipped, 2);
    }
    #[test]
    fn index_keeps_successful_and_retryable_entries_distinct() {
        let bytes = br#"{"status":"ok","path":"done"}
{"status":"skipped","path":"existing"}
{"status":"failed","path":"retry"}
{"status":"source-not-found","path":"retry-too"}"#;
        let index = ManifestIndex::from_jsonl(bytes).unwrap();
        assert_eq!(index.entries().len(), 4);
        assert_eq!(index.successful().count(), 2);
        assert_eq!(index.retryable().count(), 2);
        assert_eq!(
            index.entries()[2].serialized,
            br#"{"status":"failed","path":"retry"}"#
        );
    }

    #[test]
    fn malformed_index_is_rejected() {
        let error = ManifestIndex::from_jsonl(br#"{"status":"ok"} trailing"#).unwrap_err();
        assert!(error.to_string().contains("JSON error"));
    }

    #[test]
    fn classifies_hetzner_gateway_and_transport_failures_as_transient() {
        assert!(is_transient_s3_status(500));
        assert!(is_transient_s3_status(502));
        assert!(is_transient_s3_status(503));
        assert!(is_transient_s3_status(504));
        assert!(!is_transient_s3_status(404));
        assert!(!is_transient_s3_status(403));
        assert!(is_transient_s3_message("manifest GET returned HTTP 504"));
        assert!(is_transient_s3_message("error decoding response body: unexpected EOF"));
        assert!(is_transient_s3_message("GET key timed out after 3 attempts (30s each)"));
        assert!(is_transient_s3_message("connection reset by peer"));
        assert!(is_transient_s3_message("HEAD returned HTTP 503"));
        assert!(is_transient_s3_error(&Error::S3(
            "manifest PUT returned HTTP 503".into()
        )));
        assert!(!is_transient_s3_message("manifest GET returned HTTP 403"));
        assert!(!is_transient_s3_message("manifest GET returned HTTP 404"));
        assert!(!is_transient_s3_error(&Error::Config("missing".into())));
    }

    #[test]
    fn snapshot_deadline_and_op_timeout_are_bounded() {
        assert!(COMPATIBILITY_SNAPSHOT_DEADLINE <= Duration::from_secs(15 * 60));
        assert!(COMPATIBILITY_SNAPSHOT_DEADLINE >= Duration::from_secs(5 * 60));
        assert!(S3_OP_TIMEOUT <= Duration::from_secs(60));
        assert!(S3_OP_MAX_ATTEMPTS >= 2);
    }
}
