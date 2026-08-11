//! Google Drive discovery and download for Kraken historical ZIPs.

mod download;
mod ids;
mod list;
mod scrape;

pub use download::{download_artifact, zip_scratch_path};
pub use ids::{
    complete_artifact, OHLCVT_COMPLETE_FILE_ID, OHLCVT_QUARTERLY_FOLDER_ID,
    TRADES_COMPLETE_FILE_ID, TRADES_QUARTERLY_FOLDER_ID,
};
pub use list::{discover_artifacts, list_quarterly_artifacts, parse_quarter_filename};
pub use scrape::{extract_drive_file_ids, scrape_complete_file_ids};

use crate::config::Config;
use crate::error::Result;
use crate::model::{DataType, DriveArtifact};

/// Discover artifacts for one or both data types (complete + quarterly via public folder scrape).
pub async fn discover(config: &Config, data_type: Option<DataType>) -> Result<Vec<DriveArtifact>> {
    discover_artifacts(config, data_type).await
}
