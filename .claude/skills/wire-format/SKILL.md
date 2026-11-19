---
name: wire-format
description: Use whenever creating or changing any wire message or on-disk format in this repository — Rx packets and headers, XDR encoders and decoders, RPC interface definitions, callback messages, token or key formats, and any cache format on disk. Triggers on "format", "encoding", "decode", "parse", "serialize", "XDR", "Rx packet", "new field", "wire message", "on-disk", "cache format".
---

# Wire and on-disk formats

IronAFS reads bytes from fileservers it does not control, and may one day
keep caches it promises to read in later versions. Every format change
follows this skill. Coding standards rules 1–3 and 9 apply too.

## Decoding

- **Cap before allocating.** Every length or count read from the input is
  checked against a hard, named cap before any allocation or loop. A frame
  that claims more bytes than it carries is rejected, never grown to fit.
- **Consume exactly.** A decoder reports trailing bytes. Whether they are an
  error is decided per message type and written in the spec.
- **No panics** (rule 1): bad input produces an error value, never a panic,
  an overflow or an out-of-bounds access.
- **Tolerance is a recorded decision.** IronAFS must talk to real servers, so
  "strict" does not mean "reject anything unusual". Each field's tolerance
  (unknown flags, extra padding, future enum values) is decided in the spec
  and tested.
- **Integrity over what arrived.** Checksums, MACs and signatures are
  verified over the bytes received, never over a re-encoding of the decoded
  value.

## Encoding

- **The encoder refuses what the decoder rejects.** A value the decoder would
  refuse never leaves the encoder.
- **Reserved fields stay reserved.** They are never repurposed. A new meaning
  needs a new field or a version.

## Every format change carries, in the same commit

1. **The spec** in `docs/specs/`: layout, field types and order, caps,
   tolerance decisions, and the sources it rests on (ADR 0001: facts only, in
   IronAFS's own words).
2. **Round-trip property tests**, where an encoder exists: decode(encode(x))
   == x for every value the encoder accepts, and encode(decode(b)) == b for
   every *canonical* input b. For accepted non-canonical inputs (tolerated
   padding or trailing bytes), test the decoded value and the canonical
   re-encoding. Read-only parsers are tested against independent fixtures and
   their invariants.
3. **A fuzz target** for the decoder, in the top-level `fuzz/` crate
   (coding standards rule 20). Fuzzing needs nightly Rust and runs on Linux
   and macOS.
4. **A reject corpus:** inputs that must fail, each with the reason.
5. **Golden vectors with independent expected bytes**, derived from the spec,
   or from captures reduced to what the test needs, with synthetic identities
   and keys, inspected before they are added (ADR 0001). Never from another
   implementation's test files. Snapshots made by IronAFS's own encoder are
   regression tests only, never evidence of interoperability. CI checks the
   vectors on Linux, and on Windows and macOS once those runners are added
   (coding standards rule 15); until then they run by hand on every other OS
   a change touches.

## Formats IronAFS promises to keep

A cache or other format that later versions must read:

- starts with a magic value and a version;
- states, in its spec, whether older versions are decoded or dropped and
  rebuilt;
- is listed as frozen in `docs/specs/README.md`, which the first frozen format
  creates. Changing a frozen layout is class R.
