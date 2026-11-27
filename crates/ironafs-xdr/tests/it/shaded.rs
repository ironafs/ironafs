// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `shade` and `shaded` fixtures shared by the golden, sizing and
//! fuzz tests (XDR profile, section 10).

use super::xdr::{
    Classified, Decode, Decoder, Encode, Encoder, Error, Message, MessageType, Sizer,
};

/// `shade`: a closed enumeration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Shade {
    Pale,
    Deep,
    Vivid,
}

pub(crate) const SHADE: Message = Message {
    name: "test.shade",
    cap: 128,
    budget: 1024,
};

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

impl Classified for Shade {
    const SECRET: bool = false;
}

impl Encode for Shade {
    fn size(&self, sizer: &Sizer) -> Result<(), Error> {
        sizer.charge(Ok(4))
    }

    fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
        encoder.put_i32(self.value())
    }
}

impl MessageType for Shade {
    const MESSAGE: Message = SHADE;
}

// ---- shaded: a closed discriminant with a default arm ----

/// `shaded`: only VIVID selects the default arm.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Shaded {
    Pale(u64),
    Deep(u64),
    Default(Shade, u64),
}

pub(crate) const SHADED: Message = Message {
    name: "test.shaded",
    cap: 12,
    budget: 1024,
};

const SHADED_TONE: &str = "test.shaded.tone";

/// The selected layout, before its payload is read.
pub(crate) enum ShadedArm {
    Pale,
    Deep,
    Default(Shade),
}

impl ShadedArm {
    /// Selects an explicit arm first, then the default, for a declared tone.
    pub(crate) const fn select(tone: Shade) -> Self {
        match tone {
            Shade::Pale => Self::Pale,
            Shade::Deep => Self::Deep,
            Shade::Vivid => Self::Default(tone),
        }
    }

    pub(crate) const fn with_value(self, value: u64) -> Shaded {
        match self {
            Self::Pale => Shaded::Pale(value),
            Self::Deep => Shaded::Deep(value),
            Self::Default(tone) => Shaded::Default(tone, value),
        }
    }
}

impl Shaded {
    /// The discriminant and payload to size or write, after arm selection.
    fn parts(self) -> Result<(Shade, u64), Error> {
        match self {
            Self::Pale(value) => Ok((Shade::Pale, value)),
            Self::Deep(value) => Ok((Shade::Deep, value)),
            Self::Default(tone, value) => match ShadedArm::select(tone) {
                ShadedArm::Default(_) => Ok((tone, value)),
                _ => Err(Error::default_arm_mismatch(SHADED_TONE)),
            },
        }
    }
}

impl<'a> Decode<'a> for Shaded {
    const S_MIN: u64 = 12;
    const DEPTH: u32 = 1;
    const HEAP_RATIO: Option<u64> = Some(0);
    const RESERVATIONS: Option<u64> = Some(0);

    fn decode(decoder: &mut Decoder<'a, '_>) -> Result<Self, Error> {
        let tone = decoder.read_enum(SHADED_TONE, Shade::declared)?;
        let arm = ShadedArm::select(tone);
        Ok(arm.with_value(decoder.read_u64()?))
    }
}

impl Classified for Shaded {
    const SECRET: bool = false;
}

impl Encode for Shaded {
    fn size(&self, sizer: &Sizer) -> Result<(), Error> {
        let (tone, value) = sizer.check(self.parts())?;
        sizer.put(&tone)?;
        sizer.put(&value)
    }

    fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
        let (tone, value) = self.parts()?;
        encoder.put(&tone)?;
        encoder.put_u64(value)
    }
}

impl MessageType for Shaded {
    const MESSAGE: Message = SHADED;
}
