// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Where rendering writes: a byte buffer, a string, or a count. Encoders
//! run once into a [`Count`] to learn a message's exact size, and allocate
//! its buffer only once that size is known to fit the cap
//! (`docs/specs/events.md` §5.10, point 1).

use alloc::string::String;
use alloc::vec::Vec;

/// Takes rendered bytes. Everything rendered is ASCII.
pub(crate) trait Sink {
    fn put(&mut self, bytes: &[u8]);
}

impl Sink for Vec<u8> {
    fn put(&mut self, bytes: &[u8]) {
        self.extend_from_slice(bytes);
    }
}

impl Sink for String {
    fn put(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.push(char::from(byte));
        }
    }
}

/// Counts the bytes it is given and keeps none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Count(pub(crate) usize);

impl Sink for Count {
    fn put(&mut self, bytes: &[u8]) {
        self.0 = self.0.saturating_add(bytes.len());
    }
}

impl<S: Sink + ?Sized> Sink for &mut S {
    fn put(&mut self, bytes: &[u8]) {
        (**self).put(bytes);
    }
}
