---
type: Decision
title: Worktree-relative freshness
description: Makes claim/evidence/finding current-stale-unknown verdicts worktree-relative by stamping a worktree identity on the assessment events and materializing the querying worktree's verdict during replay, without a separate assessment entity.
tags: [coordination, freshness, worktrees, assessments]
generated: { by: deepseek/deepseek-v4-pro, at: 2026-09-14T19:30:00Z }
---

# Worktree-relative freshness

**Status:** the design decision behind the `contextual-freshness` action
(`urn:uuid:01a09e5a-f70e-7e22-996e-974957d84e4b`). It settles the one gating
question the [contextual coordination contract](../specifications/contextual-coordination-contract.md)
leaves open — *how* assessments are keyed and selected — so the implementation
does not have to re-derive it.

## Problem

The state store is already shared across linked worktrees: `locate.rs` hashes
`git rev-parse --git-common-dir`, so two worktrees append to one event log. But
the reducer applies `ClaimReconciled` / `EvidenceReconciled` /
`FindingReconciled` as a single **global** overwrite of
`report.freshness_within_scope`. The last worktree to reconcile wins, so one
worktree's reconciliation masquerades as another's served assessment — prohibited
failure **C1**.

## Decisions

### 1. Worktree identity is the canonical worktree git directory

`git rev-parse --git-dir`, made absolute and canonicalized: `<repo>/.git` for the
main worktree, `<repo>/.git/worktrees/<name>` for each linked worktree. Distinct
and stable, unlike a branch name or HEAD. This is the contract §1 identity and is
deliberately *different* from the project identity (`--git-common-dir`) that the
state root is keyed by. Non-git targets fall back to the canonical repository
path, mirroring `project_identity`.

### 2. Scope is claims, evidence, and findings — not observations

The contract §2 names only the three legacy `*Reconciled` events for claim,
evidence, and finding, and the action's acceptance scopes independence to those
three. Observations are already worktree-local by nature (a read is bound to a
checkout) and the working set is session-local, so their global verdict is left
unchanged. A later slice can revisit observations if the working-set view shows a
real oscillation.

### 3. The assessment events carry the worktree identity; no new entity

The seven events that currently write `report.freshness_within_scope` gain a
`#[serde(default)] worktree_identity: Option<String>`:

- `ClaimRecorded`, `ClaimAmended`, `ClaimReconciled`
- `EvidenceRecorded`, `EvidenceReconciled`
- `FindingRecorded`, `FindingReconciled`

`None` (serde default) is the legacy shape. No separate `Assessment` entity is
introduced: the event is already the durable assessment record; the only
correction is that it now names the worktree it was computed against.

### 4. The reducer materializes the querying worktree's verdict during replay

A `Workspace` handle is always opened in one worktree, so the `Projection` it
builds is always "for" that worktree. The reducer therefore materializes
`report.freshness_within_scope`, `report.reason`, and
`operational_coverage.reconciliation_fingerprint` **only when the event's
worktree identity matches the handle's** (`is_mine`). Events from other
worktrees are applied to the log but do not touch this handle's `report`. This
is what makes the shared log project differently per worktree, and it keeps
`verdict_unchanged` (no-op suppression) correct: it compares against the
materialized verdict, which is now *this worktree's* last verdict, so a divergent
worktree's reconcile is never mistaken for an unchanged one.

No separate per-worktree map is needed: replay already walks the whole log, and
"the latest event carrying my worktree identity" is exactly the querying
worktree's assessment.

### 5. Legacy events are never served as current

A record event without a worktree identity materializes as `Unknown` with reason
"not yet assessed in this worktree", and a legacy reconcile event is skipped.
Every surface that serves a verdict (`resume_status`, `resume_brief_status`,
`resume_findings_view`, `resume_transaction_preview`, and the single-entity
`reconcile_*` verbs) already reconciles before serving, so the first status after
upgrade writes a contextual assessment for the querying worktree and self-heals.
This satisfies "MUST NOT be served as a current contextual assessment without a
new reconciliation" and preserves honest single-worktree semantics.

### 6. Surfaces name the worktree context

`WorkspaceStatus`, `BriefStatus`, and `DeltaStatus` gain a `worktree` field
carrying the identity string, so every served verdict is explicitly attributed to
the worktree it was computed against (contract §2).

## Acceptance mapping

- **CC1** (linked-worktree independence): one shared claim, divergent worktrees →
  current in one, stale in the other, simultaneously; replay materializes each
  worktree's own `*Reconciled` event.
- **CC2** (legacy oscillation): a fixture replays identity-less alternating
  `ClaimReconciled` events and asserts they are *not* served as current — the
  characterization of the pre-contextual gap, retained as regression.
- **Status/delta name context**: `worktree` field on both bounded surfaces.
- **Single-worktree semantics**: a fresh record stamps the recording worktree, so
  record → immediately serves that worktree's record-time verdict.

## Residuals (out of scope, noted not hidden)

- Observation freshness remains global (see decision 2).
- Workstream/session/declaration/dependency/handoff identities are later slices,
  gated behind the coordination pilot and selection — not touched here.
- The `worktree` string is the canonical git-dir path (the honest identity), not
  a display-friendly branch name.

## Related concepts

- [Contextual coordination contract](../specifications/contextual-coordination-contract.md)
- [External workspace state and the Clearhead boundary](external-workspace-and-clearhead-boundary.md)
- [Coordination pilot before build-out, and documented trust limits](coordination-pilot-and-trust-limits.md)
