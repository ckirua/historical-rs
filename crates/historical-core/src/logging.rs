//! Tracing init shared by all CLIs.

use tracing_subscriber::EnvFilter;

/// Initialize `tracing-subscriber` with `RUST_LOG` or a verbosity default.
pub fn init_tracing(verbose: bool) -> anyhow::Result<()> {
    let default = if verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default)),
        )
        .init();
    Ok(())
}
