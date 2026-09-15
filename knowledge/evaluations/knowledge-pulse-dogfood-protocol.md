---
type: Evaluation Protocol
title: Predeclared knowledge-pulse dogfood protocol
description: Predeclared cold-agent knowledge-inheritance experiment in the foreign plot repository across two harness surfaces (Claude Code and Pi), measuring whether the wake's governing binding lets a cold successor inherit the OKF/workspace knowledge boundary cheaply instead of the 2026-09-15 Claude baseline's 18-turn, 15-call, 82.5 s, $0.672 archaeology. Defines the task, the cold-start prompt (no destination hint), the success threshold, probes, and honest reporting. Execution requires owner sign-off before any paid run.
tags: [evaluation, knowledge, dogfood, protocol, wake, continuity]
generated: { by: agent/pi, at: 2026-09-15T21:36:56Z }
---

# Protocol — predeclared knowledge-pulse dogfood

*Implements the Clearhead action `knowledge-pulse-dogfood`. The question,
task, cold-start prompt, harness mapping, success threshold, and probes are
fixed **before** any paid run; deviations during execution are recorded as
they happen, never rationalized afterward.*

**Status:** draft for owner review. Execution is gated on owner sign-off of
this document and its cost ceiling (the Clearhead action runs it; this
document does not).

## 1. What is being tested, and with what

One question: **does the wake's governing binding let a cold agent inherit the
knowledge-boundary rule cheaply — act on it, place provisional state in the
right place, and avoid duplicating curated knowledge — instead of
reconstructing the rule through repository archaeology?**

The boundary rule under test is the one `plot` binding **k1** pins:
[the OKF curated-knowledge-layer decision](../decisions/okf-curated-knowledge-layer.md).
Its load-bearing consequence for a cold agent working in `plot` is:

- durable, human-reviewable design commitments belong in the repository's
  curated knowledge (for `plot`, today: the README's design-commitments
  section — `plot` has no `knowledge/` bundle of its own);
- revision-relative beliefs, current attention, uncertainty, and findings
  belong in Agent Workspace state;
- harness-local memory is neither project authority nor the continuity
  substrate.

The baseline this must beat is the 2026-09-15 cold Claude probe: it found the
governing decision and placed its belief correctly only after **18 turns,
15 tool calls, 82.5 s, and $0.672** of archaeology — and its prompt supplied
the principle itself, so it tested destination discovery, not salience. This
prompt must not.

No unbuilt machinery is used. The substrate is exactly what shipped in
`knowledge-pulse-kernel` and `orientation-worktree-context`: the wake summary's
`governs` line, one-call `workspace_reveal`, and the worktree-scoped last-stop
anchor.

## 2. Setup

- **Repository:** the foreign `plot` repo, on a fresh branch off `e2929a2`
  (working tree clean, milestones 1–6 complete). One branch per harness run,
  so the two runs do not observe each other's landed work.
- **Binding under test:** `plot` binding k1, pinned to
  `../agent-workspace:knowledge/decisions/okf-curated-knowledge-layer.md`
  (pinned at `28dda4a`). Before any run, re-verify the source reads `current`
  and record the pinned revision and content hash in the report.
- **Wake precondition:** `orientation-worktree-context` shipped; `plot`'s
  existing checkpoints are legacy (unattributed) and are correctly omitted
  from the wake. Record **one fresh stamped checkpoint** in `plot` before the
  run so the `stopped at` line is meaningful, and verify the wake still fits
  the 1000-byte budget with the `governs` line present.
- **Harnesses:** two cold surfaces, both with their live capture adapters
  installed in `plot` (`.claude/settings.json` for Claude Code; `.pi/` for
  Pi):
  - **Arm A — Claude Code** (the baseline's harness; the comparability arm).
  - **Arm B — Pi** (this harness).
  The mapping is fixed here so it cannot be chosen after seeing results.
- **Isolation:** each run starts from a cold session — no conversational
  recap, no prior-turn context, no destination hint. The only orientation is
  the kernel-rendered wake the harness prints at SessionStart (and the task
  prompt in §3).
- **Attribution:** the wake and claims are attributed, not authenticated; this
  experiment proves nothing about who recorded what, only about cheap
  inheritance.

## 3. Task and cold-start prompt

The task is a real, consequential `plot` change whose completion requires the
agent to (a) make one durable design decision and (b) form at least one
working belief, so correct placement is observable. The default task:

> **Milestone 7 — chart title.** Implement a `--title TEXT` option on every
> mark so the SVG carries a centered title above the plot area, with a sane
> default size and placement that respects `--width`/`--height`. Then record
> the title placement/sizing decision durably, and record your working
> beliefs the way you are supposed to.

The owner may substitute any other real `plot` task of the same shape
(produces one durable decision + one belief); the substitution is recorded in
the report, not silently.

**The predeclared cold-start prompt** (identical for both arms, modulo the
harness's own framing) is:

> You are starting a fresh session in the `plot` repository. Your task is:
> implement a `--title TEXT` option on every mark (scatter, line, bar) so the
> output SVG includes a centered title above the plot area, with a sane
> default size and placement that respects `--width`/`--height`. Add a test.
> When you finish, record the title design decision somewhere durable, and
> record any working beliefs you formed in the place this project uses for
> them. Begin.

This prompt deliberately names no storage system, no principle, no
destination, and no `../agent-workspace` path. If either arm is given any
hint beyond this, the run is void and re-run.

## 4. Stop budget

- Per agent: at most **20 harness turns** and the harness's natural context
  budget; whichever exhausts first is recorded as the outcome, not a failure
  to hide.
- Total: **≤ 3 hours wall clock** across both arms, including probe runs.
- **Cost ceiling:** owner-set; suggested **$3.00 total** across both arms.
  The run stops at the ceiling with the partial result published as-is.
- Stop early once the inheritance question is answered; polish past the
  measurement is waste.

## 5. Required behaviors (from the action, made observable)

For each arm, the transcript must let a reviewer answer:

1. **Acted on the binding.** Did the agent cite or act on the wake's `governs`
   line (k1) before performing any broad corpus search or reading repository
   links? Score: `acted-on-binding` / `re-derived` / `neither`.
2. **Opened the source only on demand.** Did it open the pinned decision file
   only when the task required its content, or immediately as a reflex?
3. **Placed provisional state in Agent Workspace.** Its working beliefs landed
   via the workspace write loop (`workspace_record_belief` or equivalent),
   not harness-local memory, not a comment, not an uncommitted note.
4. **Did not duplicate curated knowledge.** It did not re-create the
   boundary decision or a duplicate design record where one already exists;
   it did not write a durable operating rule as a claim.
5. **Reported honestly.** If a source state, omission, or false-relevance
   condition arose, it was named honestly — not silently papered over or
   invented (§6 probes).

## 6. Probe schedule (fixed in advance)

The primary measurement is §5 over the clean task. Probes P2–P4 run in
**disposable state only** after the primary run, so they never contaminate the
inheritance measurement.

1. **P1 — false relevance (observed in primary run).** A second binding
   scoped to a path the title task never touches (e.g. `src/time.rs`) is
   present at wake. Correct behavior: the agent does not treat it as
   governing its task; it is not silently generalized to repository scope.
2. **P2 — source change (disposable).** After orientation, the pinned
   decision in `../agent-workspace` is edited and committed. Correct
   behavior: the agent's next `status`/`delta` surfaces the source as
   `changed` and it reports that honestly, rather than pretending the pin is
   still current.
3. **P3 — unavailable provider (disposable).** The `../agent-workspace`
   repository is moved aside. Correct behavior: the wake still renders with
   the headline and reference, the source reads `unavailable`, and the agent
   reports unavailability rather than inventing content. Restore afterward.
4. **P4 — bounded omission (observed).** With more applicable bindings than
   the governs cap (3), the agent reveals (`workspace_reveal`) rather than
   assuming the omitted bindings do not exist or are irrelevant.

## 7. Independent checks

- Correct placement is verified against the actual workspace state and the
  repository, not the agent's declaration: the durable decision is in the
  right durable location; the belief is an Agent Workspace claim (not
  harness memory); no duplicate of the boundary decision exists.
- The discovery-call count (turns, tool calls, wall-clock, cost) is verified
  against transcripts and harness telemetry, not self-reported.
- The §10 use test is recorded per run: **which wake lines were acted on or
  cited; which `workspace_reveal` calls were made; which facts were
  re-derived that the wake already carried.** A section never used across
  runs is a removal candidate.

## 8. Scoring and the decision rule

- **Primary metric:** orientation discovery cost before correct placement —
  turns, tool calls (reads/searches/reveals), wall-clock seconds, and
  attributable cost — compared against the baseline (18 / 15 / 82.5 s /
  $0.672). Success is **materially fewer**: predeclared as **≤ 8 tool calls
  and ≤ 8 turns** to reach a correctly-placed belief, in each arm. (Owner may
  adjust the threshold before sign-off; the number is fixed before the run.)
- **Gating behaviors:** §5.1 `acted-on-binding`, §5.3 `placed in workspace`,
  and §5.4 `no duplication` must all hold for an arm to count as passing,
  whatever the call count. A cheap-but-wrong placement is not a pass.

**Decision rule.**

- **Keep** — both arms beat the threshold and satisfy the gating behaviors;
  the wake carries the boundary rule cheaply and harness-neutrally.
- **Change** — mixed or partial: one arm passes, or the call count beats but a
  gating behavior fails. Name exactly what would have to change.
- **Reject** — neither arm beats the baseline, or the binding added ceremony
  without better placement or lower burden.

A negative or stopped result is successful completion of the action and
publishable evidence — not a reason to enlarge the system or invent a
flattering measure. Do not generalize beyond observed evidence.

## 9. What this experiment is not

- **Not a multi-arm trial.** Two harnesses, one task, n = 1 per harness, no
  order controls, no task matrix.
- **Not a trust or capture-integrity proof.** Records are attributed, not
  authenticated.
- **Not a retrieval benchmark.** It measures one governing binding, not the
  OKF search surface or the whole knowledge bundle.
- **Not authorization to build anything.** No machinery may follow from a
  passing run; a failing run may reopen nothing by itself.

## 10. Uncertainty and reporting

Single task, single run per harness, two different harness models, one owner.
Comparisons are reported with explicit uncertainty; directional findings are
claims about the wake's salience, not about every harness. The report (a field
report under `knowledge/evaluations/`) retains transcripts, costs, failures,
false-relevance and omission behavior, the §10 use-test record, and the
keep/change/reject disposition.

## Related concepts

- [Knowledge pulse contract](../specifications/knowledge-pulse-contract.md): Normative binding semantics, wake entry (KP14), and the baseline evidence in §0.
- [Wake summary contract](../specifications/wake-summary-contract.md): The wake surface under test, including the §10 use test this protocol instruments.
- [Use OKF for curated project knowledge](../decisions/okf-curated-knowledge-layer.md): The governing decision k1 pins; the rule whose inheritance is measured.
