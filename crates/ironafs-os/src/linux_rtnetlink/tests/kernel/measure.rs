// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The measurements of `docs/specs/network-generation.md`, section 13,
//! made through row 23's site alone (no new call): Q5, the notify socket's
//! bind with the nexthop bit and what it then receives; Q7, the round trip
//! of each dump and of the barrier behind a backlog, with the caps' worst
//! case of links and addresses; Q8, every wait's report with the receive
//! that followed, over overflows and churn; and Q9, the duration of each
//! send while another process keeps the routing tables busy.
//!
//! Each is an ignored test, run as root in a network namespace of its own
//! (`unshare --net`), one at a time, with `--nocapture`: it prints one
//! summary line per result, with key=value fields. Q7 also prints one line
//! per barrier attempt. The tests fail only when the measurement could not
//! be made. Counts and durations come from `IRONAFS_MEASURE_ROUNDS` (Q7 and
//! Q9; 1–1,000, default 1,000) and `IRONAFS_MEASURE_SECONDS` (Q8;
//! 1–4,800, default 3,600). Q7 and Q9 run both churn modes by default.
//! `IRONAFS_MEASURE_CHURN` selects only `none` or `routes`.
//! Times are read from the awake clock (`CLOCK_MONOTONIC`), in
//! nanoseconds.

use std::fmt::Write as _;
use std::fs;
use std::io::Write as _;
use std::os::fd::AsRawFd;
use std::process::{Child, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use super::super::super::{RouteSocket, Wake};
use super::{
    AF_INET, AF_INET6, HEADER, NLM_F_REQUEST, NLMSG_DONE, NLMSG_ERROR, RTM_GETADDR, RTM_GETLINK,
    RTM_NEWADDR, RTM_NEWLINK, RTM_NEWROUTE, RTM_NEWRULE, dump, events, ip, messages, now, object,
};
use crate::dump::harness::Command;
use crate::route::{Ready, Receipt, RouteError, Waited};

/// `NLMSG_NOOP`, and `NLM_F_ACK`: the barrier is a no-op that asks for an
/// acknowledgement (`netlink(7)`).
const NLMSG_NOOP: u16 = 1;
const NLM_F_ACK: u16 = 4;

/// The caps' worst case (spec 5.5, point 3).
const LINKS: usize = 1024;
const ADDRESSES: usize = 4096;

/// How long one round waits for its answer before it is a failed round:
/// the provisional deadlines' 10 seconds (spec 5.8, point 8), past which
/// the monitor would have ended the attempt anyway.
const ROUND_LIMIT: Duration = Duration::from_secs(10);

/// The awake time allowed to drain queued notifications before a barrier.
const PREP_DRAIN_LIMIT_NS: u64 = 100_000_000;

/// The receive buffer, as the monitor's (spec 5.5, point 1).
const BUFFER: usize = 65_536;

/// The kernel's release, as `uname -r` prints it.
fn kernel() -> String {
    fs::read_to_string("/proc/sys/kernel/osrelease")
        .unwrap()
        .trim()
        .to_owned()
}

/// A positive number up to the measurement's budget, or the default if unset.
fn setting(name: &str, default: u64, max: u64) -> u64 {
    match std::env::var(name) {
        Ok(value) => value
            .parse()
            .ok()
            .filter(|value| (1..=max).contains(value))
            .unwrap_or_else(|| panic!("{name} must be an integer from 1 through {max}")),
        Err(std::env::VarError::NotPresent) => default,
        Err(error) => panic!("cannot read {name}: {error}"),
    }
}

/// Runs `lines` through `ip -batch -`, which must succeed.
fn batch(lines: &str) {
    let mut child = Command::new("ip")
        .args(["-batch", "-"])
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(lines.as_bytes())
        .unwrap();
    assert!(child.wait().unwrap().success(), "ip -batch -");
}

/// A request of `kind` with sequence `sequence` and `flags`.
fn request(kind: u16, flags: u16, sequence: u32) -> Vec<u8> {
    let mut bytes = if kind == NLMSG_NOOP {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&16_u32.to_ne_bytes());
        bytes.extend_from_slice(&kind.to_ne_bytes());
        bytes.extend_from_slice(&flags.to_ne_bytes());
        bytes.extend_from_slice(&0_u32.to_ne_bytes());
        bytes.extend_from_slice(&0_u32.to_ne_bytes());
        bytes
    } else {
        dump(kind)
    };
    bytes[8..12].copy_from_slice(&sequence.to_ne_bytes());
    bytes
}

/// The `NLMSG_ERROR` answers in one datagram: each its error and the
/// sequence of the header it echoes.
fn answers(datagram: &[u8]) -> Vec<(i32, u32)> {
    messages(datagram)
        .into_iter()
        .filter(|(kind, payload)| *kind == NLMSG_ERROR && payload.len() >= 4 + HEADER)
        .map(|(_, payload)| {
            (
                i32::from_ne_bytes(payload[0..4].try_into().unwrap()),
                u32::from_ne_bytes(payload[12..16].try_into().unwrap()),
            )
        })
        .collect()
}

/// The sequence of each `NLMSG_DONE` in one datagram, and how many
/// messages of `kind` it holds.
fn dones_and_count(datagram: &[u8], kind: u16) -> (Vec<u32>, usize) {
    let mut dones = Vec::new();
    let mut count = 0;
    let mut rest = datagram;
    while rest.len() >= HEADER {
        let length = u32::from_ne_bytes(rest[0..4].try_into().unwrap()) as usize;
        let found = u16::from_ne_bytes(rest[4..6].try_into().unwrap());
        if found == NLMSG_DONE {
            dones.push(u32::from_ne_bytes(rest[8..12].try_into().unwrap()));
        } else if found == kind {
            count += 1;
        }
        rest = &rest[((length + 3) & !3).min(rest.len())..];
    }
    (dones, count)
}

/// The outcome of one waited receive: a datagram's length, or the error.
enum Got {
    Datagram(usize),
    Error(RouteError),
    Nothing,
}

/// One wait on `socket` (with a wake no one signals) for at most `limit`,
/// then, if the socket was reported, one receive.
fn waited_receive(socket: &RouteSocket, wake: &Wake, buffer: &mut [u8], limit: Duration) -> Got {
    match socket.wait(wake, limit) {
        Ok(Waited::Ready(ready)) if ready.socket() => match socket.receive(buffer) {
            Ok(Receipt::Datagram(count)) => Got::Datagram(count),
            Ok(Receipt::NothingQueued | Receipt::Interrupted) => Got::Nothing,
            Err(error) => Got::Error(error),
        },
        Ok(_) => Got::Nothing,
        Err(error) => Got::Error(error),
    }
}

/// The longest, median and 99th percentile of `samples`, in nanoseconds.
fn summary(samples: &mut [u64]) -> String {
    if samples.is_empty() {
        return "n=0".to_owned();
    }
    samples.sort_unstable();
    let at = |fraction: f64| {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss,
            reason = "a percentile's index into a measurement's samples"
        )]
        let index = ((samples.len() - 1) as f64 * fraction).round() as usize;
        samples[index]
    };
    format!(
        "n={} p50_ns={} p99_ns={} max_ns={}",
        samples.len(),
        at(0.5),
        at(0.99),
        samples[samples.len() - 1]
    )
}

/// The caps' worst case in this namespace: `LINKS` links (lo and dummy
/// links, down, so they hold no link-local address) and `ADDRESSES`
/// addresses (lo's two and IPv6 addresses on the dummies).
fn worst_case() {
    ip("link set lo up");
    let mut lines = String::new();
    for index in 0..LINKS - 1 {
        writeln!(lines, "link add d{index} type dummy").unwrap();
    }
    batch(&lines);
    let mut lines = String::new();
    for index in 0..ADDRESSES - 2 {
        let link = index % (LINKS - 1);
        writeln!(
            lines,
            "address add 2001:db8:{:x}::{:x}/128 dev d{link}",
            index / 0x1_0000,
            index % 0x1_0000 + 1
        )
        .unwrap();
    }
    batch(&lines);
}

/// A process that adds and removes routes in a loop until it is dropped:
/// the route churn of Q7 and Q9.
struct Churn {
    stop: Arc<AtomicBool>,
    writer: Option<thread::JoinHandle<()>>,
    child: Child,
}

impl Churn {
    fn start() -> Self {
        let mut child = Command::new("ip")
            .args(["-force", "-batch", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut input = child.stdin.take().unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = Arc::clone(&stop);
        let writer = thread::spawn(move || {
            let mut lines = String::new();
            for index in 0..2000 {
                writeln!(
                    lines,
                    "route add 198.18.{}.{}/32 dev lo",
                    index / 250,
                    index % 250
                )
                .unwrap();
            }
            for index in 0..2000 {
                writeln!(
                    lines,
                    "route del 198.18.{}.{}/32 dev lo",
                    index / 250,
                    index % 250
                )
                .unwrap();
            }
            while !stopping.load(Ordering::Relaxed) {
                if input.write_all(lines.as_bytes()).is_err() {
                    return;
                }
            }
        });
        Self {
            stop,
            writer: Some(writer),
            child,
        }
    }
}

impl Drop for Churn {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(writer) = self.writer.take() {
            let _ = writer.join();
        }
        let _ = self.child.wait();
    }
}

/// The bytes queued on the socket whose descriptor is `descriptor`
/// (`/proc/net/netlink`'s `Rmem`, found by the socket's inode).
fn queued(descriptor: i32) -> Option<u64> {
    let link = fs::read_link(format!("/proc/self/fd/{descriptor}")).ok()?;
    let inode = link
        .to_str()?
        .strip_prefix("socket:[")?
        .strip_suffix(']')?
        .to_owned();
    let table = fs::read_to_string("/proc/net/netlink").ok()?;
    table.lines().skip(1).find_map(|line| {
        let fields: Vec<&str> = line.split_whitespace().collect();
        (fields.get(9) == Some(&inode.as_str()))
            .then(|| fields.get(4).and_then(|rmem| rmem.parse().ok()))
            .flatten()
    })
}

/// The notify socket's receive buffer, as the kernel sizes it.
fn receive_buffer() -> u64 {
    fs::read_to_string("/proc/sys/net/core/rmem_default")
        .unwrap()
        .trim()
        .parse()
        .unwrap()
}

/// What a wait reported and the receive that followed show: `None` if the
/// report is explained, or why it is not (ADR 0005, sheet 23: `POLLERR` is
/// explained only by an error return of that receive, an overflow or
/// another failed receive; `POLLHUP` by no receive outcome).
fn unexplained(ready: Ready, outcome: &Result<Receipt, RouteError>) -> Option<&'static str> {
    if ready.hangup {
        return Some("hangup");
    }
    let error_return = matches!(
        outcome,
        Err(RouteError::Overflow(_) | RouteError::Receive(_))
    );
    (ready.error && !error_return).then_some("error with no error return")
}

#[test]
fn the_q8_logger_finds_each_report_its_receive_does_not_explain() {
    let error = Ready {
        error: true,
        ..Ready::default()
    };
    let hangup = Ready {
        hangup: true,
        ..Ready::default()
    };
    let readable = Ready {
        readable: true,
        ..Ready::default()
    };
    let overflow = || Err(RouteError::Overflow(std::io::Error::from_raw_os_error(105)));
    assert_eq!(unexplained(error, &overflow()), None);
    assert_eq!(
        unexplained(
            error,
            &Err(RouteError::Receive(std::io::Error::from_raw_os_error(9)))
        ),
        None
    );
    assert_eq!(unexplained(readable, &Ok(Receipt::Datagram(20))), None);
    assert_eq!(unexplained(readable, &Ok(Receipt::NothingQueued)), None);
    for outcome in [
        Ok(Receipt::Datagram(20)),
        Ok(Receipt::NothingQueued),
        Err(RouteError::Empty),
    ] {
        assert_eq!(
            unexplained(error, &outcome),
            Some("error with no error return")
        );
    }
    assert_eq!(unexplained(hangup, &overflow()), Some("hangup"));
    assert_eq!(
        unexplained(hangup, &Ok(Receipt::NothingQueued)),
        Some("hangup")
    );
}

#[test]
#[ignore = "a measurement (spec Q5): as root in a network namespace of its own, with iproute2"]
fn q5_the_notify_socket_binds_with_the_nexthop_bit_and_receives_each_kind() {
    ip("link set lo up");
    let bound = RouteSocket::notify();
    let mut line = format!("measure q5 kernel={} bind=", kernel());
    match &bound {
        Ok(_) => line.push_str("ok"),
        Err(error) => write!(line, "failed({error})").unwrap(),
    }
    let Ok(socket) = bound else {
        println!("{line}");
        return;
    };
    let wake = Wake::new().unwrap();
    let mut buffer = vec![0_u8; BUFFER];
    for (name, add, delete, wanted) in [
        (
            "links",
            "link set lo mtu 1500",
            "link set lo mtu 65536",
            super::link_mtu(1500),
        ),
        (
            "addresses",
            "address add 192.0.2.1/24 dev lo",
            "address del 192.0.2.1/24 dev lo",
            object(RTM_NEWADDR, AF_INET, "192.0.2.1", 24),
        ),
        (
            "routes",
            "-6 route add 2001:db8:2::/64 dev lo",
            "-6 route del 2001:db8:2::/64 dev lo",
            object(RTM_NEWROUTE, AF_INET6, "2001:db8:2::", 64),
        ),
        (
            "rules",
            "rule add from 203.0.113.0/24 table 100",
            "rule del from 203.0.113.0/24 table 100",
            object(RTM_NEWRULE, AF_INET, "203.0.113.0", 24),
        ),
    ] {
        ip(add);
        let started = now();
        let mut seen = false;
        while !seen && now() - started < 5_000_000_000 {
            if let Got::Datagram(count) =
                waited_receive(&socket, &wake, &mut buffer, Duration::from_secs(1))
            {
                seen = events(&buffer[..count]).contains(&wanted);
            }
        }
        ip(delete);
        write!(line, " {name}={}", if seen { "seen" } else { "missed" }).unwrap();
    }
    // The barrier's acknowledgement on this kernel (spec, open items).
    let started = now();
    socket
        .send(&request(NLMSG_NOOP, NLM_F_REQUEST | NLM_F_ACK, 7))
        .unwrap();
    let mut acknowledged = false;
    while !acknowledged && now() - started < 5_000_000_000 {
        if let Got::Datagram(count) =
            waited_receive(&socket, &wake, &mut buffer, Duration::from_secs(1))
        {
            acknowledged = answers(&buffer[..count]).contains(&(0, 7));
        }
    }
    let nexthop = Command::new("ip")
        .args(["nexthop", "add", "id", "7", "dev", "lo"])
        .stderr(Stdio::null())
        .status()
        .unwrap()
        .success();
    write!(
        line,
        " noop_ack={} nexthop_objects={}",
        if acknowledged { "yes" } else { "no" },
        if nexthop { "yes" } else { "no" }
    )
    .unwrap();
    println!("{line}");
}

/// One dump round trip on `socket`: from the send to the `NLMSG_DONE`
/// that answers `sequence`, the send's own duration, and the count of
/// `kind` messages; `None` for a failed round.
fn dump_round(
    socket: &RouteSocket,
    wake: &Wake,
    buffer: &mut [u8],
    kind: u16,
    sequence: u32,
) -> Option<(u64, u64, usize)> {
    let message = request(kind, NLM_F_REQUEST | 0x300, sequence);
    let started = now();
    if socket.send(&message).is_err() {
        return None;
    }
    let sent = now() - started;
    let answered = if kind == RTM_GETLINK {
        RTM_NEWLINK
    } else {
        RTM_NEWADDR
    };
    let mut count = 0;
    loop {
        if now() - started > u64::try_from(ROUND_LIMIT.as_nanos()).unwrap() {
            return None;
        }
        match waited_receive(socket, wake, buffer, Duration::from_secs(1)) {
            Got::Datagram(length) => {
                let (dones, found) = dones_and_count(&buffer[..length], answered);
                count += found;
                if dones.contains(&sequence) {
                    return Some((now() - started, sent, count));
                }
            }
            Got::Error(_) => return None,
            Got::Nothing => {}
        }
    }
}

/// How a barrier attempt ended, with its elapsed time on acknowledgement.
enum BarrierOutcome {
    Acknowledged(u64),
    TimedOut,
    SendFailed,
}

/// A barrier attempt with its send duration, pre-send queue fill, and
/// overflow errors observed while waiting for its answer.
struct BarrierRound {
    outcome: BarrierOutcome,
    send_time: u64,
    fill: u64,
    pre_send_overflows: u64,
    overflowed: bool,
}

#[derive(Default)]
struct BarrierCounts {
    overflowed: usize,
    failed_with_overflow: usize,
    failed_without_overflow: usize,
}

impl BarrierCounts {
    /// Count the observations from one attempt, including acknowledged rounds.
    fn record(&mut self, outcome: &BarrierOutcome, overflowed: bool) {
        self.overflowed += usize::from(overflowed);
        match (outcome, overflowed) {
            (BarrierOutcome::Acknowledged(_), _) => {}
            (BarrierOutcome::TimedOut | BarrierOutcome::SendFailed, true) => {
                self.failed_with_overflow += 1;
            }
            (BarrierOutcome::TimedOut | BarrierOutcome::SendFailed, false) => {
                self.failed_without_overflow += 1;
            }
        }
    }
}

#[test]
fn failed_barriers_are_counted_with_their_own_overflow_observation() {
    let mut with = BarrierCounts::default();
    with.record(&BarrierOutcome::TimedOut, true);
    with.record(&BarrierOutcome::Acknowledged(1), false);
    let mut without = BarrierCounts::default();
    without.record(&BarrierOutcome::TimedOut, false);
    without.record(&BarrierOutcome::Acknowledged(1), true);
    assert_eq!((with.overflowed, without.overflowed), (1, 1));
    assert_eq!(
        (with.failed_with_overflow, with.failed_without_overflow),
        (1, 0)
    );
    assert_eq!(
        (
            without.failed_with_overflow,
            without.failed_without_overflow
        ),
        (0, 1)
    );
}

/// Drain the notify socket before building a fresh backlog for this sequence.
fn drain_before_barrier(socket: &RouteSocket, buffer: &mut [u8], sequence: u32) -> u64 {
    let started = now();
    let mut overflows = 0;
    loop {
        assert!(
            now() - started < PREP_DRAIN_LIMIT_NS,
            "barrier sequence={sequence} preparation drain exhausted, pre_send_overflows={overflows}"
        );
        match socket.receive(buffer) {
            Ok(Receipt::NothingQueued) => return overflows,
            Ok(Receipt::Datagram(_) | Receipt::Interrupted) => {}
            Err(RouteError::Overflow(_)) => overflows += 1,
            Err(error) => panic!("barrier sequence={sequence} preparation drain failed: {error}"),
        }
    }
}

/// One barrier round on the notify socket. The backlog is `changes`
/// address additions and removals; with none (under churn), the socket is
/// drained, then left unread until the churn has queued 75 per cent of
/// its receive buffer.
fn barrier_round(
    socket: &RouteSocket,
    wake: &Wake,
    buffer: &mut [u8],
    changes: usize,
    sequence: u32,
) -> BarrierRound {
    let pre_send_overflows = drain_before_barrier(socket, buffer, sequence);
    let mut lines = String::new();
    for index in 0..changes {
        writeln!(lines, "address add 2001:db8:ff::{:x}/128 dev lo", index + 1).unwrap();
    }
    for index in 0..changes {
        writeln!(lines, "address del 2001:db8:ff::{:x}/128 dev lo", index + 1).unwrap();
    }
    if changes == 0 {
        let target = receive_buffer() * 3 / 4;
        let waiting = now();
        while queued(socket.socket.as_raw_fd()).unwrap_or(0) < target {
            assert!(
                now() - waiting < 10_000_000_000,
                "barrier sequence={sequence} preparation refill exhausted before {target} bytes"
            );
            thread::sleep(Duration::from_millis(1));
        }
    } else {
        batch(&lines);
    }
    let fill = queued(socket.socket.as_raw_fd()).unwrap_or(0);
    let started = now();
    let sent = socket.send(&request(NLMSG_NOOP, NLM_F_REQUEST | NLM_F_ACK, sequence));
    let send_time = now() - started;
    let mut overflowed = false;
    let outcome = if sent.is_err() {
        BarrierOutcome::SendFailed
    } else {
        loop {
            if now() - started > u64::try_from(ROUND_LIMIT.as_nanos()).unwrap() {
                break BarrierOutcome::TimedOut;
            }
            match waited_receive(socket, wake, buffer, Duration::from_secs(1)) {
                Got::Datagram(length) => {
                    if answers(&buffer[..length]).contains(&(0, sequence)) {
                        break BarrierOutcome::Acknowledged(now() - started);
                    }
                }
                Got::Error(RouteError::Overflow(_)) => overflowed = true,
                Got::Error(_) | Got::Nothing => {}
            }
        }
    };
    BarrierRound {
        outcome,
        send_time,
        fill,
        pre_send_overflows,
        overflowed,
    }
}

/// The changes whose notifications fill the notify socket's receive buffer
/// to between 75 and 95 per cent, found by doubling then halving.
fn calibrated(socket: &RouteSocket, wake: &Wake, buffer: &mut [u8]) -> (usize, u64) {
    let limit = receive_buffer();
    let mut changes = 8;
    let mut low = 0;
    let mut high = usize::MAX;
    for round in 0..40 {
        let BarrierRound {
            fill, overflowed, ..
        } = barrier_round(socket, wake, buffer, changes, 1_000_000 + round);
        let fraction = fill * 100 / limit;
        if overflowed || fraction > 95 {
            high = changes;
        } else if fraction < 75 {
            low = changes;
        } else {
            return (changes, fill);
        }
        changes = if high == usize::MAX {
            changes * 2
        } else {
            usize::midpoint(low, high)
        };
        if changes == low || changes == high {
            break;
        }
    }
    (changes, 0)
}

#[test]
#[ignore = "a measurement (spec Q7 and Q9): as root in a network namespace of its own, with iproute2"]
fn q7_and_q9_each_round_trip_and_send_at_the_caps_worst_case() {
    let rounds = setting("IRONAFS_MEASURE_ROUNDS", 1000, 1000);
    let modes: &[bool] = match std::env::var("IRONAFS_MEASURE_CHURN").as_deref() {
        Ok("none") => &[false],
        Ok("routes") => &[true],
        Err(std::env::VarError::NotPresent) => &[false, true],
        _ => panic!("IRONAFS_MEASURE_CHURN must be none or routes"),
    };
    worst_case();
    let kernel = kernel();
    let request_socket = RouteSocket::request().unwrap();
    let notify = RouteSocket::notify().unwrap();
    let wake = Wake::new().unwrap();
    let mut buffer = vec![0_u8; BUFFER];

    for &churn in modes {
        // Keep sequence identities the same when either mode runs on its own.
        let mut sequence = if churn {
            1 + 3 * u32::try_from(rounds).unwrap()
        } else {
            1
        };
        let changes = if churn {
            0
        } else {
            let (changes, fill) = calibrated(&notify, &wake, &mut buffer);
            println!(
                "measure q7 kernel={kernel} backlog_changes={changes} backlog_fill_bytes={fill} \
                 receive_buffer={}",
                receive_buffer()
            );
            changes
        };
        let active_churn = churn.then(Churn::start);
        let label = if churn { "routes" } else { "none" };
        let mut dump_summaries = String::new();
        for (name, kind) in [("links", RTM_GETLINK), ("addresses", RTM_GETADDR)] {
            let mut times = Vec::new();
            let mut sends = Vec::new();
            let mut failed = 0;
            let mut counted = 0;
            for _ in 0..rounds {
                sequence += 1;
                match dump_round(&request_socket, &wake, &mut buffer, kind, sequence) {
                    Some((time, send, count)) => {
                        times.push(time);
                        sends.push(send);
                        counted = count;
                    }
                    None => failed += 1,
                }
            }
            writeln!(
                dump_summaries,
                "measure q7 kernel={kernel} churn={label} dump={name} objects={counted} \
                 failed={failed} {}\nmeasure q9 kernel={kernel} churn={label} send=dump-{name} {}",
                summary(&mut times),
                summary(&mut sends)
            )
            .unwrap();
        }
        let mut records = Vec::with_capacity(usize::try_from(rounds).unwrap());
        for _ in 0..rounds {
            sequence += 1;
            records.push((
                sequence,
                barrier_round(&notify, &wake, &mut buffer, changes, sequence),
            ));
        }
        drop(active_churn);
        print!("{dump_summaries}");

        report_barriers(&kernel, label, records);
    }
}

/// Report barrier evidence after the round loop and churn have ended.
fn report_barriers(kernel: &str, label: &str, records: Vec<(u32, BarrierRound)>) {
    let mut times = Vec::new();
    let mut sends = Vec::new();
    let mut counts = BarrierCounts::default();
    let mut fills = Vec::new();
    for (sequence, round) in records {
        let BarrierRound {
            outcome,
            send_time,
            fill,
            pre_send_overflows,
            overflowed,
        } = round;
        sends.push(send_time);
        fills.push(fill);
        counts.record(&outcome, overflowed);
        let outcome = match outcome {
            BarrierOutcome::Acknowledged(time) => {
                times.push(time);
                "acknowledged"
            }
            BarrierOutcome::TimedOut => "timed_out",
            BarrierOutcome::SendFailed => "send_failed",
        };
        println!(
            "measure q7-round kernel={kernel} churn={label} sequence={sequence} \
             outcome={outcome} pre_send_overflows={pre_send_overflows} \
             overflow_observed={overflowed} fill_bytes={fill} send_ns={send_time}"
        );
    }
    let BarrierCounts {
        overflowed,
        failed_with_overflow,
        failed_without_overflow,
    } = counts;
    let failed = failed_with_overflow + failed_without_overflow;
    fills.sort_unstable();
    println!(
        "measure q7 kernel={kernel} churn={label} barrier failed={failed} overflowed={overflowed} \
         failed_with_observed_overflow={failed_with_overflow} \
         failed_without_observed_overflow={failed_without_overflow} \
         fill_min={} fill_max={} {}",
        fills.first().copied().unwrap_or(0),
        fills.last().copied().unwrap_or(0),
        summary(&mut times)
    );
    println!(
        "measure q9 kernel={kernel} churn={label} send=barrier {}",
        summary(&mut sends)
    );
}

#[test]
#[ignore = "a measurement (spec Q8): as root in a network namespace of its own, with iproute2"]
fn q8_every_report_is_explained_by_its_receive_over_overflows_and_churn() {
    let seconds = setting("IRONAFS_MEASURE_SECONDS", 3600, 4800);
    ip("link set lo up");
    let kernel = kernel();
    let socket = RouteSocket::notify().unwrap();
    let wake = Wake::new().unwrap();
    let mut buffer = vec![0_u8; BUFFER];
    let _churn = Churn::start();
    let started = now();
    let mut next_burst = 0_u64;
    let (mut waits, mut errors, mut hangups, mut overflows, mut bursts) = (0, 0, 0, 0, 0);
    let mut unexplained_reports = Vec::new();
    while now() - started < seconds * 1_000_000_000 {
        // Each minute, a burst the socket does not read: the overflow the
        // tests provoke.
        if now() - started >= next_burst {
            let mut lines = String::new();
            for index in 0..4000 {
                writeln!(lines, "address add 2001:db8:9::{index:x}/128 dev lo").unwrap();
            }
            for index in 0..4000 {
                writeln!(lines, "address del 2001:db8:9::{index:x}/128 dev lo").unwrap();
            }
            batch(&lines);
            bursts += 1;
            next_burst += 60_000_000_000;
        }
        let Ok(Waited::Ready(ready)) = socket.wait(&wake, Duration::from_secs(1)) else {
            continue;
        };
        waits += 1;
        errors += u64::from(ready.error);
        hangups += u64::from(ready.hangup);
        let outcome = socket.receive(&mut buffer);
        overflows += u64::from(matches!(outcome, Err(RouteError::Overflow(_))));
        if let Some(reason) = unexplained(ready, &outcome) {
            let outcome = match &outcome {
                Ok(receipt) => format!("{receipt:?}"),
                Err(error) => format!("{error}"),
            };
            unexplained_reports.push(format!("{reason}: {outcome}"));
        }
    }
    println!(
        "measure q8 kernel={kernel} seconds={seconds} bursts={bursts} waits={waits} \
         pollerr={errors} pollhup={hangups} overflows={overflows} unexplained={}",
        unexplained_reports.len()
    );
    for report in unexplained_reports.iter().take(20) {
        println!("measure q8 unexplained {report}");
    }
}
