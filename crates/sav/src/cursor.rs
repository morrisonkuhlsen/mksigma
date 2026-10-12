//! Bounds-checked reads in the file's byte order. Every length comes from the file, so every read
//! is checked: a short or hostile file yields an error, never a panic.

use crate::SavError;

#[derive(Debug, Clone)]
pub(crate) struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
    pub(crate) big_endian: bool,
}

impl<'a> Cursor<'a> {
    pub(crate) fn new(bytes: &'a [u8], big_endian: bool) -> Self {
        Self { bytes, pos: 0, big_endian }
    }

    pub(crate) fn pos(&self) -> usize {
        self.pos
    }

    pub(crate) fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.pos)
    }

    pub(crate) fn invalid(&self, what: &'static str) -> SavError {
        SavError::Invalid { offset: self.pos, what }
    }

    pub(crate) fn take(&mut self, n: usize) -> Result<&'a [u8], SavError> {
        let truncated = SavError::Truncated { offset: self.pos };
        let end = self.pos.checked_add(n).ok_or(truncated.clone())?;
        let s = self.bytes.get(self.pos..end).ok_or(truncated)?;
        self.pos = end;
        Ok(s)
    }

    pub(crate) fn array<const N: usize>(&mut self) -> Result<[u8; N], SavError> {
        let offset = self.pos;
        self.take(N)?.try_into().map_err(|_| SavError::Truncated { offset })
    }

    pub(crate) fn i32(&mut self) -> Result<i32, SavError> {
        let a = self.array::<4>()?;
        Ok(if self.big_endian { i32::from_be_bytes(a) } else { i32::from_le_bytes(a) })
    }

    pub(crate) fn f64(&mut self) -> Result<f64, SavError> {
        let a = self.array::<8>()?;
        Ok(self.f64_from(a))
    }

    pub(crate) fn f64_from(&self, a: [u8; 8]) -> f64 {
        if self.big_endian { f64::from_be_bytes(a) } else { f64::from_le_bytes(a) }
    }

    /// A count or length stored as a 32-bit integer; negative is invalid.
    pub(crate) fn count(&mut self, what: &'static str) -> Result<usize, SavError> {
        let offset = self.pos;
        let n = self.i32()?;
        usize::try_from(n).map_err(|_| SavError::Invalid { offset, what })
    }
}
