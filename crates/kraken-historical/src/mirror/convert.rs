//! Convert one planned parquet output from a disk-backed ZIP.

use std::path::Path;

use crate::error::{Error, Result};
use crate::export::{convert_member_to_parquet, convert_member_to_parquet_file};
use crate::keys::{dataset_object_key, local_dataset_path};
use crate::model::OutputUnit;
use crate::parse::ZipMember;

pub fn convert_output_unit(
    zip_path: &Path,
    member: &ZipMember,
    unit: &OutputUnit,
) -> Result<(Vec<u8>, usize)> {
    convert_member_to_parquet(zip_path, member, unit)
}

pub fn convert_output_unit_to_file(
    zip_path: &Path,
    member: &ZipMember,
    unit: &OutputUnit,
    output_path: &Path,
) -> Result<usize> {
    convert_member_to_parquet_file(zip_path, member, unit, output_path)
}

pub fn output_path_for_unit(
    config: &crate::config::Config,
    unit: &OutputUnit,
) -> Result<std::path::PathBuf> {
    Ok(local_dataset_path(&config.data_dir()?, unit))
}

#[allow(dead_code)]
pub fn object_key_for_unit(unit: &OutputUnit) -> String {
    dataset_object_key(unit)
}

pub fn is_not_found(err: &Error) -> bool {
    matches!(err, Error::NotFound(_))
}
