---
type: Decision
title: Coordination pilot before build-out, and documented trust limits
description: Records owner decisions to pilot coordination with existing tools after contextual freshness, require a bounded reliance measurement before new coordination entities, and document attribution and capture-integrity limits.
tags: [architecture, coordination, evaluation, trust]
generated: { by: pi/gpt-6-astra, at: 2026-09-14T21:28:26Z }
---

# Decision — Coordination pilot before build-out, and documented trust limits

**Status:** Accepted by the owner in conversation on 2026-09-14; recorded by Claude Code /
claude-opus-5 at the owner's instruction. Applied to Clearhead actions by Pi / gpt-6-astra
on 2026-09-14; this application does not claim the pilot or implementation has run.

**Date:** 2026-09-14

## Context

The [coordination sequencing review](../evaluations/review-coordination-sequencing.md)
argued that the `multi-agent-coordination` charter builds four slices before comparing them
with simpler tools. The original author (Pi / gpt-6-astra) recorded dispositions and
recommended decisions on the three items reserved for the owner: P1, P2, and P5. The owner
agreed with the reviewer's reconciliation of those recommendations.

## Decision

1. **Pilot before slices 2–4 (P1).** `contextual-freshness` remains first. Next, run a
   short coordination pilot with a predeclared protocol: the charter's walking-skeleton
   scenario in two harnesses and two linked worktrees, using only Git, shared claims,
   checkpoints, and handoff documents. Log each duplicated effort, missed dependency, stale or
   wrong belief acted on, human recap, and ownership or activity gap, and map each to the
   slice that would have prevented it, or to none. `workstreams-sessions`,
   `coordination-declarations`, and `coordination-handoff` are built only as the pilot's
   failures select them. If failures cannot be scored without workstream identity,
   `workstreams-sessions` may precede the pilot.
2. **Reliance measurement is a required pilot output (P2).** It is not optional and not an
   indefinite gate on the broad `foreign-dogfood` action. The pilot must report whether a
   successor reused useful current support without rereading, re-observed changed support
   before acting, and spent less maintaining records than it saved in reconstruction. A
   stale-claim ratio is a prompt to investigate, not a pass/fail measure.
3. **Trust limits are documented (P5).** The normative text lives in
   [contract §1](../specifications/contextual-coordination-contract.md#1-authorities-and-identities).
   It discloses limits; it does not make the local kernel a security platform.
4. **The rest follows the author's dispositions.** P3 (split the review action), P4
   (cheap probes and a short baseline protocol; the full comparative study stays a
   proposal), and P6 (actions cite scenario IDs after a coverage audit) are accepted as
   dispositioned. P7 (separate the continuation note) is a later editorial slice. Faked
   records for probes are injected only into disposable evaluation state.

## Consequences

- The original author applied decisions 1, 2, and 4 through the Clearhead CLI:
  `contextual-freshness` plus the short `collective-agency-protocol` precede
  `coordination-pilot`, which precedes `coordination-selection`. The three candidate
  implementation actions remain explicitly blocked until selected; selection must cancel,
  narrow or unblock them and revise downstream dependencies before it completes.
- The review is complete. Unsettled authority-crossing design has its own
  `coordination-authority-boundary` action before selected handoff implementation. The
  standing full-study action is cancelled; the written study proposal remains available.
  Scenario references replace duplicated prose where coverage exists, retaining extra
  acceptance criteria for restart, session attention and legacy replay.
- A pilot showing that existing tools suffice is a successful result, and removes planned
  entities from the roadmap.
- Any design that crosses principal or integrity boundaries must answer the contract's trust
  limits explicitly rather than inherit attribution as proof.

## Additional pilot observation: shared intent interference

During application, the owner reported that the Claude reviewer deliberately avoided
changing the global workspace intent to avoid interfering with the drafter's work. The
shared status still displayed the drafter's review-disposition intent while both agents
had distinct responsibilities. This is an observed coordination workaround, not a measured
failure rate or an independent audit of implementation.

The pilot should record such interference. If workstreams are selected, acceptance must
show independent workstream intent and session-local attention over shared repository
knowledge. The goal is contextual views, not private per-agent knowledge silos. This
observation motivates that test; it does not bypass the pilot's selection gate.
