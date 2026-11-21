// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! INIT: version negotiation and the flags the mount asks the kernel for
//! (fuse-codec.md 5.3 and 5.5, points 1 to 5).
//!
//! The reply's flags come from one place, [`select_flags`]: the kernel's
//! offer intersected with [`ALLOWED`], a reviewed constant. [`NEVER`]
//! names the flags no reply may carry (ADR 0004, L2, point 6; L4, points 1
//! to 3), and a compile-time check refuses a build in which the two share
//! a bit. A [`Negotiated`] value, the only input of the INIT reply's
//! encoder, is built only by [`negotiate`], which calls `select_flags`, so
//! no other path reaches the reply (L2, point 3).

use crate::MAX_DATA;
use crate::wire::Reader;

#[cfg(test)]
mod tests;

/// A set of INIT flags: the kernel's `flags` in the low 32 bits and its
/// `flags2` in the high 32 (fuse-codec.md 5.3, point 2).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct InitFlags(u64);

impl InitFlags {
    /// The set with these bits.
    #[must_use]
    pub const fn from_bits(bits: u64) -> Self {
        Self(bits)
    }

    /// The set's bits.
    #[must_use]
    pub const fn bits(self) -> u64 {
        self.0
    }

    /// The flags in both sets.
    #[must_use]
    pub const fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    /// Whether every flag of `other` is in this set.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// The low 32 bits, the INIT `flags` field.
    pub(crate) const fn low(self) -> u32 {
        // Truncation keeps exactly the low half, which is the point.
        #[expect(clippy::cast_possible_truncation, reason = "the low half of the set")]
        let low = self.0 as u32;
        low
    }

    /// The high 32 bits, the INIT `flags2` field.
    pub(crate) const fn high(self) -> u32 {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the high half, shifted down"
        )]
        let high = (self.0 >> 32) as u32;
        high
    }
}

impl core::fmt::Debug for InitFlags {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "InitFlags({:#x})", self.0)
    }
}

/// `FUSE_INIT_EXT`: the INIT request and reply carry `flags2`.
pub const INIT_EXT: InitFlags = InitFlags(1 << 30);
/// `FUSE_NO_EXPORT_SUPPORT` (7.40): export support is explicitly disabled.
pub const NO_EXPORT_SUPPORT: InitFlags = InitFlags(1 << 38);
/// `FUSE_WRITEBACK_CACHE`: never selected (L4, point 1).
pub const WRITEBACK_CACHE: InitFlags = InitFlags(1 << 16);
/// `FUSE_POSIX_ACL`: never selected (L4, point 3).
pub const POSIX_ACL: InitFlags = InitFlags(1 << 20);
/// `FUSE_DIRECT_IO_ALLOW_MMAP` (7.39): never selected (L4, point 2).
pub const DIRECT_IO_ALLOW_MMAP: InitFlags = InitFlags(1 << 36);
/// `FUSE_PASSTHROUGH` (7.40): never selected (L2, point 6).
pub const PASSTHROUGH: InitFlags = InitFlags(1 << 37);

/// The INIT-flag allowlist (fuse-codec.md 5.5, point 1), each flag with
/// its reason there: `FUSE_INIT_EXT`, so that `flags2` is read, and
/// `FUSE_NO_EXPORT_SUPPORT`, so that no node is reached by a file handle
/// without a LOOKUP. Every other flag is left out on purpose (5.5,
/// point 3).
pub const ALLOWED: InitFlags = InitFlags(INIT_EXT.0 | NO_EXPORT_SUPPORT.0);

/// The never-list (fuse-codec.md 5.5, point 2): the writeback cache, POSIX
/// ACLs, shared mappings in direct-I/O mode, and passthrough.
pub const NEVER: InitFlags =
    InitFlags(WRITEBACK_CACHE.0 | POSIX_ACL.0 | DIRECT_IO_ALLOW_MMAP.0 | PASSTHROUGH.0);

const _: () = assert!(
    ALLOWED.0 & NEVER.0 == 0,
    "a never-list flag is on the allowlist"
);

/// The protocol's major version, the only one the codec speaks.
pub const MAJOR: u32 = 7;
/// The floor: protocol 7.23 (ADR 0004, L2, point 2).
pub const MIN_MINOR: u32 = 23;
/// The newest minor whose interface definition the codec is written from.
pub const MAX_MINOR: u32 = 46;

/// The INIT reply's flags for the kernel's `offer`: exactly the offered
/// flags that are on [`ALLOWED`] (fuse-codec.md 5.5, point 4).
#[must_use]
pub const fn select_flags(offer: InitFlags) -> InitFlags {
    offer.intersection(ALLOWED)
}

/// The kernel's INIT request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InitRequest {
    /// The kernel's major version.
    pub major: u32,
    /// The kernel's minor version.
    pub minor: u32,
    /// The kernel's largest read-ahead, in bytes.
    pub max_readahead: u32,
    /// The flags the kernel offers.
    pub offer: InitFlags,
}

/// Decodes an INIT body (fuse-codec.md 5.1, point 4; 5.3, point 2):
/// `None` if it is shorter than 16 bytes, or than 20 when it says it
/// carries `flags2`. Bytes past the fields read are ignored.
pub(crate) fn decode_init(body: &[u8]) -> Option<InitRequest> {
    let mut reader = Reader::new(body);
    let major = reader.u32()?;
    let minor = reader.u32()?;
    let max_readahead = reader.u32()?;
    let flags = reader.u32()?;
    let mut offer = u64::from(flags);
    if offer & INIT_EXT.0 != 0 {
        offer |= u64::from(reader.u32()?) << 32;
    }
    Some(InitRequest {
        major,
        minor,
        max_readahead,
        offer: InitFlags(offer),
    })
}

/// Why an [`InitConfig`] was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigError {
    /// `max_write` is over the data cap, [`MAX_DATA`].
    MaxWrite,
}

impl core::fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::MaxWrite => "max_write is over the 1 MiB data cap",
        })
    }
}

impl core::error::Error for ConfigError {}

/// The values of the INIT reply that the layer chooses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InitConfig {
    max_readahead: u32,
    max_write: u32,
    max_background: u16,
    congestion_threshold: u16,
    time_gran: u32,
}

impl InitConfig {
    /// The layer's choices: the largest read-ahead it wants, the largest
    /// write, the kernel's background-request limit and congestion
    /// threshold, and the time granularity in nanoseconds.
    ///
    /// # Errors
    ///
    /// [`ConfigError::MaxWrite`] if `max_write` is over [`MAX_DATA`]
    /// (vfs-contract 5.2, point 3).
    pub const fn new(
        max_readahead: u32,
        max_write: u32,
        max_background: u16,
        congestion_threshold: u16,
        time_gran: u32,
    ) -> Result<Self, ConfigError> {
        if max_write > MAX_DATA {
            return Err(ConfigError::MaxWrite);
        }
        Ok(Self {
            max_readahead,
            max_write,
            max_background,
            congestion_threshold,
            time_gran,
        })
    }
}

/// What INIT settled. Only [`negotiate`] builds one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Negotiated {
    minor: u32,
    flags: InitFlags,
    pub(crate) max_readahead: u32,
    pub(crate) max_write: u32,
    pub(crate) max_background: u16,
    pub(crate) congestion_threshold: u16,
    pub(crate) time_gran: u32,
}

impl Negotiated {
    /// The negotiated minor version.
    #[must_use]
    pub const fn minor(&self) -> u32 {
        self.minor
    }

    /// The flags the reply carries, from [`select_flags`].
    #[must_use]
    pub const fn flags(&self) -> InitFlags {
        self.flags
    }
}

/// The result of an INIT the codec accepts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InitOutcome {
    /// The protocol is settled; the reply is the INIT reply.
    Ready(Negotiated),
    /// The kernel offered a newer major: the reply carries only major 7,
    /// and the kernel sends a new INIT (fuse-codec.md 5.3, point 1).
    OfferMajor,
}

/// Why the codec refused an INIT. The reply is `EPROTO`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InitRefused {
    /// A major version older than 7.
    MajorTooOld {
        /// The kernel's major.
        major: u32,
    },
    /// Major 7 below the floor, 7.23.
    MinorBelowFloor {
        /// The kernel's minor.
        minor: u32,
    },
}

impl core::fmt::Display for InitRefused {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::MajorTooOld { major } => write!(formatter, "fuse major {major} is older than 7"),
            Self::MinorBelowFloor { minor } => {
                write!(formatter, "fuse 7.{minor} is below the 7.23 floor")
            }
        }
    }
}

impl core::error::Error for InitRefused {}

/// Negotiates the protocol with the kernel's INIT (fuse-codec.md 5.3,
/// point 1): the smaller of the two minors at major 7, with the flags of
/// [`select_flags`] and the smaller of the two read-aheads.
///
/// # Errors
///
/// [`InitRefused`] for a major older than 7, or a minor below 7.23.
pub const fn negotiate(
    request: &InitRequest,
    config: &InitConfig,
) -> Result<InitOutcome, InitRefused> {
    if request.major > MAJOR {
        return Ok(InitOutcome::OfferMajor);
    }
    if request.major < MAJOR {
        return Err(InitRefused::MajorTooOld {
            major: request.major,
        });
    }
    if request.minor < MIN_MINOR {
        return Err(InitRefused::MinorBelowFloor {
            minor: request.minor,
        });
    }
    let minor = if request.minor < MAX_MINOR {
        request.minor
    } else {
        MAX_MINOR
    };
    let max_readahead = if request.max_readahead < config.max_readahead {
        request.max_readahead
    } else {
        config.max_readahead
    };
    Ok(InitOutcome::Ready(Negotiated {
        minor,
        flags: select_flags(request.offer),
        max_readahead,
        max_write: config.max_write,
        max_background: config.max_background,
        congestion_threshold: config.congestion_threshold,
        time_gran: config.time_gran,
    }))
}
