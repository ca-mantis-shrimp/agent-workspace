---
type: Specification
title: Coordination authority boundary contract
description: Defines how a Workspace Objective's optional external reference reuses knowledge-pulse pin/source-state machinery, and specifies the ready/git-landed/validated/externally-completed lifecycle, reorientation points, and explicit non-automatic write-back for bound external work items — with no mandatory Clearhead dependency.
tags: [coordination, intent, objective, clearhead, contract]
generated: { by: claude-code/claude-sonnet-5, at: 2026-09-16T08:56:56Z }
---

# Coordination Authority Boundary Contract

**Status:** normative contract for the `multi-agent-coordination` charter
(`coordination-authority-boundary` action). It extends the
[knowledge pulse contract](knowledge-pulse-contract.md) — §1 reuses that
contract's reference kinds and source-state machinery rather than defining a
second one — and the
[contextual coordination contract](contextual-coordination-contract.md), whose
worktree-relative assessment rule (§2) governs `git-landed` below. It makes
concrete the sibling-authorities boundary in
[external workspace state and the Clearhead boundary](../decisions/external-workspace-and-clearhead-boundary.md)
and the link principle in
[selection disposition](../decisions/coordination-selection-disposition.md):
`coordination-handoff` was cancelled because existing tools sufficed, and this
action was kept specifically to define how the link between the intentions
board and an external authority behaves when the external side changes.

## 0. Why, in evidence

`coordination-handoff` closed with "existing tools suffice," which removed the
planned entities but left one real gap named explicitly in the disposition:
nothing yet says what a bound external work item's *change* means to the
workspace, or how completion crosses that boundary without letting either side
silently overwrite the other. The knowledge-pulse contract already anticipated
the reuse this document makes concrete (its §9: "Assessing
`Intent.external_reference` with the same pin machinery — a separate slice;
noted because it is the obvious next reuse") and already ruled out heading or
byte-range references until dogfood shows whole-file pins causing alarm
fatigue (same §9). Both apply unchanged here; neither is re-litigated.

## 1. Vocabulary

- **Objective binding** — the existing `Intent` record (`thesis`,
  `external_reference`). This document only adds structure to
  `external_reference`; it does not rename or restructure `thesis`.
- **Reference kinds** — identical to the knowledge pulse contract §1.2:
  `repository_file { repository?, path }`, `opaque { locator }`, or none.
  There is no third, Clearhead-specific kind. A bound Clearhead action is a
  line inside a `.actions` file under `.clearhead/charters/`; it is pinned as
  a `repository_file` exactly like an OKF decision, using the file that
  currently holds it. Resolving an action id (or any tracker id) to that file
  is the binding agent's one-time job at bind time — a plain `clearhead show
  action <id>` or equivalent, run once — never kernel or adapter logic. A
  tracker id typed without that resolution is `opaque` and stays `unknown`
  forever, honestly, per §1.4 below; nothing infers a path from an id string.
- **Source state** — identical to the knowledge pulse contract §1.4:
  `current | changed | unavailable | unknown`, reported only after assessment,
  never called `stale`. No new source-state value is introduced.
- **Lifecycle state** — new, and distinct from source state. It describes the
  bound *external work item's* progress toward completion, not the byte-level
  state of the file that names it:

  | State | Meaning | Owning authority |
  | --- | --- | --- |
  | `ready` | No evidence yet that the objective's work has landed. | none (default) |
  | `git-landed` | The content this worktree's Git history shows satisfies the objective has been committed, assessed contextually per contract §2. | Git, in the querying worktree |
  | `validated` | A workspace checkpoint or claim, cited by id, records evidence that the landed change satisfies the objective. | Agent Workspace |
  | `externally-completed` | An explicit write-back attempt (§4) to the bound external authority succeeded. | the external authority itself |

  States are ordered but not required to be visited in strict sequence by the
  kernel — the kernel reports whichever conditions hold; it is normal for
  `git-landed` to be true while `validated` is not yet recorded, or for a
  binding with no external reference to never have anything beyond `ready`/
  `validated` (§2.4).
- **Write-back** — an explicit, caller-initiated attempt to tell the bound
  external authority that the objective is done. It is not a kernel-owned
  synchronization loop; see §4.

## 2. Invariants

1. **One reference, one pin machinery.** `Intent.external_reference` uses
   exactly the knowledge pulse contract's reference/pin/source-state code
   path. No second parser, resolver, or state enum is built for objectives.
2. **Lifecycle state is a report, never a mutation of the external record.**
   Computing or serving `git-landed`, `validated`, or `externally-completed`
   never writes to the bound Clearhead action, tracker, or any external
   system. Only an explicit write-back call (§4) does, and only the one
   authority it names.
3. **`git-landed` is worktree-relative, not global.** It is assessed against
   the querying worktree's Git history, exactly as claim freshness and
   checkpoint scoping already are (contextual coordination contract §2,
   `orientation-worktree-context`). Two linked worktrees may disagree.
4. **`validated` requires a cited checkpoint or claim.** `git-landed` alone,
   however true, never implies `validated`. Evidence must be named, not
   inferred from the presence of a commit.
5. **`externally-completed` requires a recorded write-back outcome.**
   `validated` alone never implies `externally-completed`. Absence of a
   write-back attempt is the honest state `not-attempted`, not an error and
   not a silent promotion.
6. **No automatic retry.** A failed or partial write-back attempt is retried
   only by a new, explicit caller-initiated call. The kernel never re-attempts
   on its own schedule, on wake, or on the next status/delta call.
7. **Append-only write-back history.** Each write-back attempt is its own
   event (`succeeded`, `failed` with a reason, or `partial` naming which named
   effects succeeded). No attempt event is edited or removed by a later one.
8. **No permission escalation.** Write-back uses only the credentials or tools
   already available to the caller's process. The kernel never stores,
   requests, or elevates external-system credentials.
9. **No mandatory dependency.** Everything through `validated` works with zero
   external tooling installed. Only `externally-completed` needs one, and its
   absence degrades to `not-attempted`, never to a blocked or failing
   status/delta call.
10. **No silent retargeting.** Identical to knowledge pulse contract invariant
    5 — a moved, renamed, or deleted bound file becomes `changed` or
    `unavailable` per its actual bytes; it is never followed to a guessed new
    location, and a Clearhead action's own archival move (`.actions` →
    `.completed.actions`) is exactly this case, not a special one (§6, AB6).

## 3. Reorientation points

A binding is worth re-examining, and its lifecycle/source state is
re-reported, when:

- source state transitions (`current` → `changed`/`unavailable`, or back),
  reported by the same `*SourceAssessed`-shaped event the knowledge pulse
  contract defines, scoped to the intent binding instead of a knowledge
  binding;
- `git-landed` changes in the querying worktree (a relevant commit lands or
  the worktree's branch moves);
- a write-back attempt completes, whatever its outcome.

None of these three infer *why* — a changed source might mean the action was
edited, cancelled, or archived on completion. The state is a nudge to re-read,
never a verdict (§6, AB6).

## 4. Write-back

Write-back is a single explicit operation the caller names: "attempt to tell
the bound external authority that this objective reached `validated`." It is
not defined by this contract as CLI/MCP verbs yet — this action is design-only
— but its shape is fixed for whatever slice implements it:

- Input: the binding, and the `validated` evidence it cites.
- Output: one event recording the attempt's outcome — `succeeded`,
  `failed { reason }`, or `partial { succeeded: [...], failed: [...] }` when
  the external effect is not atomic.
- The caller may retry an explicit failed or partial attempt at will; the
  kernel imposes no backoff, schedule, or limit, and does not retry on its
  own.
- A `succeeded` outcome is the only path to `externally-completed`.

## 5. Prohibited failures

- **AB-F1 — silent completion:** accepting a workspace transaction or
  checkpoint advances lifecycle state past `validated` without a recorded
  write-back attempt.
- **AB-F2 — invented `git-landed`:** reported without checking the querying
  worktree's actual Git history, or served identically to a worktree whose
  history disagrees.
- **AB-F3 — automatic retry:** a failed or partial write-back is re-attempted
  without a new explicit call.
- **AB-F4 — external overreach:** a write-back call mutates anything beyond
  the one named external effect it was called for (no history rewriting, no
  unrelated field changes, no completing a different action).
- **AB-F5 — mandatory dependency:** `ready`, `git-landed`, or `validated`
  reporting fails, blocks, or is withheld because no external tool, network,
  or tracker is present.
- **AB-F6 — invented resolution:** the kernel or an adapter infers a `path`
  from an opaque tracker id or UUID instead of leaving it `opaque`/`unknown`
  until a caller resolves it explicitly.

## 6. Executable scenarios

Fixtures are temporary Git repositories with a `.clearhead/charters/*.actions`
file where a Clearhead binding is exercised.

**AB1 — ad hoc objective, no external authority.**
- *Given* an `Intent` bound with a `thesis` and no `external_reference`,
- *then* lifecycle state exposes only `ready`/`validated`; `git-landed` and
  `externally-completed` are never offered as reachable states, because there
  is no bound source and no external authority to report against;
- *when* a checkpoint later cites evidence for the thesis,
- *then* lifecycle reports `validated`, with no `git-landed` field at all
  (absent, not `false`).

**AB2 — Clearhead action reference, `ready` → `git-landed`.**
- *Given* an `Intent` bound with `external_reference = repository_file{path:
  ".clearhead/charters/foo.actions"}`, pinned to that file's bytes at bind
  time, in two linked worktrees A and B,
- *when* the commit satisfying the objective lands only in worktree A,
- *then* A reports `git-landed = true` and B reports `git-landed = false`,
  each assessed against its own history, matching AB-F2's prohibition.

**AB3 — `validated` needs cited evidence.**
- *Given* AB2 in worktree A at `git-landed = true`,
- *then* `validated` is absent until a checkpoint or claim citing evidence for
  the objective is recorded in A;
- *when* that checkpoint is recorded,
- *then* A reports `validated = true`; B, still without the commit, reports
  neither `git-landed` nor `validated`.

**AB4 — `externally-completed` via explicit write-back, success.**
- *Given* AB3's validated state,
- *when* the caller explicitly invokes write-back naming the bound action,
- *then*, if the external call succeeds, a `succeeded` event is recorded and
  lifecycle reports `externally-completed = true`; the bound action's Git
  bytes are not touched by this call (Clearhead's own tooling, not the
  workspace, owns writing to `.actions` files).

**AB5 — write-back failure and partial failure, no auto-retry.**
- *Given* AB3's validated state,
- *when* write-back is attempted and the external call fails outright (tool
  absent, network down, permission denied),
- *then* a `failed { reason }` event is recorded, lifecycle stays `validated`,
  and no further attempt occurs without another explicit call;
- *when* a write-back names two external effects and only one succeeds,
- *then* a `partial` event names which succeeded and which failed, and
  lifecycle stays `validated` until a later explicit call reports full
  success.

**AB6 — archival move is a reorientation signal, not an error.**
- *Given* AB2's pinned `repository_file` reference to the action's line inside
  `foo.actions`,
- *when* the action later completes through ordinary Clearhead lifecycle
  tooling and its line moves to `foo.completed.actions`,
- *then* the pinned file's bytes differ (the line is gone), so source state
  reports `changed`, not `unavailable` and not `git-landed`/`validated`
  inferred from the move — the file still exists, its content changed, and no
  Clearhead-aware code path is required to reach this correct, honest report;
- re-reading the file (not the kernel) is what tells the caller the action
  completed.

**AB7 — opaque reference never resolves itself.**
- *Given* an `Intent` bound with `external_reference = opaque{locator:
  "01a0a1d2-31a8-78e1-8d49-eb2c97c65807"}` (a Clearhead action id typed
  without resolving it to a file),
- *then* source state reports `unknown` and stays `unknown` indefinitely; no
  kernel or adapter code attempts to detect the UUID shape and resolve it,
  matching AB-F6;
- *when* the caller later re-binds with the resolved `path` instead,
- *then* source state becomes assessable per AB2.

**AB8 — no external tooling installed.**
- *Given* a repository with no `clearhead` binary and no `.clearhead/`
  directory at all,
- *then* `ready` and `validated` (for a binding with no reference, or an
  `opaque` one) work exactly as in AB1; nothing about status or delta fails,
  blocks, or degrades beyond the absent field, matching AB-F5 and knowledge
  pulse contract invariant 9.

## 7. Deliberately out of scope

Each item has its reopen trigger.

| Out of scope | Reopen when |
| --- | --- |
| Content-anchored or byte-range references for a bound external item | Same trigger as the knowledge pulse contract §9: whole-file pins produce `changed` noise that dogfood can score as alarm fatigue, now observed on Intent bindings specifically. |
| Automatic write-back retry, scheduling, or backoff | Manual re-invocation proves to be a measured friction point in dogfood, not a conceptual concern. |
| A Clearhead-specific (or any tracker-specific) resolver or adapter shipped in the kernel | A new evidence-backed decision record, per the link principle in `coordination-selection-disposition.md`. |
| Multiple external references per objective | Same "which one is canonical" problem the knowledge pulse contract already declined (§1.1); one only. |
| Any structured handoff, workstream, session, or declaration entity | Not reopened by this document; those remain cancelled per `coordination-selection-disposition.md` unless a new decision record reopens them. |

## 8. Acceptance

This document itself is the acceptance artifact for the design-only
`coordination-authority-boundary` action: AB1–AB8 are executable scenario
definitions distinguishing `ready`, `git-landed`, `validated`, and
`externally-completed`; §2 and §5 rule out automatic Clearhead completion
(AB-F1, invariant 6), history rewriting (invariant 7, AB-F4), permission
grants (invariant 8), and a mandatory Clearhead dependency (invariant 9,
AB-F5, AB8). A future implementation slice closes against AB1–AB8 as its
fixture suite, the same way `knowledge-pulse-kernel` closed against KP1–KP14.

## Related concepts

- [Knowledge pulse contract](knowledge-pulse-contract.md): Supplies the
  reference kinds and source-state machinery §1 reuses without modification.
- [Contextual coordination contract](contextual-coordination-contract.md):
  Supplies the worktree-relative assessment rule `git-landed` inherits.
- [External workspace state and the Clearhead boundary](../decisions/external-workspace-and-clearhead-boundary.md):
  The sibling-authorities boundary this contract makes concrete for
  completion write-back specifically.
- [Selection disposition — linked, not built-in](../decisions/coordination-selection-disposition.md):
  Records why `coordination-handoff` was cancelled and this action kept in
  its place.
