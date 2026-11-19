---
name: add-dependency
description: Use BEFORE adding, upgrading, or un-pinning any dependency in any crate of this workspace. Triggers on "add a crate", "add dependency", "cargo add", "bump version", "upgrade dep".
---

# Adding a dependency

Walk this checklist and record the answers in the commit message that adds
the dependency.

## Checklist

1. **Do we really have to?** Coding standards rule 21: IronAFS writes its own
   protocols (Rx, XDR, the FUSE wire protocol), event loop, CLI parsing,
   errors, logging, configuration parsing and test helpers. A dependency is
   only for what would be less safe written ourselves: mainstream
   cryptography, and the operating system's own interfaces (decided per
   platform in an ADR), plus the one named test-only exception in rule 21.
   Anything else is refused. Every eligible crate still needs the owner's
   recorded decision.
2. **Measure the transitive cost**: count the distinct resolved packages
   (from `cargo metadata --format-version 1`) before and after, on a scratch
   branch, for every supported target and feature set, dev-dependencies
   included. Check that each command succeeded before counting. More than ~5
   new crates means it must replace more code than it adds.
3. **Build-time execution and bundled C**: does it, or anything it pulls in,
   have a build script or a proc-macro? Check the target kinds
   (`custom-build`, `proc-macro`) in `cargo metadata --format-version 1`, and
   look in the package for bundled C or C++ sources before building it.
   Each build script goes into `deny.toml` `[bans.build]`. Record the reason,
   and any bundled native sources with their licences, in the commit.
   Dev-dependencies follow the same rules.
4. **Maintenance and security**:
   - a release within 24 months;
   - `cargo deny check advisories` is clean;
   - record the crates.io owner list as it stands, and one sentence on what
     happens if those owners stop. A headcount is not a test: it can be
     gamed, and it would exclude core crates with one careful maintainer.
5. **Licence**: ADR 0001 and `deny.toml` are the authority:
   - allowed (exact SPDX ids): `Apache-2.0`, `Apache-2.0 WITH LLVM-exception`,
     `MIT`, `BSD-2-Clause`, `BSD-3-Clause`, `ISC`, `Zlib`, `0BSD`, `BSL-1.0`,
     `Unicode-3.0`, `Unicode-DFS-2016`;
   - an owner decision per package: `CC0-1.0` and `CDLA-Permissive-2.0` (data
     only), `MPL-2.0`, `OpenSSL`, `Unlicense`, anything else;
   - refused: GPL, LGPL, AGPL and IPL, and anything without a clear licence.

   Run `cargo deny check` (licences, bans, sources and advisories).
6. **Pin it**: caret version, `default-features = false` unless a feature is
   needed and named; exact `=x.y.z` for anything that affects a wire or
   on-disk format.
7. **Check the premise**: a claim such as "we already depend on it" is
   checked in `Cargo.lock` (package identity, source and version). A claim
   about how it is used ("only a dev-dependency", "only on Linux") is checked
   in the manifests and with `cargo tree -e <kind> --target <triple>`, because
   the lock file does not record edge kinds or targets.
8. **Bans that target it**: if `clippy.toml`'s `disallowed-methods` or
   `disallowed-types` names something from the crate, prove the ban fires by
   planting a violation once.
9. **Exceptions** (a failed criterion accepted anyway) name the criterion, why
   it is accepted, how `deny.toml` or the gate records it, and the condition
   for removing it.
10. **Every dependency is re-justified once a year**, against this checklist.

Build tools that run but do not ship (linters, scanners) are pinned by
version. They don't count against the dependency budget, but their licence
must still allow use.

## Refusal template

If the checklist fails, say which rule failed and propose the in-house
alternative with an effort estimate. The default answer is no; every
dependency that is adopted is the owner's decision.
