<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Local IPC framework: the Windows named pipe

Status: draft (2026-11-21). Raised in issue #62.

Kind: behaviour. Class: S. On Windows the daemon must read from a pipe
before it knows who connected; what happens in that interval, and how
the finite pipe pool is counted, are security rules ([README](README.md),
"Review class per kind").

This is the third of three parts of the local IPC framework. The first,
[local-ipc.md](local-ipc.md), holds the wire format and the order of
checks, and the table that says which part holds each place; the second,
[local-ipc-resources.md](local-ipc-resources.md), holds admission, the
accounts and scheduling. Section and point numbers are shared by the
three parts: a number names one place, whichever part cites it. This
part applies to the Windows control endpoint only, whose transport ADR
0005, sheets 17 to 20 decide; another endpoint's Windows transport is
its own specification's, with its own sites and gate (10.5). Labels
follow [README](README.md), "Labelling claims". Source ids are those of
local-ipc.md, section 12.

## 1. What this implements

ADR 0004, C45, point 1 and C5, point 5 as [ADR 0005](../adr/0005-ironafs-os.md)
amends it, quoted in local-ipc.md, section 1, for the Windows control
endpoint; and ADR 0005, sheets 17 to 20, whose references to "the
control IPC design" resolve as local-ipc.md, section 1 says.

## 2. Scope

1. **Covered:** the named pipe's instance pool; the one read made before
   the caller is attested; the Windows connection states; the ownership
   of buffers across overlapped operations; the residual this leaves;
   the Windows gate.
2. **Not covered, and where it is covered:** the operating-system calls
   and their safety obligations: ADR 0005, sheets 17 to 20. Everything
   that is the same on every operating system: local-ipc.md and
   local-ipc-resources.md.

## 3. The design in short

local-ipc.md, section 3 gives the whole design in short; this part's
share is the Windows half of its point 2.

## 4. Terms

Those of local-ipc.md, section 4.

## 5. Specification

### 5.1 Layout

Not in this part: local-ipc.md, 5.1 (the transport is its 5.1.1,
point 2).

### 5.2 Caps

Point 10 of the list that local-ipc.md, 5.2 starts:

10. **The Windows pipe pool.**
    - Windows accepts an instance count from 1 to 255, and 255 means no
      limit but the system's resources `[P1 nMaxInstances: CONFIRMED]`;
      every instance of one pipe must pass the same count, or creation
      fails `[P1 Remarks: CONFIRMED]`. A count of 1,024 cannot be passed.
    - `ipc.pipe.instances` = **254** for each IronAFS pipe, unless its
      endpoint specification sets a lower constant. It is a constant in
      the `ironafs-os` wrapper, the same on every instance (ADR 0005,
      sheet 17), never configuration, and 255 is never passed, because
      it would leave the bound to the system `[D2 sheet 17]`. 254 is the
      largest finite count: a multi-session host runs one session agent
      per logon session beside short-lived `afs` commands, and the
      per-caller budgets, not a small pool, stop one user from taking
      every instance `[OWN]`. Below, P is the constant the wrapper
      actually passes for the endpoint, 254 or the endpoint's lower one.
    - **One count of instances** `[OWN]`. Every instance is in exactly
      one of these states, and counts against P from its creation until
      its handle is closed:

      | Instance state | Armed (counts as a listener) | Counts as a connection (point 8) | Operations that may be outstanding |
      |---|---|---|---|
      | Listening | yes | no | one connect wait |
      | Refused: its connect completed and admission's first step failed | no | no | none; it is disconnected at once and is then Idle |
      | Idle: released, or refused and disconnected, with no connect wait | no | no | none; armed or closed at once (Handoffs) |
      | `PreAttestation` | no | yes, and a handshake slot | one read, of at most 4 bytes |
      | `AwaitHello`, `AwaitBinding` | no | yes, a handshake slot and a per-caller one | one read, one write |
      | `Open`; `Closing` after `Open` | no | yes; no handshake slot | one read, one write |
      | `Closing` never `Open` | no | yes; the handshake slots it held | one read, one write |
      | `Draining` | no | yes; and the handshake slots it held when it entered | those started before, each awaiting its terminal completion |

      **Handoffs** `[OWN]`. Two counts are kept, both under the
      admission lock. **Live handles** is every instance from its
      creation until its handle is closed, in any state of the table;
      it is at most P. **Armed** is the instances with a connect wait
      outstanding, the Listening ones only; the daemon keeps it at the
      configured number of listening instances.
      - When a listener's connect wait completes, the instance leaves
        the armed count, and the same step either makes it a connection
        (it enters the connection count) or makes it Refused. A Refused
        instance has nothing outstanding; it is disconnected in that
        step and is Idle.
      - Whenever the armed count is below the configured number, the
        daemon arms an Idle instance if there is one, and otherwise
        creates and arms a new instance. Its connect block is already
        paid for, whatever the accounts then hold (5.2.1, rule 1). It
        can: live handles are then
        armed + connections, below P by the configuration rule below.
      - Whenever an instance becomes Idle (Refused, or a connection
        released) while the armed count is already at the configured
        number, because a replacement was armed while it was a
        connection, its handle is closed in that step. So no instance
        rests in Idle, and the armed count is below the configured
        number only inside one step.

      A connection's **handshake slots**,
      global and per caller, are freed at the first of: the step that
      makes it `Open`, or its release. So an `Open` connection never
      holds one, and a connection refused or timed out in the handshake
      holds its slots through `Closing` and `Draining`.

      An instance is **released**, and only then leaves the connection
      count and frees any handshake slot, when every operation started
      on it has reached its terminal completion and it has been
      disconnected (ADR 0005, sheet 18). It is still a live handle: it
      is Idle, and is armed or closed as Handoffs says.
    - **Configuration is refused unless** listening instances +
      connections in all ≤ P `[OWN]`. Connections in handshake are
      inside connections in all (point 9), and `Draining` instances stay
      in it until release, so every instance the daemon can hold is in
      the sum, and a listener that became a connection can always be
      replaced. When the connection count is at its budget the daemon
      still listens; a client that connects then is disconnected without
      a read (5.4, point 2).
    - **Pipe buffers.** `ipc.pipe.buffer_in` = `ipc.pipe.buffer_out` =
      4,096 bytes, constants in the wrapper, the same on every instance
      (ADR 0005, sheet 17) `[OWN]`: the daemon's own buffers hold
      frames, so the pipe needs no more than one page each way. The
      system holds this storage, outside the in-flight budgets; its
      nominal bound is P × 8,192 bytes (2,080,768 at 254). How Windows
      rounds or reserves the two sizes is UNVERIFIED until the Windows
      gate records it; nothing here is coded from it.
    - **Outstanding operations** are at most the last column's: one
      connect wait on a listener, one read and one write on a connected
      instance, so at most 2 × P in all `[D2 sheet 18]`. A connected
      instance's read and write blocks are inside its connection's
      admission allowance; a listener's connect block is the instance's
      in the endpoint's standing charge of P blocks, in the global
      account only, made once when the endpoint starts (5.2.1, rule 1).
      No block is charged twice. An operation's buffer is one this
      connection has already reserved (the prefix, envelope, frame or
      discard buffer for a read; a queued frame for a write), at most
      one frame. So outstanding operations are bounded in count and in
      bytes by the budgets of point 8. A block whose completion never
      arrives stays counted, with its instance in `Draining` (5.6.2).

### 5.3 Versions

Not in this part: local-ipc.md, 5.3.

### 5.4 Decoding

Point 1 of local-ipc.md, 5.4 makes one exception to "the caller first",
on Windows. Its rule, in full:

   - Windows lets a pipe server impersonate its client only after data
     has been read from the pipe, and then takes the security context of
     the last message read `[P2 Remarks: CONFIRMED]`
     `[P3 1368: CONFIRMED]`. So on Windows the daemon reads exactly the 4
     bytes of `Hello`'s length prefix before it knows the caller, and
     nothing more. The documentation establishes that a read must come
     first; it does not say how many bytes that read must return. Four
     bytes is ADR 0005, sheet 18's rule, the whole prefix and nothing
     else `[D2 sheet 18]`. That a 4-byte read is enough on every
     supported release, and whether a shorter one would be, is
     UNVERIFIED until the Windows gate runs it (10.5).
   - The connection is in **pre-attestation** (5.6.2). Before the read
     is started it has passed point 2's first step: it holds a place in
     the connection count and one of the handshake budget's slots (5.2,
     point 8), and its admission allowance (5.2.1, rule 1) is reserved.
     It runs under the handshake deadline. Its 4-byte buffer is the only
     byte cap in this state.
   - The read of the prefix and the capture of the caller are one
     transition of the pipe driver, on one instance and one connection
     generation (ADR 0005, sheet 18). A read that completes with fewer
     than 4 bytes is continued, inside the same transition, for exactly
     the missing bytes. A read of 0 bytes, an end of stream, a broken
     pipe or any other failure ends the transition with a failure. When
     the handshake deadline passes with a read outstanding, the driver
     cancels it and fails the transition only after the read's terminal
     completion.
   - The 4 bytes are not decoded, compared or answered in this state.
     When all 4 are read, the caller is captured at once (impersonate,
     open the thread token, revert; ADR 0005, sheet 19). A result is
     bound to the connection only while its generation is the
     instance's current one and the connection is still in
     pre-attestation; any other result is discarded and its buffer
     zeroed (5.5, point 7).
   - The capture holds at most one token-information buffer at a time,
     each at most 1,024 bytes (ADR 0005, sheet 19 caps each buffer; that
     they are never held together is this specification's requirement
     on the capture `[OWN]`, which the connection's capture scratch
     pays for, 5.2.1, rule 1).
   - Only after the caller is captured is point 2's second step applied
     and the prefix checked as point 3 says.

### 5.5 Encoding

The Windows rows of the table of named zeroisation places (5.5, point 7,
in local-ipc-resources.md), and the rule that goes with them:

   | Label | Side | Buffer | Zeroed |
   |---|---|---|---|
   | `ipc-rx-pending` (Windows) | daemon | every read buffer of the pipe driver, the 4-byte prefix buffer included | when its use ends (below), on every path (the frame or prefix consumed, cancelled, failed, discarded for a stale generation or a closed connection), and never before the outstanding operation's terminal completion (ADR 0005, sheet 18) |
   | `ipc-tx-daemon` (Windows) | daemon | the same storage as `ipc-tx-daemon-stage`, from the moment a write is started on it until that write's terminal completion; the pipe driver owns it then | when its use ends (below), and never before the outstanding write's terminal completion. A timeout, close or stop in the row above requests the write's cancellation and waits; it never zeroes or frees a buffer the operating system still holds |
   | `ipc-tx-pending` (Windows) | client | the client's overlapped write buffer, if it uses one | when its use ends (below), never before terminal completion |
   | `ipc-rx-client-pending` (Windows) | client | the client's overlapped read buffer, if it uses one | when its use ends (below), never before terminal completion |

   **One owner at a time** `[OWN]`, after ADR 0005, sheet 18. A buffer
   belongs to its connection until an overlapped read or write is
   started on it, then to the transport until that operation's terminal
   completion, then to the connection again; its reservation goes with
   it and is never doubled.

   **A completion returns ownership; it does not end the use** `[OWN]`.
   A buffer's use ends, and only then is it zeroed, at one of two
   events: its frame (or prefix) has been wholly consumed, that is,
   every byte read and every value that borrows from it dropped, or
   every byte written; or it is abandoned (a failure, a cancellation, a
   stale generation, a close, a stop). A read or write that completes
   successfully with fewer bytes than asked is neither: the bytes
   already read or still unwritten stay in the buffer, the charge
   stays, and the next operation on the same buffer continues from the
   count the completion reported (for the prefix, ADR 0005, sheet 18's
   short completion). When a use ends while the transport owns the
   buffer, the order is: request cancellation, wait for the terminal
   completion, then zero with `ironafs-os`'s wipe (ADR 0005, sheet 24)
   and release. This holds for the client's Windows rows as for the
   daemon's.

### 5.6 States and ordering

#### 5.6.2 Connection states (daemon)

The Windows rows of the table in local-ipc.md, 5.6.2:

| State | Event | Next | Rule |
|---|---|---|---|
| Accepted (Windows) | admission's first step passes (5.4, point 2) | `PreAttestation` | otherwise disconnected without a read. The state records the instance's connection generation |
| `PreAttestation` | the transition returns, for this generation, 4 prefix bytes and a captured caller; admission's second step passes | `AwaitHello` | nothing decoded or answered in this state; the prefix is checked on leaving it (5.4, point 3) |
| `PreAttestation` | the transition fails, or admission's second step fails | Closed | no reply; nothing more read; a failed revert ends the daemon process (ADR 0005, sheet 19) |
| `PreAttestation` | the handshake deadline passes with the read outstanding | `Draining` | the driver requests the read's cancellation; no reply |
| `PreAttestation` | a transition result tagged with another generation | `PreAttestation` | discarded, buffer zeroed, bound to nothing |
| `Draining`, Closed (from `PreAttestation`) | a transition result of this generation | unchanged | discarded, buffer zeroed, bound to nothing |
| Closed (Windows) | an operation is still outstanding | `Draining` | the instance is not disconnected, reused or closed (ADR 0005, sheet 18) |
| `Draining` (Windows) | the last terminal completion | released | buffers zeroed, then the instance is disconnected and may listen again |

Points 1 and 3 of the list under that table:

1. `PreAttestation` is left only through its own transition's result or
   a close. On a close the driver still waits for the transition's
   terminal completion before the instance is disconnected or reused
   (ADR 0005, sheet 18).

and

3. **`Draining` has no deadline that frees anything** `[OWN]`, after
   ADR 0005, sheet 18: a block whose completion never arrives stays
   counted. Cancellation is requested once, when the state is entered.
   The cited documentation gives no bound on how long a cancelled pipe
   read or write takes to complete: UNVERIFIED, measured at the Windows
   gate (10.5). An instance still in `Draining` after one closing
   deadline is counted as **stuck** and reported in an event and in the
   daemon's status; it stays counted in the pool, in the connection
   count and in the handshake slots it held. The daemon does not close
   its handle, start anything on it, or restart itself because of it:
   a restart would drop every user's tokens (ADR 0004, C42) to recover
   one slot. Stuck instances are freed by their completion or by the
   end of the process. If they fill a budget, new connections are
   refused at admission and existing ones are unaffected; section 8
   states this as part of the Windows residual.

## 6. Arithmetic

Nothing of its own: the pool's counters are compared before each
increment, as local-ipc-resources.md, section 6 says of every counter.

## 7. Errors and failure

The codes are local-ipc.md, section 7. A connection closed in
pre-attestation has no code on the wire.

## 8. Security

The Windows rows of local-ipc.md, section 8's table:

| Who | Could try | Stopped by |
|---|---|---|
| Any local user on Windows | hold the pre-attestation slots, which cannot be counted per caller | bounded, not prevented: the **Windows pre-attestation residual**, stated below the table |
| Any local user on Windows | have bytes acted on before the caller is known | only the 4-byte prefix is read, never decoded or answered, before the capture (5.4, point 1) |
| A local user on Windows | swap clients between the prefix read and the capture | one driver-owned transition per instance and generation; stale results discarded (5.4, point 1; 5.6.2; ADR 0005, sheet 18) |
| Any local user on Windows | exhaust the pipe's instances | a finite count, never the unlimited value; one count of every instance in every state, `Draining` included; the connection and handshake limits applied before the prefix read; per-caller budgets inside them (5.2, point 10; 5.4, point 2) |

**The Windows pre-attestation residual** `[OWN]`. Windows gives a pipe
server no caller before the first read (5.4, point 1), so a connection
in `PreAttestation` cannot be charged to anyone. Any local user can hold
those connections. What bounds it, each stated on its own:

1. **How many at once:** at most the handshake budget for all callers
   (16 by default, 64 at most), and never more than the connection
   count or the pool, because both limits are applied before the prefix
   read is started (5.4, point 2). Open connections are not affected.
2. **When the daemon gives up on one:** at the handshake deadline (5 s
   by default) it requests the read's cancellation. That is a bound on
   the request, not on the slot.
3. **How long the slot stays taken:** until the cancelled read's
   terminal completion, plus the disconnect (5.6.2, `Draining`). No
   cited document bounds that time: UNVERIFIED, measured at the Windows
   gate (10.5). It is expected to be short; this specification does not
   rely on that, and a completion that never arrives leaves the slot
   counted (5.6.2, point 3).
4. **How often:** without limit. A user can take each slot again as soon
   as it is released, so while the attack lasts new connections can be
   refused indefinitely. The effect is confined to new connections on
   that endpoint.
5. **After the capture** nothing of this remains: from the moment the
   caller is known the connection is one of that caller's handshake
   slots, on Windows as elsewhere (5.4, point 2).

The residual is the smallest this design allows, since one read must
come before the capture. **It contradicts ADR 0004, O23 as that
decision stands:** O23's row for a malicious local IPC client says of
what remains "None known", and O23, rule 3 puts every residual in the
administrator's guide. O23 is Firm and this specification does not
change it. Until the owner decides section 13, Q4, the two documents
disagree, and this one is the accurate statement for Windows. This
specification recommends Q4's option (a): an amendment of O23's row
that states the scope (new connections on one endpoint), the
repeatability, and a lifetime that depends on a completion whose
timing is unverified, carried into the administrator's guide. Q4
blocks marking the Windows parts `reviewed`.

## 9. How the invariants are kept

The rows for C45.1 and C5.5 are in local-ipc.md, section 9; their
Windows tests are the gate of 10.5.

## 10. Tests

Paths and conventions are those of local-ipc.md, section 10.

### 10.5 Acceptance cases

- **Windows gate** (with the Windows layer, on each supported floor;
  gaps until then):
  - `preauth_reads_four_bytes`: impersonation before any read fails with
    1368; after the 4-byte prefix it succeeds, and nothing beyond the
    prefix has been read. The run records, per release, whether a
    shorter read would also have sufficed (recorded, not relied on);
  - `preauth_per_caller`: one user opens connections that each send
    only the 4-byte prefix: after the capture that user holds at most
    its per-caller handshake budget, the excess is closed with no
    further read, and a second user still connects;
  - `cancel_completion_time`: the time from the cancellation request to
    the terminal completion of a cancelled prefix read, a cancelled
    frame read and a cancelled write, measured over many runs and
    recorded in 12.1; with a stand-in whose completion never arrives,
    the instance stays in `Draining`, is reported stuck after the
    closing deadline, stays in every count, and is never disconnected,
    reused or closed;
  - short prefix completions: 1 then 3 bytes (attested after all 4);
    2 bytes then a close, 2 bytes then a stall past the deadline, a
    0-byte completion (each a failed transition, closed with no reply,
    the outstanding read's buffer released only after its terminal
    completion);
  - a disconnect, and a reconnect by another client, between the prefix
    read and the capture: the first identity is never bound to the second
    client;
  - `capture_scratch_peak`: under a counting allocator, the capture
    of a standard user and of an administrator with a linked token
    never has more than one token-information buffer live, and never
    more than 1,024 bytes of them;
  - `short_completions_keep_bytes`: a secret-bearing frame read in
    three short completions, and one written in three, arrive whole:
    nothing is zeroed between completions, the charge is unchanged, and
    the buffer is zeroed once, after the frame is consumed; the same
    frames with a cancellation after the first short completion and a
    terminal completion delayed past the close are zeroed only at that
    completion;
  - `pipe_pool`: with the global account at its budget and a handle
    still free, a listener that becomes a connection is replaced at
    once and the armed count returns to the configured number; P
    instances in use, in every state of 5.2, point 10's
    table, `Draining` included; with the pool full, a connect that has
    just completed and fails admission is disconnected with no read and
    its instance listens again, with the armed count back at the
    configured number and no instance left Idle; a connection released
    after its replacement was armed has its handle closed; a handshake
    that was refused keeps its
    handshake slot through `Closing` and through a delayed terminal
    completion, and an `Open` connection holds none; a configuration
    whose listeners plus connections in all pass P is refused; with the
    connection count at its budget a new client is disconnected with no
    read; with every instance in use a connect finds the pipe busy; the
    buffer sizes the live pipe reports are recorded;
  - **the daemon check, ADR 0005, sheet 20** (a shipping gate for every
    secret-bearing message on the Windows control pipe, in both
    directions, enforced as local-ipc.md, 5.5, point 6 says): the real
    daemon checked by an ordinary, unprivileged user; a pipe of the same
    name held by another user's process; the server exiting and being
    replaced between connect and check; and the residual interval before
    the process is opened, with the server's pid observed, independently
    of the check, to be held by another process: the check must answer
    "not the service" from a step that tells the connected pipe's server
    from the replacement, before any credential is read. Sheet 20 sets
    what counts as a pass. Until it passes, no secret-bearing message
    moves on the Windows control pipe in either direction: no client
    sends one and none accepts a secret-bearing reply, part or event.
    Sheets 16 to 20 cover the control endpoint's transport only. An
    endpoint with another transport (the adapter endpoint on Windows)
    names, in its own specification, its transport and its daemon check,
    with their own `ironafs-os` sites and gate, before any
    secret-bearing message of that endpoint is enabled; this gate does
    not stand in for that.

## 11. Revisit if

- This specification's own:
  - the Windows gate shows that reading only the 4-byte prefix is not
    enough to impersonate on a supported release, or that some release
    lets a pipe server identify its client before any read (5.4,
    point 1);
  - the Windows gate measures a cancelled pipe operation's completion
    time as long or unbounded in a case a local user can cause (5.6.2,
    point 3), which would make the residual of section 8 larger than
    stated;
  - a multi-session Windows host needs more than 254 connections
    (5.2, point 10);

## 12. Sources

The source table is local-ipc.md, section 12; this part uses its ids P1
to P3, D1, D2 and O. No other implementation's source, test data or
interface definitions were read for this specification, and none are
cited.

### 12.1 Observed behaviour

None yet. To be recorded at the Windows gate (10.5).

## 13. Open questions

- **Q4. ADR 0004, O23's row for a malicious local IPC client** says of
  what remains: "None known". Section 8's Windows pre-attestation
  residual is something that remains, and O23, rule 3 requires each
  residual to be in the administrator's guide. O23 is Firm, so this
  specification cannot change it. The owner decides between: (a) an
  amendment of O23 that states the residual in that row, which this
  specification recommends; (b) leaving O23 as it is and accepting the
  residual here only, which leaves the two documents disagreeing; (c) a
  further operating-system site in ADR 0005 that gives the daemon a
  hint of the connecting process before any read, used for counting
  only, which would shrink the residual at the cost of a new unsafe
  site and a value that proves nothing. Blocks: marking this
  specification `reviewed` on its Windows parts, and the Windows gate.
  No public issue yet.
- **Q5. How long a cancelled pipe operation takes to complete** (5.6.2,
  point 3) is unverified. The Windows gate measures it
  (`cancel_completion_time`). Blocks: stating a time bound for the
  residual of section 8. Part of the Windows gate (M5-28, planned).

### 13.1 Blockers and open items from review

The review ended without the second seat passing ([the review
record](../reviews/2026-11-21-m2a-62-local-ipc.md)). The items below
were still open at its last round. A **blocker** (B) is a major:
implementation (issue #95) must not start on the sections it names until
it is closed by a reviewed change to this specification. An **open
item** (M) is a minor, recorded and not gating. This part has no blocker
of its own. The Windows prefix rule's scope is part of B5 (local-ipc.md,
13.1). Q4 above is an owner decision and gates the Windows parts on its
own terms.

- **M1. The `pipe_pool` fixture for listener replacement.** Parts:
  local-ipc-windows.md, 10.5; local-ipc-resources.md, 5.2.1, rule 1.
  Quoted as received:

  > Defect: GPT 3 — PARTLY. The standing charge of P connect blocks
  > removes request-budget competition for listener replacement. The new
  > test starts with the global account at its budget and expects the
  > listener to become an admitted connection. Admission itself requires
  > a new global allowance.
  >
  > Consequence: Saturating the account before admission exercises
  > refusal and rearming of the same handle, so it does not verify that
  > an admitted connection gets a replacement.
  >
  > Fix asked: Leave exactly enough headroom for the first admission
  > step to consume, then assert that the account is full at replacement
  > and the armed count is restored without a new reservation.

  Closes with: the fixture leaving exactly the headroom the first
  admission step consumes, then asserting a full account at replacement
  and the armed count restored with no new reservation.

## 14. Review

Reviewed in five rounds on 2026-11-21 by two non-author families, and
not converged: one seat passed and one did not. The rounds, every
finding and the decision to stop are in [the review
record](../reviews/2026-11-21-m2a-62-local-ipc.md). Open here: open item
M1 (13.1), Q4 and Q5; B5 names this part's prefix rule. This
specification stays `draft`.
