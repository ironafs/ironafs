// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The volume location model: a cell's VL database, serving the two
//! lookups and the address list (`docs/specs/vl-fs-read.md`, 5.1.2 and
//! 5.1.4, points 1 to 3), with the abort codes of 5.9's VL rows.

use std::collections::{BTreeMap, BTreeSet};

use super::codes::{UNKNOWN_PROCEDURE, VL_NO_ENTRY};
use super::types::{Addr, Answer, Busy, ProcName, Procedure, Results, ServerUuid, Site, VlEntry};

#[cfg(test)]
mod tests;

/// A cell's volume location database.
#[derive(Clone, Debug, Default)]
pub struct VlDb {
    entries: Vec<VlEntry>,
    servers: BTreeMap<ServerUuid, Vec<Addr>>,
    refused: BTreeSet<ProcName>,
    busy: Option<Busy>,
}

impl VlDb {
    /// An empty database.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an entry, replacing any entry of the same name.
    pub fn add_volume(&mut self, entry: VlEntry) {
        self.entries.retain(|known| known.name != entry.name);
        self.entries.push(entry);
    }

    /// Records the addresses of the server with identity `uuid`.
    pub fn add_server(&mut self, uuid: ServerUuid, addrs: Vec<Addr>) {
        self.servers.insert(uuid, addrs);
    }

    /// Replaces the sites of the volume named `name`, as a move or a
    /// release does; `false` if there is no such entry.
    pub fn set_sites(&mut self, name: &[u8], sites: Vec<Site>) -> bool {
        match self.entries.iter_mut().find(|entry| entry.name == name) {
            Some(entry) => {
                entry.sites = sites;
                true
            }
            None => false,
        }
    }

    /// The entry named `name`.
    #[must_use]
    pub fn entry(&self, name: &[u8]) -> Option<&VlEntry> {
        self.entries.iter().find(|entry| entry.name == name)
    }

    /// Makes the database answer `procedure` with the "unknown procedure"
    /// abort (5.9, row R6).
    pub fn refuse(&mut self, procedure: ProcName) {
        self.refused.insert(procedure);
    }

    /// Answers the next requests with a busy abort.
    pub fn busy(&mut self, busy: Busy) {
        self.busy = Some(busy);
    }

    /// The answer to `procedure`: a busy spell first, then a refusal,
    /// then the lookup. A file-service procedure is unknown here.
    pub fn serve(&mut self, procedure: &Procedure) -> Answer {
        if let Some(code) = Busy::take(&mut self.busy) {
            return Answer::Abort(code);
        }
        if self.refused.contains(&procedure.name()) {
            return Answer::Abort(UNKNOWN_PROCEDURE);
        }
        let found = match procedure {
            Procedure::VlGetEntryByName { name } => self.entry(name).cloned().map(Results::VlEntry),
            Procedure::VlGetEntryById { volume, kind } => self
                .entries
                .iter()
                .find(|entry| entry.ids.of(*kind) == Some(*volume))
                .cloned()
                .map(Results::VlEntry),
            Procedure::VlGetAddrs { server } => {
                self.servers.get(server).cloned().map(Results::Addrs)
            }
            _ => return Answer::Abort(UNKNOWN_PROCEDURE),
        };
        // The specification lists no code for an unknown identity
        // (5.9, row R24: any code makes that site unusable); the model
        // uses "no such entry" for it as for an unknown volume.
        found.map_or(Answer::Abort(VL_NO_ENTRY), Answer::Result)
    }
}
