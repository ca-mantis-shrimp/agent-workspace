---
type: Field Report
title: Second foreign dogfood — write-loop papercuts and their fixes
description: Reports the second foreign-repository session in plot, the first to observe claims going stale in-anger and to exercise supersede heavily, and proposes amend_claim and diff-on-stale.
tags: [evaluation, dogfood, foreign-repository, write-api, freshness]
generated: { by: claude-code/opus-4.8, at: 2026-09-07T19:51:52Z }
---

# Field report — second foreign dogfood, the write loop under churn

*Written by the working agent after a session in the `plot` repo using the
Claude Code adapter and the workspace_* MCP verbs. It follows
[plot-foreign-dogfood.md](plot-foreign-dogfood.md) (the first foreign trial,
which built milestones 3–4). This session built the log-scale slice of
milestone 5 and — for the first time in a foreign repo — drove the
`record_belief`/`supersede_claim` loop hard enough to watch claims go stale
mid-work. A field report, not a spec.*

## What happened

Resumed cold with claim 5 (milestone 3) projected **stale**. It was not false:
the claim rested on `README.md`/`src/main.rs`/`src/parse.rs` pinned at the
milestone-2 revision, and the m3+m4 commits had rewritten exactly those files.
The session recorded claim 8 (current render/scale ground, on live files),
superseded 5→8, then built the log-scale slice (a `Scale` enum wrapping
`LinearScale`/a new `LogScale`, a `ScaleKind` selector, `--yscale linear|log`),
recorded claim 9, and — because the new code falsified claim 8's own "no log
scale exists / three-arg `chart`" assertions — superseded 8→9. Committed
(`90cc474`), checkpointed. Immediately after the commit, the wake projection
showed **claim 7 (milestone 4) newly stale**: the commit had touched
`README.md` and `src/main.rs`, two of claim 7's cited files.

## Findings

**1. False-stale is real, was observed twice in-anger, and is a *true*
positive.** The predecessor report closed with "nothing drifted mid-session, so
no claim was observed going stale in-session." This session is the opposite:
claim 5 stale at wake, claim 7 stale seconds after a commit. Both fired because
the claim cited files that legitimately churned. The important correction to my
own first reaction: **this is not the freshness core misfiring.** `parse.rs`
really was rewritten by the CSV refactor under claim 5; `main.rs` really gained
a flag under claim 7. The evidence changed in a way that *could* have
invalidated the claim. What stung was not the flag but the **cost of
confirming it hadn't** — re-reading a whole file to answer "did this diff touch
what I asserted?" Note this is a *different* axis from the formatter-skew
false-stale the repo already defends against with the pinned toolchain + fmt
gate (see AGENTS.md): neither of these was formatter noise, so the pinned
toolchain does not cover it.

**2. Whole-claim-only supersede forces a broad-vs-narrow dilemma.** Claim 8 was
a good, honest, *broad* description of the render/scale subsystem. My own edits
falsified two of its sentences while leaving the rest (palette, legend,
>8-group rejection, `time_ticks` routing) true and unchanged. `supersede`
could only retire it *whole*, so replacing it with the *narrow* claim 9 forced
me to argue in the supersede reason that the still-true parts were "covered
elsewhere" (claim 7 + committed history). That is a workaround, not a modeling
operation. The verb set pushes agents toward either sprawling claims that
constantly stale or atomic claims that fragment the picture — with no way to
*move a belief partway*.

**3. The reuse optimization split predicted in finding #3 of the first report
was reproduced, both branches, in one session.** The cleanup belief (claim 8),
recorded on a read-heavy turn before any edit, came back `reused: true` for all
four supports. The post-edit belief (claim 9) came back `reused: false` for all
four — the kernel correctly re-captured because my edits had moved the files.
Same session, both paths, confirming the write loop (read→edit→commit→claim)
mostly re-captures and the reuse win is a read-turn phenomenon.

**4. Self-falsification within a session is legible but raises an
earned-existence question.** Claim 8 lived ~15 minutes before I retired it with
my own code. The 5→8→9 supersede chain now reads as an honest history of
changing understanding, which is a point *for* the model. But some of claim 8's
value was that it made claim 9's "before" legible; absent the explicit
write-loop test, I might not have recorded it at all. Agents will feel a pull
to record beliefs to *perform* the loop rather than because the belief earns
its keep. The tool can't police this, but design docs should not encourage
belief-recording as ceremony.

**5. Receipts are heavy.** Every write returns a large JSON record
(fingerprints, normalizers, mediated-unit coverage, reconciliation hashes). The
signal an agent acts on is `{id, freshness, reused, replacement}`. The audit
detail belongs in the store, not echoed into the working context every call.

## Proposals, ranked by leverage per cost

**1. `amend_claim` — cheapest, highest leverage; do first.** A verb to revise a
claim's statement in place, keeping its id and lineage, so a belief becomes
*durable and editable* rather than write-once-and-retire. This dissolves
finding #2 entirely: you can write honest broad beliefs and move them as
reality moves, instead of pre-atomizing to dodge staleness. It is a new
operation over data that already exists — no new machinery. **Integrity
caveat: amend must *append a revision* (visible history), never overwrite** —
otherwise the lineage can lie about what was once believed, undermining the one
thing the loop is for.

**2. Diff-on-stale — attacks the cost finding #1 identifies, keeps the signal.**
When a cited file changes, surface the *hunk* alongside the stale flag, not just
"stale, re-verify." Re-verification drops from O(whole file) to O(diff): "does
this change touch what my claim asserts?" For claim 5 that is a two-second "no,
the CSV refactor didn't alter first-appearance grouping" instead of re-reading
`parse.rs`. This makes the true-positive staleness *worth having* rather than a
tax. Relates to [structural freshness without formatter
coupling](../research/structural-freshness-without-formatter-coupling.md).

**3. Tiered receipts — trivial.** Default the write verbs to
`{id, freshness, reused, replacement}`; return the full audit record only on
request or via a `--full` projection.

**4. Sub-file / symbol selectors — the real cure for granularity, but a
project, and I'd rank it *below* diff-on-stale.** Binding a belief to
`svg::chart`'s span instead of the whole file would stop README/flag churn from
staling unrelated claims. The `relocation_fingerprint` and an available LSP
authority suggest groundwork exists. But it needs per-language span extraction
*and* relocation across edits, and — philosophically — it reduces noise by
*suppressing a real signal*, where diff-on-stale keeps the signal and cuts its
cost. Pursue for granularity, not as the false-stale fix.

If only one ships: **amend**. The others are quality-of-life; amend changes what
a belief *can be*.

## What this session did not test

Multi-actor overlap, transactions (again correctly unused — solo, sequential),
handoff/export, and reconcile. `bind_objective` is now an MCP tool (slice-2
recommendation from the first report appears to have landed) but was not
exercised — the objective needed no rebind this session.

*— the plot session agent (Claude Code / Opus 4.8), 2026-09-07*

# Related Concepts
- [First foreign dogfood of the semantic write API](plot-foreign-dogfood.md): Follows and extends the first foreign dogfood: supplies the in-anger staleness and heavy-supersede evidence its 'did not test' section left open.
- [Structural freshness without formatter coupling](../research/structural-freshness-without-formatter-coupling.md): Diff-on-stale and the true-positive reframe of false-stale bear directly on structural freshness.
- [Semantic write API](../design/semantic-write-api.md): Proposes amend_claim (append-only) and tiered receipts as revisions to the semantic write API.
