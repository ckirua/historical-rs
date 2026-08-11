//! Download and convert Vision archives to parquet.zstd.

use crate::client::VisionHttpClient;
use crate::error::{Error, Result};
use crate::export::{convert_vision_archive, convert_vision_bytes, local_dataset_path};
use crate::model::VisionFile;
use crate::vision::VisionUrl;

pub async fn download_and_convert(
    http: &VisionHttpClient,
    file: &VisionFile,
    verify_checksum: bool,
) -> Result<(Vec<u8>, usize, bool)> {
    let url = http.url_builder().build(file);
    let (zip_bytes, checksum_verified) = if verify_checksum {
        http.download_with_checksum(&url).await?
    } else {
        (http.get_bytes(&url).await?, false)
    };
    let (parquet, rows) = convert_vision_bytes(file, &zip_bytes)?;
    Ok((parquet, rows, checksum_verified))
}

pub async fn download_and_convert_file(
    http: &VisionHttpClient,
    file: &VisionFile,
    verify_checksum: bool,
) -> Result<(tempfile::NamedTempFile, usize, bool)> {
    let url = http.url_builder().build(file);
    let zip = tempfile::NamedTempFile::new()?;
    let checksum_verified = http
        .download_to_file(&url, zip.path(), verify_checksum)
        .await?;
    let parquet = tempfile::NamedTempFile::new()?;
    let zip_path = zip.path().to_path_buf();
    let parquet_path = parquet.path().to_path_buf();
    let file = file.clone();
    let rows = tokio::task::spawn_blocking(move || {
        convert_vision_archive(&file, &zip_path, &parquet_path)
    })
    .await
    .map_err(|error| Error::Io(std::io::Error::other(error.to_string())))??;
    Ok((parquet, rows, checksum_verified))
}

pub fn output_path_for_file(
    config: &crate::config::Config,
    file: &VisionFile,
) -> Result<std::path::PathBuf> {
    Ok(local_dataset_path(&config.data_dir()?, file))
}

pub fn build_url(config: &crate::config::Config, file: &VisionFile) -> String {
    VisionUrl::new(&config.vision_base_url).build(file)
}

pub fn is_not_found(err: &Error) -> bool {
    matches!(err, Error::NotFound(_))
}
