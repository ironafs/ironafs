// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! A simulated cell: its VL servers, which share one database, and its
//! fileservers, each reached by address and service. A server that is
//! down, or an address no server holds, drops every request, so its
//! caller's wait ends at its deadline.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, PoisonError};

use super::codes::MARSHALLING;
use super::envelope::{decode_request, destination, encode_answer};
use super::fs::FileServer;
use super::types::{Addr, Answer, Request, Service, Site, SiteServer, VolumeId};
use super::vl::VlDb;
use crate::sched::Setup;

#[cfg(test)]
mod tests;

/// A simulated cell.
#[derive(Clone, Debug, Default)]
pub struct Cell {
    vldb: VlDb,
    vl_servers: BTreeSet<Addr>,
    file_servers: BTreeMap<Addr, FileServer>,
    down: BTreeSet<Addr>,
}

impl Cell {
    /// A cell with no server.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a VL server at `addr`, serving the cell's database.
    pub fn add_vl_server(&mut self, addr: Addr) {
        self.vl_servers.insert(addr);
    }

    /// The cell's VL database.
    pub fn vldb(&mut self) -> &mut VlDb {
        &mut self.vldb
    }

    /// Adds a fileserver, at its own address.
    pub fn add_file_server(&mut self, server: FileServer) {
        self.file_servers.insert(server.addr(), server);
    }

    /// The fileserver at `addr`.
    pub fn file_server(&mut self, addr: Addr) -> Option<&mut FileServer> {
        self.file_servers.get_mut(&addr)
    }

    /// Takes the server at `addr` down, or brings it back.
    pub fn set_down(&mut self, addr: Addr, down: bool) {
        if down {
            self.down.insert(addr);
        } else {
            self.down.remove(&addr);
        }
    }

    /// Moves volume `volume`, named `name` in the VL database, from the
    /// fileserver holding it to the one at `to`: the old server answers
    /// "moved" for it, and the VL entry's sites name the new server.
    /// `false` if the volume, the entry or the target is unknown.
    pub fn move_volume(&mut self, name: &[u8], volume: VolumeId, to: Addr) -> bool {
        let Some(entry) = self.vldb.entry(name) else {
            return false;
        };
        if !self.file_servers.contains_key(&to) {
            return false;
        }
        let Some(from) = self
            .file_servers
            .values_mut()
            .find(|server| server.holds(volume))
        else {
            return false;
        };
        let Some(moved) = from.export_volume(volume) else {
            return false;
        };
        let from = from.addr();
        let sites = entry
            .sites
            .iter()
            .map(|site| match site.server {
                SiteServer::Addr(addr) if addr == from => Site {
                    server: SiteServer::Addr(to),
                    ..*site
                },
                _ => *site,
            })
            .collect();
        self.vldb.set_sites(name, sites);
        if let Some(target) = self.file_servers.get_mut(&to) {
            target.import_volume(moved);
        }
        true
    }

    /// Whether a server is up at `addr`.
    fn reaches(&self, addr: Addr) -> bool {
        !self.down.contains(&addr)
            && (self.vl_servers.contains(&addr) || self.file_servers.contains_key(&addr))
    }

    /// The answer to `request`, or `None` if it is dropped.
    pub fn serve(&mut self, request: &Request) -> Option<Answer> {
        if self.down.contains(&request.to) {
            return None;
        }
        match request.service {
            Service::Vl if self.vl_servers.contains(&request.to) => {
                Some(self.vldb.serve(&request.procedure))
            }
            Service::Vl => None,
            Service::Fs => self
                .file_servers
                .get_mut(&request.to)
                .map(|server| server.serve(request.client, request.context, &request.procedure)),
        }
    }

    /// The answer to a request's envelope bytes, or `None` if it is
    /// dropped. The destination is read first: bytes too short to name
    /// one, or to a server that is down or absent, are dropped unread.
    /// Bytes that reach a server but are no request get the marshalling
    /// abort (5.9, row R5).
    pub fn serve_bytes(&mut self, bytes: &[u8]) -> Option<Vec<u8>> {
        if !self.reaches(destination(bytes)?) {
            return None;
        }
        let answer = match decode_request(bytes) {
            Ok(request) => self.serve(&request)?,
            Err(_) => Answer::Abort(MARSHALLING),
        };
        Some(encode_answer(&answer))
    }
}

/// Makes `cell` the server model of a run: every [`crate::sched::TaskCx::call`]
/// is a request in the envelope, answered or dropped by the cell.
pub fn serve_in(setup: &mut Setup, cell: &Arc<Mutex<Cell>>) {
    let cell = Arc::clone(cell);
    setup.server_may_drop(move |request: &[u8]| {
        cell.lock()
            .unwrap_or_else(PoisonError::into_inner)
            .serve_bytes(request)
    });
}
