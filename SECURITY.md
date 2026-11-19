<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Security policy

IronAFS will parse data from fileservers, serve callbacks to them and handle
users' tokens, so security reports matter even before a release.

## Reporting a vulnerability

Report privately, never in a public issue:

- by email to adrian@adrianana.com, with "IronAFS security" in the subject;
  or
- through GitHub's **Report a vulnerability** button on the repository's
  Security tab, once private vulnerability reporting is enabled for the
  public repository.

Include what you found, how to reproduce it, and the version or commit. You
will get an acknowledgement within 7 days. Fixes are made in private and
disclosed once a fixed version is available, with credit to the reporter
unless they prefer otherwise.

IronAFS is built with AI model lanes (see [AI.md](AI.md)). Until a
vulnerability is disclosed, its details go only to the Anthropic (Claude) and
OpenAI (GPT) lanes, and the fix is prepared privately. The other lanes see
it once it is public (`docs/workflow.md` §2, "Embargoed security fixes").

## Supported versions

None yet: IronAFS has no release. Once it does, this section lists which
versions get security fixes.

## Scope

In scope: IronAFS's own code and its build and release process. A
vulnerability in an AFS server, or in a dependency, is reported to that
project. Tell us as well if IronAFS is affected.
