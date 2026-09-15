---
type: Decision
title: Selection disposition — existing tools suffice; the substrate is the intentions board, linked not built-in
description: Coordination-selection outcome (2026-09-14): no explicit coordination machinery in code or guidance and no explicit dependency on the Clearhead CLI; the shared substrate is the intentions board and external authorities are linked. Workstreams-sessions build-out cancelled with its gap narrowed to the orientation-worktree-context fix; declarations and handoff cancelled as existing tools sufficed; dogfood re-scoped; Clearhead lifecycle updates applied as a projection of this record.
tags: [decision, coordination, selection, roadmap, trust]
generated: { by: agent/cli, at: 2026-09-15T01:22:40Z }
---

# Decision — Selection disposition: existing tools suffice; the substrate is the intentions board, linked not built-in

**Status:** Accepted by the owner in conversation on 2026-09-14 (two directives:
no explicit dependency on the Clearhead CLI, because it may not be present in
every environment; and no explicit coordination machinery in code or guidance —
if agents need shared intentions, a shared intentions board they already
maintain is cleaner, and there is a *link* to it instead). Applied to
Clearhead by Pi / gpt-6-astra the same day.

**Date:** 2026-09-14

**Evidence:** the pilot field report
([coordination-pilot-report.md](../evaluations/coordination-pilot-report.md),
two runs, commits `a6454b9` + `a46ebad`) and the predeclared protocol
([coordination-pilot-protocol.md](../evaluations/coordination-pilot-protocol.md)).
Evidence limits carry: n=1 per configuration, exposure-dependent wrong-belief
detection, disclosed control-arm contamination.

## Context

The `multi-agent-coordination` charter planned three entity slices
(`workstreams-sessions`, `coordination-declarations`, `coordination-handoff`)
behind a pilot gate. Two pilot runs in the foreign `plot` repository showed:
existing tools (Git, tests, one handoff document, plus the already-shipped
claims/checkpoints substrate) carried both handoffs with zero missed
dependencies and zero duplication; the sharpest observed failure was
repo-wide orientation presenting another worktree's landed state as true
locally; wrong-belief detection was exposure-dependent; and the trust limits
matched contract §1 exactly. The owner's directives above reframe how any
selected coordination capability may be built.

## Decision — per slice

1. **`workstreams-sessions`: build-out CANCELLED; motivating gap NARROWED to
   a projection fix.** No durable workstream or session entities will be
   built. The failure the slice would address — shared orientation lacking
   worktree/workstream attribution — is re-scoped as a small fix inside the
   existing substrate: serve the `worktree` context in `status`/`delta` JSON
   (the projection structs already define it; the CLI drops it) and scope
   session summaries to the querying worktree. The shared intentions board is
   the substrate the agents already maintain — claims, checkpoints, intent —
   reachable by *link* from anywhere (a handoff doc, an action note, a claim
   citation), never by a hard in-code dependency on any particular CLI.
2. **`coordination-declarations`: CANCELLED.** No scope-overlap or mutation-
   collision failure was observed across either run; request/response MCP
   sufficed; CC6 never fired in practice. Reopen only on a named, scorable
   failure that atomic advisory declaration would prevent.
3. **`coordination-handoff`: CANCELLED.** A 40-line handoff document plus Git
   reachability plus tests carried two cross-harness handoffs with zero
   missed dependencies; the decision record explicitly makes "existing tools
   suffice" a successful result that removes planned entities. The
   `coordination-authority-boundary` design action remains, and now serves
   the owner's link principle directly: its unsettled semantics (bound action
   revised/cancelled/unavailable, partial write-back) define exactly how the
   *link* between the board and external authorities behaves when the
   external side changes.

The unscorable-attribution workstreams-first exception is **not** invoked:
every failure was scorable.

## The link principle (what "linked, not built-in" means)

- External authorities (Clearhead for work lifecycle, OKF for curated
  knowledge) are referenced by the board, never required by it. A selection
  disposition's durable form is this record plus a workspace checkpoint; the
  Clearhead lifecycle updates below are a **projection**, applied where the
  CLI happens to exist, and reconcilable later by any session that has it.
- No guidance or code may *require* Clearhead, OKF, or any specific harness
  for coordination to function. A cold agent in a foreign repository must be
  able to discover everything load-bearing from the substrate (claims,
  checkpoints) and ordinary Git.

## Consequences

- `coordination-dogfood` is re-scoped to prove the selected design (existing
  substrate + orientation attribution fix + links), and its predecessor
  reference to the cancelled handoff action is removed.
- Reopen conditions, stated once: a future scorable failure that names one of
  the cancelled capabilities explicitly may reopen that slice through a new
  decision record — not by silently unblocking the old action.
- The pilot's "existing tools largely suffice" outcome stands as the honest
  headline; the one selected work item is a bounded orientation fix, not a
  coordination platform.

# Related Concepts

- Related to [Coordination pilot field report — plot, two harnesses, two linked worktrees](../evaluations/coordination-pilot-report.md)
- Related to [Coordination pilot before build-out, and documented trust limits](coordination-pilot-and-trust-limits.md)
- Related to [External workspace state and the Clearhead boundary](external-workspace-and-clearhead-boundary.md)
