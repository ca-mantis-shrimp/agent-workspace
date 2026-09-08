---
type: Field Report
title: From change explanation to continuation insight
description: Reports the post-diff plot dogfood: stable claim amendment and why-rich staleness feel trustworthy, while the compact wake projection still omits decision-bearing checkpoint context needed for correct continuation.
tags: [evaluation, dogfood, foreign-repository, resume, projection, insight]
generated: { by: agent/cli, at: 2026-09-08T00:44:47Z }
---


*Written after the next `plot` session, immediately after diff-on-stale,
rewrite/relocation probing, and append-only `amend_claim` had shipped. This is a
field report and design direction, not a normative projection contract.*

## What happened

The session resumed after the bar-mark checkpoint. Brief `workspace_status`
showed current claims, the old bar objective, and the latest checkpoint label;
`workspace_delta` correctly reported no changes since that checkpoint. The user
asked to continue with new work, so the remaining milestone item was identified
as axis-label formatting.

The first implementation interpreted that phrase generically: separate tick
presentation from SVG coordinate serialization, retain readable decimals, and
use scientific notation for extreme magnitudes. That was coherent in isolation,
but it was not the decision already made. A later full status exposed the prior
checkpoint note:

> axis label formatting = SI suffixes (1.2k, 3.4M), agreed as the slice after bar

The implementation was corrected to SI prefixes. The information had not been
lost and the workspace was internally honest; it simply was not promoted into
the bounded continuation surface where it could guide the next action.

The new write semantics worked well after the correction. The axis-formatting
belief kept stable claim id 12 and was amended from planned to implemented;
claims 9 and 11 were amended in place after their cited renderer/documentation
changed. This felt materially cleaner than manufacturing replacement claims for
truths whose identity had not changed. Why-rich staleness and diff-on-stale also
make a changed claim feel inspectable rather than accusatory: the agent can ask
what moved instead of re-reading an entire file.

## Finding: recovery now needs prioritization, not more detail

The system has progressed through several layers:

1. capture facts — reads, revisions, fingerprints, edits;
2. establish beliefs — claims with provenance and freshness;
3. explain change — what moved and why;
4. preserve decisions — intent, constraints, and rejected alternatives;
5. surface insight — what matters for the next action.

The first three are now credible enough that the next bottleneck is layer four
becoming layer five. A clean delta answers "what changed?" A current status
answers "what is true?" Neither necessarily answers "what must I remember to act
correctly next?"

This is not an argument for returning full checkpoint records at wake. The full
status recovered the SI decision, but at a context cost far beyond the value of
that one sentence. Nor is it an argument for replacing evidence with a generated
summary. The desired shape is progressive disclosure:

```text
insight -> rationale -> claim -> evidence -> raw event/history
```

The concise layer should be inspectable downward and must not silently erase
uncertainty, disagreement, or source authority.

## Proposed continuation capsule

Add a small, explicitly bounded continuation section to the brief resume
projection. It is a third surface alongside status and delta:

- **status** — what is true now;
- **delta** — what changed since the checkpoint, and why;
- **continuation** — what is decision-relevant for acting next.

At the pre-formatting checkpoint, the useful projection would have been roughly:

```text
Objective
  Complete milestone 5.

Resume from
  plot-handoff-log-scale-clean

Next
  1. Add zero-anchored bar marks.
  2. Then add SI-suffix axis labels: 1.2k, 3.4M.

Decisions / constraints
  - Bars expand the linear y-domain to include zero.
  - Bar plus log scale is invalid.
  - Axis formatting means SI suffixes, not generic scientific notation.
  - x-log remains deferred because x may be temporal.

Ground
  claim 9 current — log y-scale is implemented
  claim 10 current — CSV/model pipeline remains intact

Changes since checkpoint
  none
```

After the formatting slice, the same bounded space should emphasize completion,
validation, and residual state rather than replay the implementation:

```text
Completed
  Milestone 5: bars, log y-scale, SI axis labels.

Validation
  42 tests; fmt, clippy, LSP, and diff check clean.

Working tree
  Three intended modified files; unrelated .mcp.json remains untracked.

Ground
  claim 9 current — log scale
  claim 11 current — bar marks
  claim 12 current — SI axis formatting
```

Claim references should resolve stably to the active claim while retaining a way
to request an immutable historical revision. The exact reference syntax remains
a contract decision; the important property is that a concise bullet can be
expanded without searching by paraphrased headline.

## Prefer explicit write-time structure over retrospective inference

The lowest-cost improvement is to include a bounded excerpt of the latest
checkpoint note in brief status. That would have prevented this miss.

The stronger shape is to let checkpoints optionally carry structured handoff
fields alongside durable prose:

- `completed`
- `next`
- `decisions`
- `constraints`
- `risks`
- `validation`
- stable claim references supporting each item

These fields should be optional, cardinality-bounded, and authored or confirmed
at checkpoint time. Retrospectively asking a model to infer the "important"
parts of arbitrary prose risks producing confident but unauditable summaries.
A renderer may rank explicit fields and omit low-priority items with an omission
count; it should not invent a decision that was never recorded.

This is intentionally more nuanced than the original checkpoint/delta design.
That nuance is earned by the evidence: once a system reliably retains details,
its value shifts toward selecting the small number of details that preserve a
line of thought. The target experience is not querying a database after every
restart. It is resuming a reasoned course of action, with every concise insight
still traceable to the underlying claim and evidence.

# Related Concepts

- [Second foreign dogfood — write-loop papercuts and their fixes](plot-foreign-dogfood-write-loop.md): Follows the write-loop report after amend_claim and diff-on-stale shipped, evaluating what those fixes solve and what the compact resume projection still omits.
- [S7 bounded perception](../design/s7-bounded-perception.md): Extends bounded perception from cardinality control toward a decision-bearing continuation capsule with progressive disclosure.
