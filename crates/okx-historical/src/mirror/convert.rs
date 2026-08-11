//! Download and convert OKX archives to parquet.zstd.

use crate::cdn::CdnUrl;
use crate::client::OkxHttpClient;
use crate::error::{Error, Result};
use crate::export::{convert_okx_archive, convert_okx_bytes, local_dataset_path};
use crate::model::OkxFile;

pub async fn download_and_convert(
    http: &OkxHttpClient,
    file: &OkxFile,
) -> Result<(Vec<u8>, usize)> {
    let url = http.url_builder().build(file);
    let zip_bytes = http.get_bytes(&url).await?;
    convert_okx_bytes(file, &zip_bytes)
}

pub async fn download_and_convert_file(
    http: &OkxHttpClient,
    file: &OkxFile,
) -> Result<(tempfile::NamedTempFile, usize)> {
    let url = http.url_builder().build(file);
    let zip = tempfile::NamedTempFile::new()?;
    http.download_to_file(&url, zip.path()).await?;
    let parquet = tempfile::NamedTempFile::new()?;
    let zip_path = zip.path().to_path_buf();
    let parquet_path = parquet.path().to_path_buf();
    let file = file.clone();
    let rows =
        tokio::task::spawn_blocking(move || convert_okx_archive(&file, &zip_path, &parquet_path))
            .await
            .map_err(|error| Error::Io(std::io::Error::other(error.to_string())))??;
    Ok((parquet, rows))
}

pub fn output_path_for_file(
    config: &crate::config::Config,
    file: &OkxFile,
) -> Result<std::path::PathBuf> {
    Ok(local_dataset_path(&config.data_dir()?, file))
}

pub fn build_url(config: &crate::config::Config, file: &OkxFile) -> String {
    CdnUrl::new(&config.cdn_base_url).build(file)
}

pub fn is_not_found(err: &Error) -> bool {
    matches!(err, Error::NotFound(_))
}
