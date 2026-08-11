//! CSV parsers for Binance Vision archives.

mod agg_trades;
mod generic;
mod klines;
mod trades;
mod zip;

pub use agg_trades::parse_agg_trades_csv;
pub use generic::{parse_generic_csv, GenericCsvTable};
pub use klines::parse_klines_csv;
pub use trades::parse_trades_csv;
pub use zip::extract_csv_from_zip;
