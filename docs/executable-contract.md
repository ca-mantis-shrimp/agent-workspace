# Executable Contract — Agent Workspace MVP (2026-09-01)

*Infrastructure-agnostic behavioral contract. It precedes the choice of storage/language, per
the charter's `contract` action. Every scenario is a concrete, checkable behavior — the walking
skeleton made rigorous — and every failure-model item is something that must **never** be
observed. It synthesizes [`design-note-agent-perspective.md`](design-note-agent-perspective.md)
and [`response-to-agent-perspective.md`](response-to-agent-perspective.md); it does not restate
the mechanics already in [`initial-design.md`](initial-design.md).*

## 1. Vocabulary

- **Observation** — automatically captured provider output or bounded source material, carrying
  reproducible input fingerprints. Not an interpretation.
- **Claim** — an actor's interpretation, referencing its supporting observations and carrying a
  **dependency scope**: explicitly declared, or a conservative default when undeclared.
- **Evidence** — the outcome of a named check, bound to its exact invocation and relevant
  inputs, supporting stated acceptance claims.
- **Freshness state** — one of:
  - **`current`** — supported by tracked inputs whose relevant fingerprints still match. A
    *positive, reproducible* claim.
  - **`stale`** — a supporting input changed, or semantic relocation no longer resolves
    unambiguously.
  - **`unknown`** — the workspace lacks enough tracked information to assert either.
- **Coverage boundary** — the set of paths/operations the workspace is currently observing.
  Visible in status; may shrink when out-of-band activity occurs.

## 2. Invariants (must always hold)

1. **`current` is never inferred from silence.** Absence of a stale warning must never establish
   freshness; `current` is only ever a positive, reproducible claim over tracked inputs.
2. **No `current` over an untracked dependency.** If a relevant dependency cannot be excluded,
   the verdict is `unknown` (or conservatively `stale`) — never `current`.
3. **Layered invalidation.** A claim's freshness is bounded by *both* its supporting
   observations *and* its dependency scope; a claim may be `stale` while its observed span is
   byte-identical.
4. **Evidence expires with inputs.** Validation evidence cannot support a transaction after its
   relevant inputs change.
5. **No silent rebinding.** Failed semantic relocation yields `stale`/ambiguous, never a guessed
   new binding reported as `current`.
6. **Provenance survives normalization.** Every normalized record retains provider identity plus
   enough native detail (or a content-addressed reference) to reconstruct what the provider said.
7. **Transactions name their base.** A transaction always records its base revision and can
   expose the exact delta from it.
8. **Out-of-band activity degrades honestly.** Raw reads/edits/revision changes must reduce
   coverage or trigger conservative invalidation — never leave a false `current` — and the loss
   of coverage must be visible in status.
9. **Restart from records, not chat.** Restart reconstructs objective, working set, claims,
   transaction, evidence, and freshness from durable records, not conversation history.
10. **Stopping is success.** Checkpoint and handoff are valid *successful* lifecycle outcomes,
    not incomplete-failure states.
11. **Failure ≠ empty; secrets excluded.** Provider failure is recorded distinctly from a
    successful empty result; secrets and unbounded output are not stored by default.

## 3. Failure model (must NEVER be observed)

- **F1 — False-current** *(most dangerous)*: a `current` verdict for a claim whose supporting
  bytes, or whose relevant-but-untracked dependency, changed.
- **F2 — Silent rebind**: relocation guesses a span and reports `current`.
- **F3 — Silence as assurance**: an uncovered region reads as `current`, or coverage loss is not
  surfaced as `unknown`.
- **F4 — Zombie evidence**: stale evidence counts as current validation.
- **F5 — Chat-only state**: any state that cannot be recovered after restart without chat history.
- **F6 — Erased provenance**: normalization discards provider identity or native detail.
- **F7 — Bookkeeping tax**: a bookkeeping-only verb required on the critical path (the adoption
  failure mode).

## 4. Scenarios (executable — Given / When / Then)

**S1 — Observation goes stale after an edit** *(core skeleton)*
Given a tiny Git fixture at revision R and an observation of symbol `foo` recorded at R,
when `foo`'s bytes change to R′, then the observation becomes `stale` with reason
*supporting input changed*.

**S2 — Bypass yields `unknown`, never false-current** *(soundness of the coverage model)*
Given the workspace observing path A only, when a raw out-of-band edit changes uncovered path B,
then any claim depending on B (and B itself) is `unknown`, status shows reduced coverage, and
**no** record anywhere flips to `current` as a result.

**S3 — Inference-dependency guard** *(the false-current case)*
Given a claim "empty emails are rejected" over an observation of `validate_user`, whose
conservative dependency scope includes helper `is_blank` in another file, when `is_blank`
changes but `validate_user`'s bytes do not, then the claim is `stale` (or at worst `unknown`) —
never `current`.

**S4 — Evidence invalidation gates acceptance**
Given evidence from `test X` bound to inputs at R supporting acceptance claim C, when a relevant
input changes, then the evidence is `stale`, C is no longer validated, and the transaction
cannot be accepted on it.

**S5 — Restart recovery**
Given a recorded objective, working set, one open transaction, one claim, and one evidence item,
when the process restarts with no chat history, then the same coherent status — including
freshness states — is reconstructed from durable records.

**S6 — Reversible transaction**
Given a transaction opened at base R with a mutation, when it is reverted, then the repository is
restored to R and affected claims/evidence re-evaluate their freshness.

**S7 — Bounded perception costs less context** *(the inversion, made testable)*
Given a large file, when the agent requests it through the workspace, then it receives structure
(outline/signatures) by default and unfolds only a requested span; and workspace-assisted
reading ingests measurably fewer bytes than the raw read for the same task.

**S8 — Provenance retained**
Given a normalized finding from an analyzer, then the provider identity and native payload (or
its content-addressed reference) remain retrievable.

## 5. Deferred to implementation (not resolved here)

- Smallest safe input fingerprint per evidence type (command / target / file / repository level).
- The conservative dependency scope default for undeclared claims (whole-file? one-hop
  call-graph?) — trades false-current against alarm fatigue; **must be measured, not assumed**.
- How the coverage boundary is declared and discovered.
- The Pi interface form (extension / MCP server / direct package).
- Cognitive proprioception and motivational braking remain **documented research questions**,
  explicitly out of MVP scope.

## 6. Acceptance (behavioral, not architectural)

A first implementation of Git + tree-sitter + retained tool results + careful joins is
acceptable; algorithmic novelty is not the criterion. The MVP earns continuation only if, on
dogfooded maintenance tasks, workspace-assisted operation measurably:

- drives **false-current (F1) to ~zero**;
- reduces bytes/tokens ingested and repeated reads;
- recovers faster after restart;
- does so **without** hiding provider detail (F6) or requiring bookkeeping-only verbs (F7);
- while keeping conservative-invalidation (alarm-fatigue) rates low enough to stay usable.

Metrics extend `initial-design.md` §8 with explicit **false-current** and
**conservative-invalidation** rates. If the composition does not move these numbers, it has not
earned continuation regardless of sophistication.
