---
type: Evaluation Protocol
title: Brief-only resume with placement control and objective-drift detection
description: A falsifiable dogfood protocol for the next plot cold resume: brief status and delta only, a two-arm guardrail placement control, and an objective-drift dimension motivated by the categorical-all-marks session.
tags: [evaluation, dogfood, foreign-repository, resume, projection, protocol]
generated: { by: muse-spark, at: 2026-09-08T00:00:00Z }
---

# Protocol — brief-only resume with placement control and objective-drift detection

*Follows [Checkpoint summary follow-up](plot-checkpoint-summary-follow-up.md) and the
checkpoint-capsule disambiguation thread, whose resolution this protocol implements. Motivated in part by the
categorical-all-marks `plot` session (belief 14, checkpoint
`plot-categorical-all-marks`), in which brief orientation succeeded but the
workspace objective described already-finished work.*

## Question under test

Can a cold agent continue correctly from **brief `status` + `delta` alone** —
without a structured continuation capsule and without a first-class constraint
entity? Three sub-questions, each with its own pass criterion:

1. **Sufficiency.** Does the brief surface carry state, next action, settled
   decisions, intentional dirt, and guardrails?
2. **Placement vs. structure.** If a guardrail is missed, is the fix
   checkpoint-authoring discipline (write it early) or new mechanism (a
   `guardrails` capsule field)?
3. **Objective drift.** Does the agent notice when claims are all `current`
   yet the objective sentence no longer describes reality?

## Setup (all arms)

- Fresh agent, rebuilt MCP server (no version skew between binary and surface).
- Target repo: `plot` at checkpoint `plot-categorical-all-marks`
  (all milestones 1–6 plus categorical-for-all-marks done; `.mcp.json`
  untracked by intent).
- Contamination controls, enforced until the agent states its continuation:
  1. only default brief `status` and `delta`;
  2. no ambient prose memory for the target workstream;
  3. no `full:true`, no git history/status, no file reads.
- The agent must state, **before any read**: current state, proposed next
  action, settled decisions it will not reopen, intentional dirt vs. unfinished
  work, and guardrails it will obey.
- Only then reveal full status (and files) and score each dimension as
  recovered or missed. Any control violated before the statement voids the
  affected dimension, not the whole run — log it.

## Arm A — sufficiency (brief excerpt as shipped)

Checkpoint note excerpt as currently implemented (bounded prefix). Score five
dimensions: state, next, decision, intentional dirt, guardrail
(`.mcp.json` untouched, pipeline preserved, numeric/time behavior preserved).
Pass = all five recovered with no constraint entity.

## Arm B — placement control (the thread's correction)

Same as Arm A, but run twice:

- **B1:** one load-bearing guardrail placed *after* the excerpt prefix
  (current authoring habit).
- **B2:** the same guardrail placed *inside* the prefix.

If B2 recovers what B1 misses, placement was the fix and the `guardrails`
capsule field is **not** earned. Only a miss in *both* arms justifies
checkpoint-local structure; only repeated carry-forward failures across
checkpoints justify the standalone constraint entity (per the already-gated
[durable-constraints decision](../decisions/durable-constraints-portable-intent-entity.md)).

## Arm C — objective drift (new, from this session)

At `plot-categorical-all-marks` the brief surface reads: 6 claims `current`,
clean delta — while the objective still says "complete M5, then begin M6,"
both of which are committed. Score:

- **Pass with flag:** agent notices the intent/reality gap, names it, and
  asks or proposes a rebound objective before inventing work. (This is what
  the motivating session did: milestones exhausted per README, so it asked
  the user and bound the next slice explicitly.)
- **Fail silent:** agent treats freshness as correctness and starts work
  under the stale objective without comment.
- **Fail noisy:** agent distrusts the whole projection and re-derives
  (git log, full re-reads) instead of flagging the one stale sentence.

Arm C is the only arm where asking the user counts as evidence rather than
contamination — provided the flag names the gap first.

## What each outcome earns

- Arms A+B1+B2 all pass → excerpt + authoring discipline suffice; do not
  build the capsule.
- B1 misses, B2 passes → fix is placement discipline (write guardrails
  early), still no new mechanism.
- Misses in both B arms on guardrails only → earn checkpoint-local
  `guardrails` presentation with hard negative boundaries (no identity, no
  lifecycle, latest-checkpoint only, capped with omission count).
- Repeated manual carry-forward of the same guardrail across objectives →
  promotion signal for the first-class constraint entity. Not before.
- Arm C fail-silent or fail-noisy → the gap is objective-sync, not capsule
  shape: explore staleness for intent (e.g. objective review at checkpoint
  time), not more continuation fields.

## Non-goals

- No eight-field capsule implementation in this round. The candidate JSON in
  the follow-up report stays a falsifiable hypothesis.
- No retrospective inference: continuation content must be authored or
  confirmed at checkpoint time, never invented from prose after the fact.
- No extending the excerpt length as a substitute for ranking: longer
  prefixes spend context without guaranteeing the right sentence appears.

# Related Concepts

- [Checkpoint summary follow-up](plot-checkpoint-summary-follow-up.md): The contaminated run this protocol isolates.
- [From change explanation to continuation insight](plot-continuation-insight.md): The SI-formatting miss that started the continuation thread.
- [Durable constraints are a portable intent entity, distinct from claims](../decisions/durable-constraints-portable-intent-entity.md): The gated entity this protocol refuses to build early.
- [S7 bounded perception](../design/s7-bounded-perception.md): The bounded, progressively disclosed surface this protocol tests before extending.
