//! Generic CSV → columnar strings for uncommon Vision data types.

use crate::error::{Error, Result};

#[derive(Debug, Clone)]
pub struct GenericCsvTable {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

pub fn parse_generic_csv(csv_bytes: &[u8]) -> Result<GenericCsvTable> {
    let text = std::str::from_utf8(csv_bytes)
        .map_err(|e| Error::Parse(format!("CSV is not UTF-8: {e}")))?;
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(text.as_bytes());

    let mut rows = Vec::new();
    let mut max_cols = 0usize;
    for record in reader.records() {
        let record = record?;
        if record.iter().all(|f| f.trim().is_empty()) {
            continue;
        }
        max_cols = max_cols.max(record.len());
        rows.push(record.iter().map(|s| s.to_string()).collect());
    }
    let headers = (0..max_cols)
        .map(|i| format!("col_{i}"))
        .collect::<Vec<_>>();
    Ok(GenericCsvTable { headers, rows })
}
