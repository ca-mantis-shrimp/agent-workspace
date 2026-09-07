---
type: Decision
title: Claims gain a retirement disposition: retire without a replacement
description: Adds a ClaimRetired event and ClaimLifecycle::Retired variant so a claim can be retired without a successor, giving claims the freshness-orthogonal disposition axis findings already have and unclogging the stale signal.
tags: [architecture, claims, freshness, model]
generated: { by: claude-code/opus-4.8, at: 2026-09-07T07:46:12Z }
---

# Decision — Claims gain a retirement disposition: retire without a replacement

**Status:** implemented 2026-09-07 (kernel event + reducer + command, CLI
`retire-claim`, MCP `workspace_retire_claim`, acceptance tests).
**Date:** 2026-09-07
**Participants:** user + assistant.

## Context

Dogfooding the live write loop made an asymmetry unignorable. Two kinds of claim
had no honest disposal:

1. **Mistaken records** — a debugging probe recorded as a "belief" (real case:
   claim 99, typed while diagnosing the MCP door).
2. **Obsolete-but-once-true claims** — beliefs whose subject work is simply done
   (many historical claims about shipped slices), which linger as `stale` forever
   because their files changed but nobody will ever re-verify them.

`supersede-claim` could not retire either: it *requires* a replacement claim
("belief B replaces belief A on the same subject"). Forcing a fake replacement to
dispose of junk would corrupt the supersession graph — violating the one thing
this store exists to protect. The result: **"stale" was doing double duty**,
meaning both *"re-verify before acting"* and *"this is old news"*, which dilutes
the freshness signal that is the whole point of the workspace.

The root cause is structural: **findings already have three axes** —
`FindingRecorded`, `FindingReconciled` (freshness), and
`FindingDispositionChanged` (open/resolved/deferred/…). **Claims had only two** —
recorded, reconciled (freshness), and `ClaimSuperseded` (replacement-required).
Claims never got the freshness-orthogonal disposition axis.

## Decision

Give claims a retirement disposition — **the minimal typed thread**, not a full
disposition enum:

- New event **`ClaimRetired { claim_id, reason }`** — structurally
  `ClaimSuperseded`'s sibling, minus the replacement link. Reason mandatory, same
  discipline as supersession.
- New lifecycle variant **`ClaimLifecycle::Retired { reason }`**. Because every
  active window and reconciliation path filters on `lifecycle.is_active()` (which
  matches only `Active`), a retired claim is **automatically** excluded from the
  active window and from reconciliation — no other call sites change.
- Command `Workspace::retire_claim`; CLI `retire-claim --id --reason`; MCP
  `workspace_retire_claim`. Retirement deliberately does **not** reconcile
  freshness first (supersession does): retirement is a disposition, orthogonal to
  whether the claim was still true.
- The `--full` audit splits inactive claims three ways so a retired claim is never
  mislabeled superseded (`retired_claims` DTO field + brief count).
- Append-only preserved: a retired claim stays in the log, fully auditable. This
  is a disposition event, never a deletion.

## Rationale

- **Single event over a disposition enum.** Findings earned five dispositions
  because they *behave* differently (suppressed vs false-positive change
  behavior). A retired and a "retracted (mistaken)" claim behave identically —
  both just leave the active set — so the only real difference is the mandatory
  reason text, which already carries it. One event, one concept: "a claim can be
  retired without a replacement." Simplicity over speculative symmetry.
- **Rides an existing groove.** It mirrors `ClaimSuperseded` (which moves a claim
  out of active) minus the successor, so the projection change is a single new
  lifecycle variant the existing `is_active` filters already handle.
- **Restores signal precision.** After this, `stale` in the active window means
  only "re-verify"; "no longer maintained" is carried by `Retired`.

## Consequences

- Junk and obsolete claims can be disposed of honestly (claim 99 retired as the
  end-to-end proof). Historical stale claims about completed work can now be
  archived so the freshness histogram reflects only live beliefs.
- If a behavioral difference between retirement flavors ever emerges (e.g. a
  "suppress this class" disposition), this generalizes into a `ClaimDisposition`
  enum the way findings did — deferred until earned.

## Alternatives rejected

- **Full findings-style `ClaimDisposition` enum now.** Rejected: machinery for
  states that behave identically today; the reason text distinguishes them.
- **Overload `supersede-claim` with an optional replacement.** Rejected: it muddies
  a verb whose meaning is precisely "replaced by a named successor".
- **Let obsolete claims just stay stale.** Rejected: that is the status quo that
  degraded the freshness signal in the first place.

# Related Concepts
- [MCP servers resolve their repository from the launch directory and fail loud when it is wrong](mcp-repository-resolution.md): Fixes the missing-retract-verb gap this decision surfaced while dogfooding the MCP door.
