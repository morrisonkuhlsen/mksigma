//! SPSS system files (`.sav`, `.zsav`). Clean-room: built from the publicly documented record
//! layout, never from SPSS binaries or GPL code.
//!
//! A system file is a 176-byte header, a dictionary (variable, value-label, document and
//! extension records, ended by record 999) and the cases, either raw or bytecode-compressed.
//! Reading supports both byte orders, user-missing values, value labels, long variable names,
//! measurement levels and the character encoding. Not yet: `.zsav` (zlib) data, very long
//! strings (over 255 bytes) as one variable, long-string value labels and missing values.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod cursor;
mod data;
mod dictionary;
#[cfg(test)]
mod tests;

use mksigma_core::{Dataset, DatasetError};
use thiserror::Error;

#[derive(Debug, Clone, Error, PartialEq)]
pub enum SavError {
    #[error("not an SPSS system file")]
    NotSav,
    #[error("the file ends early (at byte {offset})")]
    Truncated { offset: usize },
    #[error("invalid {what} at byte {offset}")]
    Invalid { offset: usize, what: &'static str },
    #[error("reading {0} is not supported yet")]
    Unsupported(&'static str),
    #[error(transparent)]
    Dataset(#[from] DatasetError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flavour {
    /// `$FL2`: uncompressed or bytecode-compressed `.sav`.
    Sav,
    /// `$FL3`: zlib-compressed `.zsav`.
    Zsav,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    None,
    Bytecode,
    Zlib,
}

/// A system file read into a dataset, with what the header says about it.
#[derive(Debug, Clone)]
pub struct SavFile {
    pub dataset: Dataset,
    /// The product that wrote the file (`@(#) SPSS DATA FILE ...`).
    pub product: String,
    /// Creation date and time as written (`dd mmm yy hh:mm:ss`).
    pub created: String,
    pub compression: Compression,
    /// Things read leniently or skipped, for the user to see.
    pub warnings: Vec<String>,
}

pub fn detect(bytes: &[u8]) -> Result<Flavour, SavError> {
    match bytes.get(..4) {
        Some(b"$FL2") => Ok(Flavour::Sav),
        Some(b"$FL3") => Ok(Flavour::Zsav),
        _ => Err(SavError::NotSav),
    }
}

pub fn read(bytes: &[u8]) -> Result<SavFile, SavError> {
    let flavour = detect(bytes)?;
    let dict = dictionary::read(bytes, flavour)?;
    data::read(bytes, dict)
}
