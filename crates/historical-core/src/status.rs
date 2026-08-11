//! Mirror progress summary counters.

use serde::{Deserialize, Serialize};

/// Aggregate counts from a JSONL mirror manifest.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MirrorStatus {
    pub total: usize,
    pub done: usize,
    pub failed: usize,
    pub skipped: usize,
    pub pending: usize,
}

impl MirrorStatus {
    /// Fold status strings (`ok` / `skipped` / `failed` / other) into counters.
    pub fn record(&mut self, status: &str) {
        self.total += 1;
        match status {
            "ok" => self.done += 1,
            "skipped" => self.skipped += 1,
            "failed" => self.failed += 1,
            _ => self.pending += 1,
        }
    }

    /// Merge counters from another independently processed batch.
    pub fn merge(&mut self, other: Self) {
        self.total += other.total;
        self.done += other.done;
        self.failed += other.failed;
        self.skipped += other.skipped;
        self.pending += other.pending;
    }

    /// Summarize a slice of status strings without growing `total` twice.
    pub fn from_statuses<'a, I>(statuses: I) -> Self
    where
        I: IntoIterator<Item = &'a str>,
    {
        let mut status = Self::default();
        for s in statuses {
            status.record(s);
        }
        status
    }
}

#[derive(Debug, Deserialize)]
struct StatusLine {
    status: String,
}

/// Count status fields from a JSONL manifest body.
pub fn mirror_status_from_jsonl(content: &str) -> crate::Result<MirrorStatus> {
    let mut status = MirrorStatus::default();
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let entry: StatusLine = serde_json::from_str(line)?;
        status.record(&entry.status);
    }
    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_jsonl_statuses() {
        let body = r#"{"status":"ok"}
{"status":"skipped"}
{"status":"failed"}
{"status":"pending"}
"#;
        let s = mirror_status_from_jsonl(body).unwrap();
        assert_eq!(
            s,
            MirrorStatus {
                total: 4,
                done: 1,
                skipped: 1,
                failed: 1,
                pending: 1,
            }
        );
    }

    #[test]
    fn merges_independent_batches() {
        let mut status = MirrorStatus::from_statuses(["ok", "failed"]);
        status.merge(MirrorStatus::from_statuses(["ok", "skipped"]));
        assert_eq!(
            status,
            MirrorStatus {
                total: 4,
                done: 2,
                failed: 1,
                skipped: 1,
                pending: 0,
            }
        );
    }
}
