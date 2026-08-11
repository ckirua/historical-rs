//! Hardcoded Kraken Google Drive folder/file IDs.

use crate::model::{DataType, DriveArtifact, Source};

/// Trades complete dump file.
pub const TRADES_COMPLETE_FILE_ID: &str = "10zh3tDpqANYvVtYVgczwVz3UZFRUb1el";
/// Trades quarterly folder.
pub const TRADES_QUARTERLY_FOLDER_ID: &str = "188O9xQjZTythjyLNes_5zfMEFaMbTT22";
/// OHLCVT complete dump file.
pub const OHLCVT_COMPLETE_FILE_ID: &str = "1ptNqWYidLkhb2VAKuLCxmp2OXEfGO-AP";
/// OHLCVT quarterly folder.
pub const OHLCVT_QUARTERLY_FOLDER_ID: &str = "15RSlNuW_h0kVM8or8McOGOMfHeBFvFGI";

pub const TRADES_SUPPORT_URL: &str =
    "https://support.kraken.com/articles/360047543791-downloadable-historical-market-data-time-and-sales-";
pub const OHLCVT_SUPPORT_URL: &str =
    "https://support.kraken.com/articles/360047124832-downloadable-historical-ohlcvt-open-high-low-close-volume-trades-data";

pub fn complete_artifact(data_type: DataType) -> DriveArtifact {
    match data_type {
        DataType::Trades => DriveArtifact {
            data_type,
            source: Source::Complete,
            file_id: TRADES_COMPLETE_FILE_ID.to_string(),
            name: "Kraken_Trading_History.zip".to_string(),
            size_hint: None,
        },
        DataType::Ohlcvt => DriveArtifact {
            data_type,
            source: Source::Complete,
            file_id: OHLCVT_COMPLETE_FILE_ID.to_string(),
            name: "Kraken_OHLCVT.zip".to_string(),
            size_hint: None,
        },
    }
}

pub fn quarterly_folder_id(data_type: DataType) -> &'static str {
    match data_type {
        DataType::Trades => TRADES_QUARTERLY_FOLDER_ID,
        DataType::Ohlcvt => OHLCVT_QUARTERLY_FOLDER_ID,
    }
}
