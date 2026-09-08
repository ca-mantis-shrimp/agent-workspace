---
type: Field Report
title: Checkpoint summary follow-up — sufficient content, inconclusive transport
description: Reports the next plot dogfood after checkpoint-note promotion; continuation succeeded and the guardrail held, but full status, ambient memory, and version skew contaminated the test, so a first-class constraint entity remains friction-gated.
tags: [evaluation, dogfood, foreign-repository, resume, projection, constraints]
generated: { by: pi/gpt-5.4, at: 2026-09-08T04:27:57Z }
---

# Field report — checkpoint summary follow-up

*Written after the first `plot` continuation following the addition of a bounded
latest-checkpoint note to brief status and delta. This is a field report, not a
projection contract. It follows [From change explanation to continuation
insight](plot-continuation-insight.md), whose SI-formatting miss motivated the
change.*

## Outcome

The continuation succeeded. The agent recovered that Milestone 5 axis-label
formatting was complete but uncommitted, committed it as `73111c1`, selected the
next natural slice, implemented categorical bar axes through a band scale, and
committed that work as `c952db1`. The explicit guardrail to leave the target
repository's untracked `.mcp.json` alone was obeyed. Validation finished with 44
tests, formatting, Clippy with warnings denied, an end-to-end categorical-bar CLI
smoke check, Rust LSP, and Pi Lens clean.

That is evidence that the *content* of the narrative handoff was sufficient for
correct continuation. It is not clean evidence that the new brief
`latest_checkpoint.note` excerpt was the mechanism that carried it.

## Why the transport result is inconclusive

Three factors contaminated the test.

1. The agent explicitly requested `workspace_status(full: true)` at orientation.
   The full checkpoint note therefore entered context whether or not the bounded
   brief excerpt would have been enough.
2. Pi's ambient session memory also supplied a compact prior-run summary. That
   summary included the exact worktree condition and the `.mcp.json` guardrail.
3. A later default `workspace_status` call from the already-running MCP session
   projected the checkpoint label and sequence but no note field. The checked-out
   Agent Workspace repository contained the new implementation at `ee2a565`, so
   this is consistent with process/version skew: MCP tool surfaces are snapshotted
   at session start. The live client was not a clean test of the newly installed
   binary.

The earlier conversational conclusion — “the new summary was sufficient” — was
correct about the information available to the agent, but too strong about its
provenance. The honest verdict is:

> Continuation content sufficient; brief checkpoint-excerpt delivery not
> isolated; first-class constraints neither justified nor ruled out.

There is a second limitation in the excerpt design itself. The checkpoint note
used for this continuation began with implementation and validation details. The
`.mcp.json` guardrail appeared later than the first 200 characters. A fixed
prefix excerpt reliably promotes the opening sentence, which would have solved
the prior SI-formatting miss if that decision appeared early, but it cannot
reliably preserve a constraint placed later in otherwise well-written prose.

## What felt good

The semantic write loop felt coherent under real feature churn:

- the objective was rebound before the new slice;
- the already-validated Milestone 5 work was committed and checkpointed before
  changing direction;
- one design belief was amended from planned to implemented without losing its
  stable identity;
- four older active claims that became stale through shared model, renderer, and
  documentation edits were amended in place rather than replaced ceremonially;
- the final workspace had six current claims, zero stale claims, zero findings,
  and no open transactions.

The narrative guardrail also produced the intended behavior: `.mcp.json` stayed
untracked and untouched across both commits without enforcement machinery.

## What did not feel solved

The agent still ran `git status` and `git log` during initial orientation despite
a current orientation claim explicitly saying not to re-derive current workspace
state that way. This is the same defensive reflex the foreign dogfood exists to
measure. A first-class constraint entity would not fix it: the relevant
instruction was already explicit, current, and visible. The remaining problem is
behavioral trust and projection salience, not merely vocabulary.

The continuation also showed that one prose field is being asked to carry several
different jobs:

- completed work;
- immediate next action;
- prior decisions that close design forks;
- guardrails such as “do not touch this file”;
- intentional worktree dirt;
- validation state;
- open questions.

A prefix excerpt cannot rank those jobs. Making the excerpt longer only spends
more context without guaranteeing that the right sentence appears.

## Recommended summary structure

The next low-cost experiment should be a bounded, structured continuation summary
inside brief **status**. It should remain authored or confirmed at checkpoint
time; it must not be retrospectively invented from arbitrary prose.

```json
{
  "continuation": {
    "as_of_sequence": 944,
    "revision": "c952db1",
    "state": "Milestone 6 categorical bar axes are complete and committed.",
    "next": [
      "Decide whether categorical x should extend to line or scatter."
    ],
    "guardrails": [
      "Preserve numeric and time-axis behavior.",
      "Keep the parse→model→scale→axis→mark→svg pipeline.",
      "Do not modify the untracked .mcp.json."
    ],
    "decisions": [
      "Categorical x uses first-appearance indices plus label metadata.",
      "Categorical x currently requires the bar mark."
    ],
    "working_tree": {
      "summary": "Only .mcp.json is intentionally untracked."
    },
    "verification": [
      "44 tests passed",
      "fmt and clippy clean",
      "LSP and Pi Lens clean"
    ],
    "open_questions": [
      "Does narrative guardrail handling remain reliable across cold resumes?"
    ],
    "basis": {
      "checkpoint": "plot-milestone-6-categorical-bars",
      "claim_ids": [9, 10, 11, 12, 13]
    }
  }
}
```

The semantics should stay narrow:

- **`state`** says what is true now;
- **`next`** carries only the immediate continuation, not the roadmap;
- **`guardrails`** promotes action-shaping prose without yet creating a new
  durable entity type;
- **`decisions`** prevents settled forks from silently reopening;
- **`working_tree`** distinguishes intentional dirt from unfinished work;
- **`verification`** reports what was last proven without pretending the text is
  content-addressed evidence;
- **`open_questions`** keeps uncertainty visible;
- **`basis`** provides progressive disclosure into checkpoint and claim history.

Each collection should have a small cardinality cap and an explicit omission
count. The whole summary needs `as_of_sequence` and revision provenance so it
cannot masquerade as timeless prose. If any cited active claim is stale, status
should show that fact rather than regenerate a reassuring sentence.

## Status versus delta

This belongs primarily in **status**:

- `status` answers “where can I safely continue now?”;
- `delta` answers “what changed since the selected checkpoint?”

Delta should not duplicate the whole continuation capsule on every call. It can
report whether the capsule changed, which sections changed, and optionally the
current bounded capsule when a transition occurred:

```json
{
  "continuation_change": {
    "changed": true,
    "from_sequence": 757,
    "to_sequence": 944,
    "sections_changed": ["state", "next", "decisions", "verification"],
    "current": "<the same bounded continuation projection>"
  }
}
```

The rendering and bounds should be kernel-owned and shared by status and delta;
adapters remain transports.

## Constraint decision

Do not implement the first-class constraint entity yet. The existing decision
remains correctly friction-gated: [Durable constraints are a portable intent
entity, distinct from claims](../decisions/durable-constraints-portable-intent-entity.md).
This run produced no constraint miss. It instead produced a test-design problem
and evidence that a typed *section* within a checkpoint-authored summary may be
enough.

Promote constraints to their own lifecycle only after repeated evidence that
summary-local guardrails fail because they need one of the properties a section
cannot supply: survival across unrelated objectives, independent
supersession/retirement, binding to multiple future actions, or optional linkage
to an external authority.

## Next falsifiable dogfood

Run a fresh agent with the rebuilt MCP server and deliberately isolate the wake
surface:

1. provide only default brief `status` and `delta`;
2. do not inject ambient prose memory for the target workstream;
3. forbid full status, Git history, and file reads until the agent states its
   proposed next action and guardrails;
4. place one important guardrail outside the checkpoint note's first 200
   characters;
5. record whether the agent recovers state, next action, decision, intentional
   dirt, and guardrail;
6. only then reveal full status and compare.

Pass means the structured summary carries all five without a first-class
constraint entity. Failure should identify which semantic property was missing;
only lifecycle-shaped failures justify building the entity already designed.

# Related Concepts

- [From change explanation to continuation insight](plot-continuation-insight.md): The prior report whose salience miss prompted the bounded checkpoint-note excerpt.
- [Durable constraints are a portable intent entity, distinct from claims](../decisions/durable-constraints-portable-intent-entity.md): Keeps the separate entity designed but gated on observed friction.
- [Second foreign dogfood — write-loop papercuts and their fixes](plot-foreign-dogfood-write-loop.md): Earlier foreign-repository evidence about amendment, stale explanation, and write-loop cost.
- [S7 bounded perception](../design/s7-bounded-perception.md): Governs the bounded, progressively disclosed resume surface proposed here.
