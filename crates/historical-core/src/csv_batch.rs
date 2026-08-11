//! Bounded-memory CSV record batching for archive conversion.

use std::io::Read;

/// Default number of data records converted into one Arrow record batch.
pub const DEFAULT_BATCH_ROWS: usize = 65_536;

/// Read CSV records in bounded batches and pass each well-formed CSV fragment to `convert`.
///
/// Set `repeat_header` for parsers configured with `has_headers(true)`. The first record is then
/// repeated at the start of every fragment and is not counted as a data record.
pub fn for_each_csv_batch<R, E, F>(
    reader: R,
    repeat_header: bool,
    batch_rows: usize,
    mut convert: F,
) -> Result<(), E>
where
    R: Read,
    E: From<csv::Error> + From<std::io::Error>,
    F: FnMut(&[u8]) -> Result<(), E>,
{
    let batch_rows = batch_rows.max(1);
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(reader);
    let mut records = reader.byte_records();
    let Some(first) = records.next().transpose()? else {
        return Ok(());
    };

    let header = repeat_header.then(|| first.clone());
    let mut pending = (!repeat_header).then_some(first);

    loop {
        let mut writer = csv::WriterBuilder::new()
            .has_headers(false)
            .from_writer(Vec::new());
        if let Some(header) = &header {
            writer.write_byte_record(header)?;
        }

        let mut rows = 0usize;
        if let Some(record) = pending.take() {
            writer.write_byte_record(&record)?;
            rows += 1;
        }
        while rows < batch_rows {
            let Some(record) = records.next().transpose()? else {
                break;
            };
            writer.write_byte_record(&record)?;
            rows += 1;
        }
        if rows == 0 {
            return Ok(());
        }

        writer.flush()?;
        let bytes = writer.into_inner().map_err(|error| error.into_error())?;
        convert(&bytes)?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeats_header_without_losing_rows() {
        let input = b"name,value\na,1\nb,2\nc,3\n";
        let mut batches = Vec::new();
        for_each_csv_batch::<_, csv::Error, _>(&input[..], true, 2, |batch| {
            batches.push(String::from_utf8(batch.to_vec()).unwrap());
            Ok(())
        })
        .unwrap();

        assert_eq!(batches, ["name,value\na,1\nb,2\n", "name,value\nc,3\n"]);
    }

    #[test]
    fn preserves_headerless_first_record() {
        let input = b"a,1\nb,2\nc,3\n";
        let mut batches = Vec::new();
        for_each_csv_batch::<_, csv::Error, _>(&input[..], false, 2, |batch| {
            batches.push(String::from_utf8(batch.to_vec()).unwrap());
            Ok(())
        })
        .unwrap();

        assert_eq!(batches, ["a,1\nb,2\n", "c,3\n"]);
    }
}
