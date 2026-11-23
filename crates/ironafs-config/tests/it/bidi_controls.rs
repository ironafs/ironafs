// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Unicode's `Bidi_Control` set, which `docs/specs/config-syntax.md`,
//! section 5.1, point 3 refuses: the one test-only list, written out here
//! rather than read from the code under test. The unit tests, the property
//! tests and the fuzz target include this file by path.

/// U+061C, U+200E, U+200F, U+202A to U+202E and U+2066 to U+2069.
pub(crate) const BIDI_CONTROLS: [char; 12] = [
    '\u{061c}', '\u{200e}', '\u{200f}', '\u{202a}', '\u{202b}', '\u{202c}', '\u{202d}', '\u{202e}',
    '\u{2066}', '\u{2067}', '\u{2068}', '\u{2069}',
];
