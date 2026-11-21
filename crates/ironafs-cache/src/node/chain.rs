// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The anchor chain of a volume root: the mount points crossed since its
//! cell root (namespace specification, section 4 and 5.6.1).

use crate::FileId;

use super::NodeError;

#[cfg(test)]
mod tests;

/// The most mount points a chain holds, `ns.mount.depth_max` (namespace
/// specification, 5.2): crossing 32 is allowed, crossing the 33rd is
/// refused.
pub const MAX_MOUNT_DEPTH: usize = 32;

/// The mount-point identities crossed since a cell root, oldest first, at
/// most [`MAX_MOUNT_DEPTH`], held inline (5.6.1, point 4). A cell root has
/// the empty chain.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MountChain {
    len: u8,
    ids: [FileId; MAX_MOUNT_DEPTH],
}

impl MountChain {
    /// The empty chain of a cell root.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            len: 0,
            ids: [FileId {
                cell: crate::CellIndex(0),
                volume: crate::VolumeId(0),
                vnode: crate::Vnode(0),
                uniquifier: crate::Uniquifier(0),
            }; MAX_MOUNT_DEPTH],
        }
    }

    /// The chain's mount points, oldest first.
    #[must_use]
    pub fn ids(&self) -> &[FileId] {
        self.ids.get(..usize::from(self.len)).unwrap_or(&[])
    }

    /// The depth: how many mount points the chain holds.
    #[must_use]
    pub const fn depth(&self) -> usize {
        self.len as usize
    }

    /// This chain followed by `mount_point`, the chain of the root entered
    /// by crossing it (5.6.1, point 1).
    ///
    /// # Errors
    ///
    /// [`NodeError::MountDepthExceeded`] if the chain already holds
    /// [`MAX_MOUNT_DEPTH`] mount points, checked first; then
    /// [`NodeError::MountCycle`] if `mount_point` is already in it.
    pub fn extended(&self, mount_point: FileId) -> Result<Self, NodeError> {
        if self.depth() >= MAX_MOUNT_DEPTH {
            return Err(NodeError::MountDepthExceeded);
        }
        if self.ids().contains(&mount_point) {
            return Err(NodeError::MountCycle);
        }
        let mut next = *self;
        let slot = next
            .ids
            .get_mut(usize::from(self.len))
            .ok_or(NodeError::MountDepthExceeded)?;
        *slot = mount_point;
        next.len = self
            .len
            .checked_add(1)
            .ok_or(NodeError::MountDepthExceeded)?;
        Ok(next)
    }
}
