// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! A simulated cell: its VL servers, which share one database, and its
//! fileservers, each reached by address and service. A server that is
//! down, or an address no server holds, drops every request, so its
//! caller's wait ends at its deadline.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use super::callback::{CbAnswer, CbKind, CbRequest, decode_cb_answer, encode_cb_request};
use super::codes::{MARSHALLING, UNKNOWN_PROCEDURE};
use super::envelope::{decode_request, destination, encode_answer};
use super::fs::FileServer;
use super::types::{
    Addr, Answer, ClientId, Procedure, Request, Results, Service, Site, SiteServer, VolumeId,
};
use super::vl::VlDb;
use crate::sched::{Polled, Push, Served, Setup, World};

#[cfg(test)]
mod tests;

/// A simulated cell.
#[derive(Clone, Debug, Default)]
pub struct Cell {
    vldb: VlDb,
    vl_servers: BTreeSet<Addr>,
    file_servers: BTreeMap<Addr, FileServer>,
    down: BTreeSet<Addr>,
    /// Callback calls in flight, by the token the scheduler knows them by.
    pushes: BTreeMap<u64, Origin>,
    next_token: u64,
    /// Forged calls not yet handed to the scheduler.
    forged: Vec<CbRequest>,
    forged_answers: Vec<(CbRequest, Option<CbAnswer>)>,
    /// The network's fault plan: the first attempts of each callback call
    /// of this kind to this client that it loses.
    faults: BTreeMap<(ClientId, CbKind), u32>,
}

/// Who sent a callback call: a fileserver, by its own number for the
/// call, or the network forging one.
#[derive(Clone, Debug)]
struct Origin {
    server: Option<(Addr, u64)>,
    request: CbRequest,
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

    /// Makes the network lose the first `attempts` attempts of every
    /// callback call of `kind` to `to`: a dropped break, or with as many
    /// as a call makes, a break lost in every retransmission.
    pub fn drop_callbacks(&mut self, to: ClientId, kind: CbKind, attempts: u32) {
        self.faults.insert((to, kind), attempts);
    }

    /// Each forged call that settled, with its answer, or `None` if it
    /// got none.
    #[must_use]
    pub fn forged_answers(&self) -> &[(CbRequest, Option<CbAnswer>)] {
        &self.forged_answers
    }

    /// Whether a server is up at `addr`.
    fn reaches(&self, addr: Addr) -> bool {
        addr == NETWORK
            || !self.down.contains(&addr)
                && (self.vl_servers.contains(&addr) || self.file_servers.contains_key(&addr))
    }

    /// The answer to `request`, or `None` if it is dropped.
    pub fn serve(&mut self, request: &Request) -> Option<Answer> {
        if request.to == NETWORK {
            let Procedure::SimForge {
                from,
                to,
                procedure,
            } = &request.procedure
            else {
                return Some(Answer::Abort(UNKNOWN_PROCEDURE));
            };
            self.forged.push(CbRequest {
                from: *from,
                to: *to,
                procedure: procedure.clone(),
            });
            return Some(Answer::Result(Results::Started { pushes: 1 }));
        }
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
        match self.serve_envelope(bytes, None) {
            Served::Reply(reply) => Some(reply),
            Served::Drop | Served::Hold => None,
        }
    }

    /// The cell's one serving policy for envelope bytes, shared by
    /// [`Cell::serve_bytes`] and the world path: the destination first,
    /// then the decode, then the route. With `call`, the bytes are a
    /// task's call through the world, which a fileserver may hold (A14).
    fn serve_envelope(&mut self, bytes: &[u8], call: Option<u64>) -> Served {
        if !destination(bytes).is_some_and(|to| self.reaches(to)) {
            return Served::Drop;
        }
        let Ok(request) = decode_request(bytes) else {
            return Served::Reply(encode_answer(&Answer::Abort(MARSHALLING)));
        };
        if let Some(call) = call
            && request.service == Service::Fs
            && let Some(server) = self.file_servers.get_mut(&request.to)
        {
            return server
                .serve_call(call, request.client, request.context, &request.procedure)
                .map_or(Served::Hold, |answer| Served::Reply(encode_answer(&answer)));
        }
        self.serve(&request)
            .map_or(Served::Drop, |answer| Served::Reply(encode_answer(&answer)))
    }
}

/// The network's own address, the simulator's: the one destination of
/// `sim.forge`, which no server holds. Every other request sent to it
/// is answered with the unknown-procedure abort.
pub const NETWORK: Addr = Addr([0, 0, 0, 0]);

/// How many times a callback call is sent before it is given up: the
/// model's own value, since no specification states a server's.
pub const PUSH_ATTEMPTS: u32 = 3;

/// The time between a lost attempt of a callback call and the next: the
/// model's own value.
pub const PUSH_INTERVAL: Duration = Duration::from_secs(1);

/// The time after which a callback call with no answer has failed: the
/// model's own value, past every attempt.
pub const PUSH_TIMEOUT: Duration = Duration::from_secs(5);

/// A cell as the simulator's world: it answers, drops or holds the
/// tasks' calls, and sends the callback calls its fileservers and the
/// network start, each to the endpoint numbered by its client host.
#[derive(Debug)]
pub struct CellWorld(Arc<Mutex<Cell>>);

impl CellWorld {
    /// The world of `cell`.
    #[must_use]
    pub fn new(cell: &Arc<Mutex<Cell>>) -> Self {
        Self(Arc::clone(cell))
    }

    fn cell(&self) -> MutexGuard<'_, Cell> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl World for CellWorld {
    fn serve(&mut self, call: u64, bytes: &[u8]) -> Served {
        self.cell().serve_envelope(bytes, Some(call))
    }

    fn admit(&mut self, token: u64, attempt: u32) -> bool {
        let cell = self.cell();
        let Some(origin) = cell.pushes.get(&token) else {
            return true;
        };
        let key = (origin.request.to, origin.request.procedure.kind());
        attempt > cell.faults.get(&key).copied().unwrap_or(0)
    }

    fn settled(&mut self, token: u64, response: Option<Vec<u8>>) {
        let mut cell = self.cell();
        let Some(origin) = cell.pushes.remove(&token) else {
            return;
        };
        // An answer that is no envelope still shows the call arrived.
        let answer =
            response.map(|bytes| decode_cb_answer(&bytes).unwrap_or(CbAnswer::Abort(MARSHALLING)));
        match origin.server {
            Some((addr, id)) => {
                if let Some(server) = cell.file_servers.get_mut(&addr) {
                    server.settled(id, answer);
                }
            }
            None => cell.forged_answers.push((origin.request, answer)),
        }
    }

    fn poll(&mut self) -> Polled {
        let mut cell = self.cell();
        let cell = &mut *cell;
        let mut started: Vec<Origin> = Vec::new();
        let mut releases = Vec::new();
        for (addr, server) in &mut cell.file_servers {
            started.extend(server.take_outgoing().into_iter().map(|out| Origin {
                server: Some((*addr, out.id)),
                request: out.request,
            }));
            releases.extend(
                server
                    .take_released()
                    .into_iter()
                    .map(|(call, answer)| (call, answer.as_ref().map(encode_answer))),
            );
        }
        started.extend(cell.forged.drain(..).map(|request| Origin {
            server: None,
            request,
        }));
        let mut pushes = Vec::with_capacity(started.len());
        for origin in started {
            let token = cell.next_token;
            cell.next_token = token.saturating_add(1);
            pushes.push(Push {
                token,
                endpoint: origin.request.to.0,
                request: encode_cb_request(&origin.request),
                attempts: PUSH_ATTEMPTS,
                interval: PUSH_INTERVAL,
                timeout: PUSH_TIMEOUT,
            });
            cell.pushes.insert(token, origin);
        }
        Polled { pushes, releases }
    }
}

/// Makes `cell` the world of a run: every [`crate::sched::TaskCx::call`]
/// is a request in the envelope, answered, dropped or held by the cell,
/// and the callback calls it starts go to the tasks' `accept`.
pub fn serve_in(setup: &mut Setup, cell: &Arc<Mutex<Cell>>) {
    setup.world(CellWorld::new(cell));
}
