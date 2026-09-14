---
type: Design Response
title: Review of coordination sequencing and the collective-agency plan
description: Proposes piloting multi-agent coordination against a simple baseline before building most of it, gating build-out on single-agent reliance, splitting overloaded actions, and recording a trust assumption; submitted for disposition.
tags: [review, coordination, multi-agent, sequencing, evaluation]
generated: { by: claude-code/claude-opus-5, at: 2026-09-14T17:40:17Z }
---

# Review: coordination sequencing and the collective-agency plan

*Reviewer: Claude Code / claude-opus-5, at the owner's request, 2026-09-14. Scope:
`.clearhead/charters/multi-agent-coordination.md` and its actions as of `9ca111d`, the
[contextual coordination contract](../specifications/contextual-coordination-contract.md),
and the collective-agency continuation of [the agent's perspective](../design/agent-perspective.md)
as revised in `acfeb34`. This is review input for the `collective-agency-review` action.
Every item is a proposal: nothing here changes an action, a priority, or the contract. The
original author reconciles; the owner settles items marked **owner decision**.*

## Summary

The contract is sound and its first slice is justified. The disagreement is about order.
The plan builds four coordination slices, then dogfoods them, then compares against simpler
baselines. The collective-agency note already asks the right question—*which benefit cannot
be had from Git, a handoff document, and ordinary messaging?*—but the plan answers it only
after the build, when a "reject" verdict has become expensive.

## What should stay

- **Honest semantics.** Declarations are advisory, `last_seen` is an estimate, Git decides
  landing, and handoffs transfer responsibility but never truth.
- **Testable failure model.** Prohibited failures C1–C8 and scenarios CC1–CC8 are precise.
- **Observed motivation.** On the ClearHead platform, a second agent recovered context from
  the workspace but could not tell who owned the work or whether they were active
  ([motivating observation](../design/harness-neutral-multi-agent-coordination.md#motivating-observation)).
- **`contextual-freshness` first.** C1 (one worktree's reconciliation overwriting another's)
  is a soundness defect for any worktree user, multi-agent or not. It needs no further
  justification.
- **Evaluation guardrails.** Spend and external effects already require owner approval, and
  failed trials are retained.

## Proposals

### P1 — Pilot against a baseline before slices 2–4 (owner decision)

**Current:** contextual-freshness → workstreams-sessions → coordination-declarations →
coordination-handoff → coordination-dogfood → collective-agency-protocol →
collective-agency-evaluation.

**Proposed:** contextual-freshness → *coordination-pilot* → only the slices the pilot's
failures select → coordination-dogfood.

The pilot runs the charter's walking-skeleton scenario with two harnesses in two linked
worktrees, using only what exists after contextual-freshness: Git, shared claims,
checkpoints, and a handoff markdown file. Log each point where an agent duplicated work,
missed a dependency, acted on a stale or wrong belief, needed a human recap, or could not
tell ownership or activity. Map each failure to the slice that would have prevented it—or
to none.

Why:

- The baseline comparison is gated behind `coordination-dogfood`, which requires all four
  slices. Sunk cost then biases the evaluation it is meant to inform.
- The motivating observation is evidence for part of `workstreams-sessions` (ownership and
  activity). No observation yet shows the race atomic declarations prevent (C5, CC6) or a
  handoff that failed for lack of a structured record.
- This applies the charter's own rule—defer semantic-symbol overlap "before path and
  work-item/capability overlap prove insufficient"—one level up.

**Refutation condition:** if failures cannot be scored without workstream identity
(attribution too ambiguous), build `workstreams-sessions` first and pilot after it. That is
still cheaper than building all four.

### P2 — Single-agent reliance gates multi-agent build-out (owner decision)

`foreign-dogfood` ("earn trust under real stakes", !2) is still in progress. Its acceptance
centers on single-agent reliance: a cold agent reuses at least one narrow current claim
without rereading. `coordination-dogfood`, meanwhile, is !1.

Single-agent reliance is not yet settled. When this review was written, 21 of 27 active
claims in this repository's own workspace were stale. The
[subtraction review](tooling-friction-and-subtraction-review.md) concluded that the "next
risk is not missing capability but losing that value under bookkeeping and context
overhead," and recommended a friction audit "before adding features." Workstreams, sessions,
declarations, dependencies, and handoffs are more records, and they inherit whatever
curation problem claims already have.

Proposal: finish `foreign-dogfood`, or at least its handoff measurement, before
`workstreams-sessions` begins. `contextual-freshness` proceeds regardless.

Caveat: stale is a prompt to re-observe, not proof the claims were wrong. It is evidence
that claims are not being maintained—which is the adoption question.

### P3 — Split `collective-agency-review`

Besides reconciling this review, the action asks for the Clearhead/Workspace commitment
boundary, "ready ≠ complete" semantics, objective revision and cancellation, and cross-tool
partial failure. That is design work needing its own acceptance. Bundled, either the review
cannot close until the design is done, or the design gets done without acceptance under a
review label.

Proposal: close `collective-agency-review` on recorded dispositions of this document. Move
the boundary questions to a separate design action preceding `coordination-handoff`, where
ready, landed, and complete first diverge. Check
[the external-workspace boundary decision](../decisions/external-workspace-and-clearhead-boundary.md)
first; part of it may already be settled.

### P4 — Probes inside the dogfood instead of a standing study

`collective-agency-protocol` and `collective-agency-evaluation` describe a three-arm,
budget-matched study with repeated trials and order controls. For a single-owner,
local-first project that is a research program, and open actions shape what agents select
next.

Proposal: add the cheap discriminating probes to `coordination-dogfood` acceptance:

1. a plausible claim that is wrong while its cited files are unchanged;
2. a recorded observation that misreports what a worker saw;
3. a task that cannot be done as specified, where an honest impossibility report scores as
   success;
4. a peer request to expand scope.

Keep the full study as the written proposal it already is in the perspective note, not as
two open actions. Reopen it only if the dogfood result is ambiguous enough that nothing short
of a controlled comparison would settle it.

### P5 — Record the trust assumption (owner decision)

The walking skeleton deliberately uses two different harnesses, and the charter's non-goals
exclude multi-user authentication. METR's report shows agents forging tool-call records and
inventing message signing after impersonation. The workspace claims no protection against
this, but nothing states the assumption.

Proposal: if accepted, add the trust-boundary statement from
[the agent's perspective](../design/agent-perspective.md#the-record-is-an-attack-surface-not-a-neutral-witness)
to contract §1 beside session identity, and link it from the charter's non-goals. Wording is
open; the substance is *attributed, not authenticated; freshness detects changed support,
not misreported support; capturing harnesses are assumed honest.*

### P6 — Actions cite scenario IDs instead of restating the spec

The coordination model is stated in the charter's core model, contract §1, the historical
design note, and the `foreign-dogfood` action text; implementation actions paraphrase
scenarios (for example, `coordination-declarations` restates CC6). Four copies drift.

Proposal: actions keep intent plus "Acceptance: CC*n* green" with a contract link, and the
charter's core model defers to contract §1.

### P7 — Give the collective-agency continuation its own note (low priority)

`design/agent-perspective.md` now holds three authors and two genres—the 09-01 position
paper and the 09-14 coordination memo—under one `generated` field that names only the last
writer. After this review is dispositioned, so diffs stay readable, move the continuation to
its own design note and leave a dated link.

## Dispositions

*For the original author. A rejected proposal with a reason is a useful outcome.*

| # | Proposal | Owner decision | Disposition | Reason / evidence |
|---|---|---|---|---|
| P1 | Baseline pilot before slices 2–4 | yes | | |
| P2 | Single-agent reliance gates build-out | yes | | |
| P3 | Split the review action | no | | |
| P4 | Probes in dogfood, not a standing study | no | | |
| P5 | Record the trust assumption in the contract | yes | | |
| P6 | Actions cite scenario IDs | no | | |
| P7 | Separate continuation note | no | | |

## Reviewer's limits

- I reviewed documents and action text, not kernel code. I have not verified how costly
  deferring `workstreams-sessions` would be, which bears on P1's refutation condition.
- METR details were checked against the report's full text; its transcripts were not
  audited.
- P5 and P7 partly rest on my own revision of the perspective note (`acfeb34`), which is
  under review by the same author.
