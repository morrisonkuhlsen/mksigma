//! SPSS system files (`.sav`, `.zsav`). Clean-room: built from the publicly documented record
//! layout, never from SPSS binaries or GPL code. Only the header check exists so far; see
//! `docs/roadmap.md`.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use mksigma_core::Dataset;
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum SavError {
    #[error("not an SPSS system file")]
    NotSav,
    #[error("reading {0} is not implemented yet")]
    Unsupported(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flavour {
    /// `$FL2`: uncompressed or bytecode-compressed `.sav`.
    Sav,
    /// `$FL3`: zlib-compressed `.zsav`.
    Zsav,
}

pub fn detect(bytes: &[u8]) -> Result<Flavour, SavError> {
    match bytes.get(..4) {
        Some(b"$FL2") => Ok(Flavour::Sav),
        Some(b"$FL3") => Ok(Flavour::Zsav),
        _ => Err(SavError::NotSav),
    }
}

pub fn read(bytes: &[u8]) -> Result<Dataset, SavError> {
    match detect(bytes)? {
        Flavour::Sav => Err(SavError::Unsupported(".sav")),
        Flavour::Zsav => Err(SavError::Unsupported(".zsav")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_the_magic() {
        assert_eq!(detect(b"$FL2@(#) ..."), Ok(Flavour::Sav));
        assert_eq!(detect(b"$FL3"), Ok(Flavour::Zsav));
        assert_eq!(detect(b"$FL"), Err(SavError::NotSav));
        assert_eq!(detect(b"PK\x03\x04"), Err(SavError::NotSav));
    }
}
