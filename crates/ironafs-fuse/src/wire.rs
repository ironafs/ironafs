// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Little-endian fields read from a borrowed buffer and written to a
//! growing one (fuse-codec.md 5.1, point 1).
//!
//! The protocol uses the host's byte order, and every Linux target IronAFS
//! builds for is little-endian, so the codec is little-endian on every
//! host and its tests run the same on any machine.

#[cfg(test)]
mod tests;

/// A cursor over bytes that never reads past their end: a read that does
/// not fit returns `None` and leaves the cursor where it was.
pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
}

impl<'a> Reader<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    /// The next `N` bytes, if there are that many.
    fn take<const N: usize>(&mut self) -> Option<[u8; N]> {
        let (head, tail) = self.bytes.split_first_chunk::<N>()?;
        self.bytes = tail;
        Some(*head)
    }

    pub(crate) fn u16(&mut self) -> Option<u16> {
        self.take().map(u16::from_le_bytes)
    }

    pub(crate) fn u32(&mut self) -> Option<u32> {
        self.take().map(u32::from_le_bytes)
    }

    pub(crate) fn u64(&mut self) -> Option<u64> {
        self.take().map(u64::from_le_bytes)
    }

    /// Steps over `count` bytes the codec ignores.
    pub(crate) fn skip(&mut self, count: usize) -> Option<()> {
        self.bytes = self.bytes.get(count..)?;
        Some(())
    }

    /// The bytes not yet read.
    pub(crate) fn rest(self) -> &'a [u8] {
        self.bytes
    }
}

pub(crate) fn put_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

pub(crate) fn put_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

pub(crate) fn put_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}
