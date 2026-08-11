//! Shared progress counters for long mirror runs.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::task::JoinHandle;
use tracing::info;

#[derive(Debug)]
pub struct MirrorProgress {
    pub total: usize,
    pub done: AtomicUsize,
    pub skipped: AtomicUsize,
    pub failed: AtomicUsize,
}

impl MirrorProgress {
    pub fn new(total: usize) -> Arc<Self> {
        Arc::new(Self {
            total,
            done: AtomicUsize::new(0),
            skipped: AtomicUsize::new(0),
            failed: AtomicUsize::new(0),
        })
    }

    pub fn record(&self, status: &str) {
        match status {
            "ok" => {
                self.done.fetch_add(1, Ordering::Relaxed);
            }
            "skipped" => {
                self.skipped.fetch_add(1, Ordering::Relaxed);
            }
            "failed" => {
                self.failed.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
    }

    pub fn processed(&self) -> usize {
        self.done.load(Ordering::Relaxed)
            + self.skipped.load(Ordering::Relaxed)
            + self.failed.load(Ordering::Relaxed)
    }

    pub fn snapshot(&self) -> (usize, usize, usize, usize) {
        (
            self.processed(),
            self.done.load(Ordering::Relaxed),
            self.skipped.load(Ordering::Relaxed),
            self.failed.load(Ordering::Relaxed),
        )
    }
}

/// Emit progress to stderr every 30s until all `total` files are processed.
pub fn spawn_progress_reporter(progress: Arc<MirrorProgress>, label: &str) -> JoinHandle<()> {
    let label = label.to_string();
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(30));
        ticker.tick().await;
        loop {
            let (processed, done, skipped, failed) = progress.snapshot();
            eprintln!(
                "{label}: {processed}/{} done={done} skipped={skipped} failed={failed}",
                progress.total
            );
            info!(
                label = %label,
                processed,
                total = progress.total,
                done,
                skipped,
                failed,
                "mirror progress"
            );
            if processed >= progress.total {
                break;
            }
            ticker.tick().await;
        }
    })
}
