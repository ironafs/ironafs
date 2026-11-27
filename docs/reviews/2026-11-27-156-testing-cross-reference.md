<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# #156 testing.md 10.6 cross-reference

Class I. Base `21f46b1fade4aee4db506bd5b491d9c8fdd39b67`. Reviewed source `1c81d918a192110bc6d09b208bf1db7149d1cfd6`, tree `2482fdebee86cf6b8dd75c4937d83865c3a22f66`. The change covers `docs/specs/testing.md` 10.6 only.

10.6 ends: "Each issue not yet filed is named here by its number when it is filed." The seam conformance bodies for the seams of #71 and #99 are now filed as #156 (its S1 and S2), so the clause moves out of the "not yet filed" list and names #156. The refusal-coverage check, the differential listing check, the release-build check of the fault feature (M2a-89) and the seam and ban census (M2a-50) stay in that list: none has an issue of its own.

## Independent review

| Seat | Served model | Effort | Route | Result |
|---|---|---|---|---|
| Author | `gpt-6.2-sol` | High | Codex CLI | Two-line edit |
| Independent review | `glm-5.4` | High | Z.ai Coding Plan, headless Claude Code | PASS, no findings |

The review read the pinned range `21f46b1..1c81d91` and the text of #156 inlined in its brief, and checked the edit against #156's outcome list, testing.md 5.4 point 1 and the other references to #156 in the tree. The gate confirmed the served model and the run's sentinel. Headroom was not reported. GLM 5.4 High's IronAFS calibration (#48, record 2026-11-27-48-glm54-document-calibration) measured Class S pre-code document review. This seat is a smaller document review of the same kind.

Points the seat recorded as uncertain, with their disposition:

- Whether the four items left in the list are still unfiled: the controller searched open and closed issues for each name and plan id. None has an issue of its own; they appear only inside #41, #99, #155, #156 and other issues' text. Left in the list, as 10.6's rule requires.
- The seam and ban census is owned only in part by #156, which delivers its seam walker early (record 2026-11-27-156-early-seam-census); the census itself belongs to M2a-50, not yet filed. 10.6's wording for the census predates this change and is not altered by it. No finding.

## Executed evidence

The patch applied cleanly to `21f46b1`, and the resulting file matches the prepared candidate blob `37c859b68093f8df1f2b242febdd49a8e3137a5c`. The change has no code.
