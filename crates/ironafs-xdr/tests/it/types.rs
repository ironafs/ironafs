// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The test types of `docs/specs/xdr.md`, section 10, written by hand as
//! the generator would emit them, with their caps and messages.
//!
//! This module uses only the crate's public API, so the fuzz target can
//! build the same types from this file.
//!
//! ```text
//! enum shade { PALE = 1, DEEP = 4, VIVID = 9 };     /* closed */
//! enum kind { PLAIN = 0, LINKED = 1, STAMPED = 2, RETIRED = 3 };
//! union entry switch (kind k) {                     /* no arm for RETIRED, */
//! case PLAIN:   void;                               /* and no default      */
//! case LINKED:  string target<16>;
//! case STAMPED: unsigned hyper stamp;
//! };
//! typedef string label<8>;
//! struct note {
//!     string title<32>;
//!     entry  what;
//!     label  owner;
//!     opaque body<64>;
//! };
//! label pair[2];
//! label names<>;
//! label tags<3>;
//! enum solo { ONLY = 5 };                           /* one value */
//! solo solos<>;
//! struct quad { opaque a[60]; opaque b[60]; opaque c[60]; opaque d[60]; };
//! union wide switch (bool big) {
//! case FALSE: void;
//! case TRUE:  quad block;
//! };
//! struct sealed {                                   /* secret-bearing */
//!     opaque key<16>;                               /* secret */
//!     unsigned int serial;
//!     label tag;
//! };
//! ```
//!
//! Besides those: `flag` (`bool`), `five` (`opaque x[5]`), `blob` (`opaque x<>`, cap 16),
//! `ints` (`unsigned int x<>`, cap 64), the chain `deep` of 16 nested
//! structures, and `every`, one field of each profile type.

use core::mem::size_of;

use ironafs_xdr::limits::INLINE_MAX;
use ironafs_xdr::{
    BorrowedBytes, BorrowedWord, Cap, Decode, Decoder, Encode, Encoder, Error, Message, Plain,
    Vacant, array_heap_ratio, array_len, array_reservations, fixed_array_len, fixed_opaque_len,
    max, opaque_len, proven, sum,
};

/// The message row of a test type decoded on its own.
const fn alone(name: &'static str) -> Message {
    Message {
        name,
        cap: 128,
        budget: 1024,
        secret: false,
    }
}

/// Asserts at compile time what the generator asserts for every type.
macro_rules! inline_fits {
    ($($type:ty),* $(,)?) => {
        $(const _: () = assert!(size_of::<$type>() <= INLINE_MAX);)*
    };
}

// ---- shade, kind, solo: enumerations ----

/// `shade`: a closed enumeration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Shade {
    Pale,
    Deep,
    Vivid,
}

pub(crate) const SHADE: Message = alone("test.shade");

impl Shade {
    pub(crate) const ALL: [Self; 3] = [Self::Pale, Self::Deep, Self::Vivid];

    pub(crate) fn declared(value: i32) -> Option<Self> {
        match value {
            1 => Some(Self::Pale),
            4 => Some(Self::Deep),
            9 => Some(Self::Vivid),
            _ => None,
        }
    }

    pub(crate) const fn value(self) -> i32 {
        match self {
            Self::Pale => 1,
            Self::Deep => 4,
            Self::Vivid => 9,
        }
    }
}

impl<'a> Decode<'a> for Shade {
    const S_MIN: u64 = 4;
    const DEPTH: u32 = 0;
    const HEAP_RATIO: Option<u64> = Some(0);
    const RESERVATIONS: Option<u64> = Some(0);

    fn decode(decoder: &mut Decoder<'a, '_>) -> Result<Self, Error> {
        decoder.read_enum("test.shade", Self::declared)
    }
}

impl Plain for Shade {}

impl Encode for Shade {
    fn encoded_len(&self) -> Result<u64, Error> {
        Ok(4)
    }

    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), Error> {
        encoder.put_i32(self.value())
    }
}

/// `bool` on its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Flag(pub(crate) bool);

pub(crate) const FLAG: Message = alone("test.flag");

impl<'a> Decode<'a> for Flag {
    const S_MIN: u64 = 4;
    const DEPTH: u32 = 0;
    const HEAP_RATIO: Option<u64> = Some(0);
    const RESERVATIONS: Option<u64> = Some(0);

    fn decode(decoder: &mut Decoder<'a, '_>) -> Result<Self, Error> {
        decoder.read_bool("test.flag").map(Self)
    }
}

impl Plain for Flag {}

impl Encode for Flag {
    fn encoded_len(&self) -> Result<u64, Error> {
        Ok(4)
    }

    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), Error> {
        encoder.put_bool(self.0)
    }
}

/// `kind`: the discriminant of `entry`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Plain,
    Linked,
    Stamped,
    Retired,
}

impl Kind {
    pub(crate) fn declared(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::Plain),
            1 => Some(Self::Linked),
            2 => Some(Self::Stamped),
            3 => Some(Self::Retired),
            _ => None,
        }
    }
}

/// `solo`: a closed enumeration with one value, held with no stored field
/// since its value is known.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Solo;

const _: () = assert!(size_of::<Solo>() == 0);

impl<'a> Decode<'a> for Solo {
    const S_MIN: u64 = 4;
    const DEPTH: u32 = 0;
    const HEAP_RATIO: Option<u64> = Some(0);
    const RESERVATIONS: Option<u64> = Some(0);

    fn decode(decoder: &mut Decoder<'a, '_>) -> Result<Self, Error> {
        decoder.read_enum("test.solo", |value| (value == 5).then_some(Self))
    }
}

impl Plain for Solo {}

impl Encode for Solo {
    fn encoded_len(&self) -> Result<u64, Error> {
        Ok(4)
    }

    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), Error> {
        encoder.put_i32(5)
    }
}

// ---- entry: a union with a void arm and no default ----

pub(crate) const TARGET: Cap = Cap {
    name: "test.note.what.target",
    bound: Some(16),
    bytes: 16,
};

/// `entry`: no arm for `RETIRED`, and no default arm.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Entry {
    Plain,
    Linked(Vec<u8>),
    Stamped(u64),
}

pub(crate) const ENTRY: Message = alone("test.entry");

impl<'a> Decode<'a> for Entry {
    const S_MIN: u64 = 4;
    const DEPTH: u32 = 1;
    const HEAP_RATIO: Option<u64> = Some(1);
    const RESERVATIONS: Option<u64> = Some(1);

    fn decode(decoder: &mut Decoder<'a, '_>) -> Result<Self, Error> {
        let at = decoder.mark();
        match decoder.read_enum("test.entry.k", Kind::declared)? {
            Kind::Plain => Ok(Self::Plain),
            Kind::Linked => decoder.opaque_owned(&TARGET).map(Self::Linked),
            Kind::Stamped => decoder.read_u64().map(Self::Stamped),
            Kind::Retired => Err(decoder.unknown_discriminant("test.entry.k", at)),
        }
    }
}

impl Plain for Entry {}

impl Encode for Entry {
    fn encoded_len(&self) -> Result<u64, Error> {
        match self {
            Self::Plain => Ok(4),
            Self::Linked(target) => sum(4, opaque_len(&TARGET, target.len())?),
            Self::Stamped(_) => Ok(12),
        }
    }

    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), Error> {
        match self {
            Self::Plain => encoder.put_i32(0),
            Self::Linked(target) => {
                encoder.put_i32(1)?;
                encoder.put_opaque(&TARGET, target)
            }
            Self::Stamped(stamp) => {
                encoder.put_i32(2)?;
                encoder.put_u64(*stamp)
            }
        }
    }
}

// ---- label, note ----

pub(crate) const LABEL_CAP: Cap = Cap {
    name: "test.label",
    bound: Some(8),
    bytes: 8,
};

/// `label`: a string of at most 8 bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Label(pub(crate) Vec<u8>);

pub(crate) const LABEL: Message = alone("test.label");

impl Vacant for Label {
    const VACANT: Self = Label(Vec::new());
}

impl<'a> Decode<'a> for Label {
    const S_MIN: u64 = 4;
    const DEPTH: u32 = 0;
    const HEAP_RATIO: Option<u64> = Some(1);
    const RESERVATIONS: Option<u64> = Some(1);

    fn decode(decoder: &mut Decoder<'a, '_>) -> Result<Self, Error> {
        decoder.opaque_owned(&LABEL_CAP).map(Self)
    }
}

impl Plain for Label {}

impl Encode for Label {
    fn encoded_len(&self) -> Result<u64, Error> {
        opaque_len(&LABEL_CAP, self.0.len())
    }

    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), Error> {
        encoder.put_opaque(&LABEL_CAP, &self.0)
    }
}

pub(crate) const TITLE: Cap = Cap {
    name: "test.note.title",
    bound: Some(32),
    bytes: 32,
};

pub(crate) const OWNER: Cap = Cap {
    name: "test.note.owner",
    bound: Some(8),
    bytes: 8,
};

/// Below its declared bound of 64 bytes.
pub(crate) const BODY: Cap = Cap {
    name: "test.note.body",
    bound: Some(64),
    bytes: 16,
};

/// `note`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Note {
    pub(crate) title: Vec<u8>,
    pub(crate) what: Entry,
    pub(crate) owner: Vec<u8>,
    pub(crate) body: Vec<u8>,
}

pub(crate) const NOTE: Message = Message {
    name: "test.note",
    cap: 128,
    budget: 128,
    secret: false,
};

impl<'a> Decode<'a> for Note {
    const S_MIN: u64 = 16;
    const DEPTH: u32 = 1 + Entry::DEPTH;
    const HEAP_RATIO: Option<u64> = max(Some(1), Entry::HEAP_RATIO);
    const RESERVATIONS: Option<u64> = max(Some(1), Entry::RESERVATIONS);

    fn decode(decoder: &mut Decoder<'a, '_>) -> Result<Self, Error> {
        Ok(Self {
            title: decoder.opaque_owned(&TITLE)?,
            what: Entry::decode(decoder)?,
            owner: decoder.opaque_owned(&OWNER)?,
            body: decoder.opaque_owned(&BODY)?,
        })
    }
}

impl Plain for Note {}

impl Encode for Note {
    fn encoded_len(&self) -> Result<u64, Error> {
        let mut total = opaque_len(&TITLE, self.title.len())?;
        total = sum(total, self.what.encoded_len()?)?;
        total = sum(total, opaque_len(&OWNER, self.owner.len())?)?;
        sum(total, opaque_len(&BODY, self.body.len())?)
    }

    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), Error> {
        encoder.put_opaque(&TITLE, &self.title)?;
        self.what.encode(encoder)?;
        encoder.put_opaque(&OWNER, &self.owner)?;
        encoder.put_opaque(&BODY, &self.body)
    }
}

// ---- pair, names, tags, solos, ints: arrays ----

/// `pair`: a fixed-length array of two labels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Pair(pub(crate) [Label; 2]);

pub(crate) const PAIR: Message = alone("test.pair");

impl<'a> Decode<'a> for Pair {
    const S_MIN: u64 = 2 * Label::S_MIN;
    const DEPTH: u32 = 1 + Label::DEPTH;
    const HEAP_RATIO: Option<u64> = Label::HEAP_RATIO;
    const RESERVATIONS: Option<u64> = Label::RESERVATIONS;

    fn decode(decoder: &mut Decoder<'a, '_>) -> Result<Self, Error> {
        decoder.fixed_array().map(Self)
    }
}

impl Plain for Pair {}

impl Encode for Pair {
    fn encoded_len(&self) -> Result<u64, Error> {
        fixed_array_len(&self.0)
    }

    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), Error> {
        encoder.put_fixed_array(&self.0)
    }
}

/// Declares a variable-length array type of `$element` under `$cap`.
macro_rules! array_type {
    ($name:ident, $element:ty, $cap:ident, $message:ident, $cap_name:literal, $bound:expr, $bytes:expr) => {
        pub(crate) const $cap: Cap = Cap {
            name: $cap_name,
            bound: $bound,
            bytes: $bytes,
        };

        pub(crate) const $message: Message = alone($cap_name);

        #[derive(Clone, Debug, Default, PartialEq, Eq)]
        pub(crate) struct $name(pub(crate) Vec<$element>);

        impl<'a> Decode<'a> for $name {
            const S_MIN: u64 = 4;
            const DEPTH: u32 = 1 + <$element as Decode<'a>>::DEPTH;
            const HEAP_RATIO: Option<u64> = array_heap_ratio::<$element>();
            const RESERVATIONS: Option<u64> = array_reservations::<$element>();

            fn decode(decoder: &mut Decoder<'a, '_>) -> Result<Self, Error> {
                decoder.array(&$cap).map(Self)
            }
        }

        impl Plain for $name {}

        impl Encode for $name {
            fn encoded_len(&self) -> Result<u64, Error> {
                array_len(&$cap, &self.0)
            }

            fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), Error> {
                encoder.put_array(&$cap, &self.0)
            }
        }
    };
}

array_type!(Names, Label, NAMES_CAP, NAMES, "test.names", None, 16);
array_type!(Tags, Label, TAGS_CAP, TAGS, "test.tags", Some(3), 16);
array_type!(Solos, Solo, SOLOS_CAP, SOLOS, "test.solos", None, 16);
array_type!(Ints, u32, INTS_CAP, INTS, "test.ints", None, 64);

// ---- five, blob: opaque data on its own ----

/// `opaque x[5]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Five(pub(crate) [u8; 5]);

pub(crate) const FIVE: Message = alone("test.five");

impl<'a> Decode<'a> for Five {
    const S_MIN: u64 = 8;
    const DEPTH: u32 = 0;
    const HEAP_RATIO: Option<u64> = Some(0);
    const RESERVATIONS: Option<u64> = Some(0);

    fn decode(decoder: &mut Decoder<'a, '_>) -> Result<Self, Error> {
        decoder.fixed_opaque_inline().map(Self)
    }
}

impl Plain for Five {}

impl Encode for Five {
    fn encoded_len(&self) -> Result<u64, Error> {
        fixed_opaque_len(5)
    }

    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), Error> {
        encoder.put_fixed_opaque(&self.0)
    }
}

pub(crate) const BLOB_CAP: Cap = Cap {
    name: "test.blob",
    bound: None,
    bytes: 16,
};

/// `opaque x<>` with a cap of 16.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Blob(pub(crate) Vec<u8>);

pub(crate) const BLOB: Message = alone("test.blob");

impl<'a> Decode<'a> for Blob {
    const S_MIN: u64 = 4;
    const DEPTH: u32 = 0;
    const HEAP_RATIO: Option<u64> = Some(1);
    const RESERVATIONS: Option<u64> = Some(1);

    fn decode(decoder: &mut Decoder<'a, '_>) -> Result<Self, Error> {
        decoder.opaque_owned(&BLOB_CAP).map(Self)
    }
}

impl Plain for Blob {}

impl Encode for Blob {
    fn encoded_len(&self) -> Result<u64, Error> {
        opaque_len(&BLOB_CAP, self.0.len())
    }

    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), Error> {
        encoder.put_opaque(&BLOB_CAP, &self.0)
    }
}

// ---- quad, wide: the inline limit ----

/// `quad`: 240 bytes of fixed-length opaque, held inline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Quad {
    pub(crate) a: [u8; 60],
    pub(crate) b: [u8; 60],
    pub(crate) c: [u8; 60],
    pub(crate) d: [u8; 60],
}

impl<'a> Decode<'a> for Quad {
    const S_MIN: u64 = 240;
    const DEPTH: u32 = 1;
    const HEAP_RATIO: Option<u64> = Some(0);
    const RESERVATIONS: Option<u64> = Some(0);

    fn decode(decoder: &mut Decoder<'a, '_>) -> Result<Self, Error> {
        Ok(Self {
            a: decoder.fixed_opaque_inline()?,
            b: decoder.fixed_opaque_inline()?,
            c: decoder.fixed_opaque_inline()?,
            d: decoder.fixed_opaque_inline()?,
        })
    }
}

impl Plain for Quad {}

impl Encode for Quad {
    fn encoded_len(&self) -> Result<u64, Error> {
        Ok(240)
    }

    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), Error> {
        encoder.put_fixed_opaque(&self.a)?;
        encoder.put_fixed_opaque(&self.b)?;
        encoder.put_fixed_opaque(&self.c)?;
        encoder.put_fixed_opaque(&self.d)
    }
}

/// `wide`: its `TRUE` arm holds a `quad` inline.
#[derive(Clone, Debug, PartialEq, Eq)]
#[expect(
    clippy::large_enum_variant,
    reason = "rule 15: wide exists to hold its large arm inline, within the inline limit"
)]
pub(crate) enum Wide {
    Small,
    Big(Quad),
}

pub(crate) const WIDE: Message = Message {
    name: "test.wide",
    cap: 244,
    budget: 1024,
    secret: false,
};

impl<'a> Decode<'a> for Wide {
    const S_MIN: u64 = 4;
    const DEPTH: u32 = 1 + Quad::DEPTH;
    const HEAP_RATIO: Option<u64> = Some(0);
    const RESERVATIONS: Option<u64> = Some(0);

    fn decode(decoder: &mut Decoder<'a, '_>) -> Result<Self, Error> {
        if decoder.read_bool("test.wide.big")? {
            Quad::decode(decoder).map(Self::Big)
        } else {
            Ok(Self::Small)
        }
    }
}

impl Plain for Wide {}

impl Encode for Wide {
    fn encoded_len(&self) -> Result<u64, Error> {
        match self {
            Self::Small => Ok(4),
            Self::Big(block) => sum(4, block.encoded_len()?),
        }
    }

    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), Error> {
        match self {
            Self::Small => encoder.put_bool(false),
            Self::Big(block) => {
                encoder.put_bool(true)?;
                block.encode(encoder)
            }
        }
    }
}

// ---- deep: 16 nested structures ----

/// The innermost link of `deep`: a hyper and 64 bytes of padding data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Deep16 {
    pub(crate) value: u64,
    pub(crate) pad: [u8; 64],
}

impl<'a> Decode<'a> for Deep16 {
    const S_MIN: u64 = 72;
    const DEPTH: u32 = 1;
    const HEAP_RATIO: Option<u64> = Some(0);
    const RESERVATIONS: Option<u64> = Some(0);

    fn decode(decoder: &mut Decoder<'a, '_>) -> Result<Self, Error> {
        Ok(Self {
            value: decoder.read_u64()?,
            pad: decoder.fixed_opaque_inline()?,
        })
    }
}

impl Plain for Deep16 {}

impl Encode for Deep16 {
    fn encoded_len(&self) -> Result<u64, Error> {
        Ok(72)
    }

    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), Error> {
        encoder.put_u64(self.value)?;
        encoder.put_fixed_opaque(&self.pad)
    }
}

/// Declares a link of `deep` holding a hyper and the next link.
macro_rules! deep_link {
    ($($name:ident => $next:ident),* $(,)?) => {$(
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub(crate) struct $name {
            pub(crate) value: u64,
            pub(crate) next: $next,
        }

        impl<'a> Decode<'a> for $name {
            const S_MIN: u64 = 8 + $next::S_MIN;
            const DEPTH: u32 = 1 + $next::DEPTH;
            const HEAP_RATIO: Option<u64> = Some(0);
            const RESERVATIONS: Option<u64> = Some(0);

            fn decode(decoder: &mut Decoder<'a, '_>) -> Result<Self, Error> {
                Ok(Self {
                    value: decoder.read_u64()?,
                    next: $next::decode(decoder)?,
                })
            }
        }

        impl Plain for $name {}

        impl Encode for $name {
            fn encoded_len(&self) -> Result<u64, Error> {
                sum(8, self.next.encoded_len()?)
            }

            fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), Error> {
                encoder.put_u64(self.value)?;
                self.next.encode(encoder)
            }
        }
    )*};
}

deep_link!(
    Deep15 => Deep16, Deep14 => Deep15, Deep13 => Deep14, Deep12 => Deep13,
    Deep11 => Deep12, Deep10 => Deep11, Deep9 => Deep10, Deep8 => Deep9,
    Deep7 => Deep8, Deep6 => Deep7, Deep5 => Deep6, Deep4 => Deep5,
    Deep3 => Deep4, Deep2 => Deep3, Deep => Deep2,
);

const _: () = assert!(Deep::DEPTH == 16);
const _: () = assert!(Deep::S_MIN == 192);

pub(crate) const DEEP: Message = Message {
    name: "test.deep",
    cap: 192,
    budget: 1024,
    secret: false,
};

// ---- sealed: the secret-bearing type ----

pub(crate) const KEY: Cap = Cap {
    name: "test.sealed.key",
    bound: Some(16),
    bytes: 16,
};

pub(crate) const TAG: Cap = Cap {
    name: "test.sealed.tag",
    bound: Some(8),
    bytes: 8,
};

/// `sealed`: every field is borrowed from the input, and `Debug` shows
/// field names only.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Sealed<'a> {
    pub(crate) key: BorrowedBytes<'a>,
    pub(crate) serial: BorrowedWord<'a>,
    pub(crate) tag: BorrowedBytes<'a>,
}

pub(crate) const SEALED: Message = Message {
    name: "test.sealed",
    cap: 64,
    budget: 64,
    secret: true,
};

impl<'a> Decode<'a> for Sealed<'a> {
    const S_MIN: u64 = 12;
    const DEPTH: u32 = 1;
    const HEAP_RATIO: Option<u64> = Some(0);
    const RESERVATIONS: Option<u64> = Some(0);

    fn decode(decoder: &mut Decoder<'a, '_>) -> Result<Self, Error> {
        Ok(Self {
            key: BorrowedBytes::new(decoder.opaque(&KEY)?),
            serial: BorrowedWord::new(decoder.word()?),
            tag: BorrowedBytes::new(decoder.opaque(&TAG)?),
        })
    }
}

impl Encode for Sealed<'_> {
    fn encoded_len(&self) -> Result<u64, Error> {
        let total = sum(opaque_len(&KEY, self.key.reveal().len())?, 4)?;
        sum(total, opaque_len(&TAG, self.tag.reveal().len())?)
    }

    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), Error> {
        encoder.put_opaque(&KEY, self.key.reveal())?;
        encoder.put_u32(self.serial.reveal_u32())?;
        encoder.put_opaque(&TAG, self.tag.reveal())
    }
}

// ---- every: one field of each profile type ----

pub(crate) const BIG_FIELD: &str = "test.every.big";
pub(crate) const BIG_LEN: usize = 100;

pub(crate) const VAR: Cap = Cap {
    name: "test.every.var",
    bound: None,
    bytes: 8,
};

pub(crate) const LIST: Cap = Cap {
    name: "test.every.list",
    bound: None,
    bytes: 16,
};

/// `every`: one field of each profile type, with a 100-byte fixed-length
/// opaque held out of line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Every {
    pub(crate) int: i32,
    pub(crate) unsigned: u32,
    pub(crate) hyper: i64,
    pub(crate) unsigned_hyper: u64,
    pub(crate) flag: bool,
    pub(crate) shade: Shade,
    pub(crate) big: Box<[u8; BIG_LEN]>,
    pub(crate) var: Vec<u8>,
    pub(crate) name: Label,
    pub(crate) fixed: [u32; 2],
    pub(crate) list: Vec<Label>,
    pub(crate) what: Entry,
}

pub(crate) const EVERY: Message = Message {
    name: "test.every",
    cap: 256,
    budget: 4096,
    secret: false,
};

impl<'a> Decode<'a> for Every {
    const S_MIN: u64 = 4 + 4 + 8 + 8 + 4 + 4 + 100 + 4 + 4 + 8 + 4 + 4;
    // The list of labels and the entry are each one level below.
    const DEPTH: u32 = 2;
    const HEAP_RATIO: Option<u64> = max(array_heap_ratio::<Label>(), Entry::HEAP_RATIO);
    const RESERVATIONS: Option<u64> = max(array_reservations::<Label>(), Entry::RESERVATIONS);

    fn decode(decoder: &mut Decoder<'a, '_>) -> Result<Self, Error> {
        Ok(Self {
            int: decoder.read_i32()?,
            unsigned: decoder.read_u32()?,
            hyper: decoder.read_i64()?,
            unsigned_hyper: decoder.read_u64()?,
            flag: decoder.read_bool("test.every.flag")?,
            shade: decoder.read_enum("test.every.shade", Shade::declared)?,
            big: decoder.fixed_opaque_owned(BIG_FIELD)?,
            var: decoder.opaque_owned(&VAR)?,
            name: Label::decode(decoder)?,
            fixed: decoder.fixed_array()?,
            list: decoder.array(&LIST)?,
            what: Entry::decode(decoder)?,
        })
    }
}

impl Plain for Every {}

impl Encode for Every {
    fn encoded_len(&self) -> Result<u64, Error> {
        let mut total = 32;
        total = sum(total, fixed_opaque_len(BIG_LEN)?)?;
        total = sum(total, opaque_len(&VAR, self.var.len())?)?;
        total = sum(total, self.name.encoded_len()?)?;
        total = sum(total, fixed_array_len(&self.fixed)?)?;
        total = sum(total, array_len(&LIST, &self.list)?)?;
        sum(total, self.what.encoded_len()?)
    }

    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), Error> {
        encoder.put_i32(self.int)?;
        encoder.put_u32(self.unsigned)?;
        encoder.put_i64(self.hyper)?;
        encoder.put_u64(self.unsigned_hyper)?;
        encoder.put_bool(self.flag)?;
        self.shade.encode(encoder)?;
        encoder.put_fixed_opaque(&self.big)?;
        encoder.put_opaque(&VAR, &self.var)?;
        self.name.encode(encoder)?;
        encoder.put_fixed_array(&self.fixed)?;
        encoder.put_array(&LIST, &self.list)?;
        self.what.encode(encoder)
    }
}

inline_fits!(
    Shade,
    Flag,
    Solo,
    Entry,
    Label,
    Note,
    Pair,
    Names,
    Tags,
    Solos,
    Ints,
    Five,
    Blob,
    Quad,
    Wide,
    Deep,
    Sealed<'static>,
    Every,
);

// What the generator proves for every message (docs/specs/xdr.md, 5.2,
// points 9.3 and 10).
const _: () = assert!(proven::<Shade>(&SHADE));
const _: () = assert!(proven::<Flag>(&FLAG));
const _: () = assert!(proven::<Entry>(&ENTRY));
const _: () = assert!(proven::<Label>(&LABEL));
const _: () = assert!(proven::<Note>(&NOTE));
const _: () = assert!(proven::<Pair>(&PAIR));
const _: () = assert!(proven::<Names>(&NAMES));
const _: () = assert!(proven::<Tags>(&TAGS));
const _: () = assert!(proven::<Solos>(&SOLOS));
const _: () = assert!(proven::<Ints>(&INTS));
const _: () = assert!(proven::<Five>(&FIVE));
const _: () = assert!(proven::<Blob>(&BLOB));
const _: () = assert!(proven::<Wide>(&WIDE));
const _: () = assert!(proven::<Deep>(&DEEP));
const _: () = assert!(proven::<Sealed<'static>>(&SEALED));
const _: () = assert!(proven::<Every>(&EVERY));
