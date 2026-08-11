//! CSV extraction from Vision ZIP archives.

use std::io::{Cursor, Read};

use zip::read::ZipArchive;

use crate::error::{Error, Result};

/// Extract the best CSV payload from a Vision ZIP file.
///
/// Prefers a single root-level CSV when present; otherwise picks the largest CSV
/// (handles nested duplicate CSV quirk in older aggTrades archives).
pub fn extract_csv_from_zip(zip_bytes: &[u8]) -> Result<Vec<u8>> {
    let cursor = Cursor::new(zip_bytes);
    let mut archive =
        ZipArchive::new(cursor).map_err(|e| Error::Zip(format!("failed to read zip: {e}")))?;

    let mut root_csvs: Vec<(String, Vec<u8>)> = Vec::new();
    let mut nested_csvs: Vec<(String, Vec<u8>)> = Vec::new();

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| Error::Zip(format!("zip index {i}: {e}")))?;
        let name = file.name().to_string();
        if !name.ends_with(".csv") || file.is_dir() {
            continue;
        }
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)
            .map_err(|e| Error::Zip(format!("read {name}: {e}")))?;
        if name.contains('/') {
            nested_csvs.push((name, buf));
        } else {
            root_csvs.push((name, buf));
        }
    }

    if root_csvs.len() == 1 {
        return Ok(root_csvs.remove(0).1);
    }
    if root_csvs.len() > 1 {
        root_csvs.sort_by_key(|(_, b)| std::cmp::Reverse(b.len()));
        return Ok(root_csvs.remove(0).1);
    }
    if nested_csvs.is_empty() {
        return Err(Error::Parse("no CSV found in zip archive".into()));
    }
    nested_csvs.sort_by_key(|(_, b)| std::cmp::Reverse(b.len()));
    Ok(nested_csvs.remove(0).1)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    use super::*;

    fn make_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut writer = ZipWriter::new(std::io::Cursor::new(&mut buf));
            for (name, data) in entries {
                writer
                    .start_file(*name, SimpleFileOptions::default())
                    .unwrap();
                writer.write_all(data).unwrap();
            }
            writer.finish().unwrap();
        }
        buf
    }

    #[test]
    fn prefers_single_root_csv() {
        let zip = make_zip(&[("BTCUSDT-aggTrades.csv", b"a,b\n1,2\n")]);
        let csv = extract_csv_from_zip(&zip).unwrap();
        assert_eq!(csv, b"a,b\n1,2\n");
    }

    #[test]
    fn picks_largest_when_multiple_root_csvs() {
        let zip = make_zip(&[
            ("small.csv", b"x\n"),
            ("large.csv", b"x\n".repeat(100).as_slice()),
        ]);
        let csv = extract_csv_from_zip(&zip).unwrap();
        assert_eq!(csv.len(), 200);
    }
}
