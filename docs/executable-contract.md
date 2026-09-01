# Executable Contract — Agent Workspace MVP (rev. 2, 2026-09-01)

*Revision 2 closes the design phase. It folds in both critiques
([response-to-agent-perspective](response-to-agent-perspective.md),
[response-to-executable-contract](response-to-executable-contract.md)) and is the doc of
record from which the walking skeleton begins. Infrastructure-agnostic: it precedes the choice
of storage/language. Every scenario is a concrete, checkable behavior; every failure-model item
must **never** be observed. Further doc-only refinement is explicitly out of scope — open
questions in §5 are settled by running code, not another pass. Amended after the S1 review to
add invariant 7 (claim completeness non-inheritance) — an implementation-evidence hardening,
not a design reopening.*

## 0. Normative imports

Binding by reference (not restated here): `workspace-mvp.md` §*Authority boundaries*,
§*Constraints*, and §*Non-goals for this charter*; and its §*Core concepts* definitions of
objective, workstream, semantic location, finding, and checkpoint. `initial-design.md` is
rationale and background — **not** binding except where a section is named. Where this contract
sharpens an imported term, this contract governs.

## 1. Vocabulary this contract owns

- **Observation** — auto-captured provider output or bounded source, with reproducible input
  fingerprints. Not an interpretation.
- **Claim** — an actor's interpretation, referencing supporting observations and carrying a
  **dependency scope** (declared, or a conservative default when undeclared).
- **Evidence** — a named check bound to its exact invocation and relevant inputs, supporting
  stated acceptance claims.

**Three orthogonal reports** (this replaces the single freshness enum of rev. 1, which unsafely
braided three kinds of knowledge):

1. **Freshness within scope** — `current` | `stale` | `unknown`.
2. **Scope assurance** — `declared` | `derived` | `conservative`, plus a completeness flag
   (`asserted-complete` | `not-asserted`).
3. **Operational coverage** — the set of mediated paths/operations, and the repository
   fingerprint at the last reconciliation boundary.

A result is always the *triple*, never dimension 1 alone.

- **Reconciliation boundary** — a defined point at which the workspace re-reads *the inputs
  relevant to what is being queried* (scoped, to preserve laziness — never a whole-repo scan)
  and recomputes verdicts. The boundaries are: (a) before returning any freshness verdict;
  (b) before validation or transaction acceptance; (c) on opening or resuming a workstream;
  (d) after a mediated mutation; (e) on an optional watcher event.
- **Transaction boundary** — a revision identity *and* a captured initial worktree state (or
  equivalent transaction-owned delta boundary).

## 2. Invariants (must always hold)

1. **`current` is positive and scoped.** `current` means every input in the claim's *recorded*
   dependency scope was reconciled against current repository state at a named boundary and
   still matches. It asserts freshness **within recorded scope only** — never objective truth,
   never completeness of that scope.
2. **No unqualified truth escapes.** A `current` verdict must always travel with its scope
   assurance and its reconciliation fingerprint. Client projections must not present `current`
   without them. *(This is the executable replacement for rev. 1's non-executable "no `current`
   over an untracked dependency": we cannot prove no undeclared dependency exists, so we instead
   refuse to hide the scope.)*
3. **Verdicts name their reconciliation.** Every verdict identifies the fingerprint it was
   computed against; if current state is unreconciled, the verdict is `unknown`, never inherited
   from a prior session.
4. **Reads do not mutate freshness.** A raw (unmediated) read changes no tracked record's
   freshness; it yields a belief with *absent provenance*, not staleness.
5. **Mutations degrade only at boundaries.** A raw edit or revision change is detected at the
   next reconciliation boundary touching the affected inputs; until then, dependent claims that
   cannot be reconciled are `unknown`, never `current`.
6. **Layered invalidation.** Claim freshness is bounded by *both* its observations *and* its
   dependency scope; a claim may be `stale` while its observed span is byte-identical.
7. **Claims never inherit observation completeness.** An observation may report its own scope
   `asserted-complete` because it captures a bounded, fully-fingerprinted payload. A claim's
   scope assurance must be independently established from its declared observations and
   dependency scope, and may never be set to `asserted-complete` by inheritance from a
   supporting observation. *(Violation is an F1 vector.)*
8. **Evidence expires with inputs.** Validation evidence cannot support a transaction after its
   relevant inputs change.
9. **No silent rebinding.** Failed semantic relocation yields `stale`/ambiguous, never a guessed
   binding reported as `current`.
10. **Provenance survives normalization.** Every record retains provider identity plus enough
    native detail (or a content-addressed reference) to reconstruct what the provider said.
11. **Transactions are safely reversible.** A transaction records a revision identity *and* its
    initial worktree state. Revert restores only transaction-owned mutations and reconstructs
    the initial worktree; on ambiguity from overlapping later mutations it **halts with a
    conflict** — never destroys unrelated work, never reports success on a partial revert.
12. **Restart from records.** Restart reconstructs objective, working set, claims, transaction,
    evidence, and the full report triple from durable records, not chat history.
13. **Stopping is success.** Checkpoint and handoff are valid successful lifecycle outcomes.
14. **Failure ≠ empty; secrets excluded.** Provider failure is recorded distinctly from a
    successful empty result; secrets and unbounded output are not stored by default (retention
    and redaction per imported §*Constraints*).

## 3. Failure model (must NEVER be observed)

- **F1 — False-current** *(most dangerous)*: `current` for a claim whose in-scope inputs
  changed, or whose scope is marked `asserted-complete` yet a relevant dependency was excluded.
- **F2 — Silent rebind**: relocation guesses a span and reports `current`.
- **F3 — Silence as assurance**: coverage loss not surfaced; an uncovered region read as
  `current`.
- **F4 — Zombie evidence**: stale evidence counts as current validation.
- **F5 — Chat-only state**: state unrecoverable after restart without chat history.
- **F6 — Erased provenance**: normalization discards provider identity or native detail.
- **F7 — Bookkeeping tax**: a bookkeeping-only verb required on the critical path.
- **F8 — Destructive revert**: rollback that removes/overwrites non-transaction-owned work, or
  reports success on an ambiguous revert.
- **F9 — Inherited verdict**: a freshness verdict served without reconciliation against current
  state.

## 4. Scenarios (executable — Given / When / Then)

**S1 — Observation stale after edit** *(core)*: Given an observation of `foo` recorded at R,
when `foo`'s bytes change and a reconciliation boundary is crossed, then the observation is
`stale` (*supporting input changed*).

**S2 — Scoped invalidation, honestly bounded**: Given a claim whose *declared* scope includes
path B, when an out-of-band edit changes B and a named boundary is crossed, then the claim is
`stale` within scope. **Variant:** if B is *not* in the declared scope, the claim stays
`current within scope` but the coverage report shows B unobserved — proving scope honesty rather
than false assurance.

**S3 — Inference-dependency guard**: Given a claim "empty emails rejected" over `validate_user`
whose conservative scope includes helper `is_blank` in another file, when `is_blank` changes but
`validate_user`'s bytes do not, then the claim is `stale` (or `unknown`) — never `current`.

**S4 — Evidence invalidation gates acceptance**: Given evidence supporting acceptance claim C,
when a relevant input changes, then the evidence is `stale`, C is unvalidated, and the
transaction cannot be accepted on it.

**S5 — Restart recovery**: Given recorded objective, working set, open transaction, claim, and
evidence, when the process restarts with no chat history, then the same report triples are
reconstructed from durable records.

**S6 — Reversible transaction (clean base)**: Given a transaction at clean base R with a
mutation, when reverted, then the repository is restored to R and affected claims/evidence
re-reconcile.

**S7 — Bounded perception, outcome-equivalent**: Given a fixed repository task, when performed
raw vs. workspace-assisted, then *both complete the task successfully*, the assisted path
ingests fewer bytes/tokens under a defined accounting boundary, and full provider detail remains
retrievable on demand. *(Returning an outline that reduces bytes but fails the task does not
pass.)*

**S8 — Provenance retained**: Given a normalized finding, then provider identity and native
payload (or CAS reference) remain retrievable.

**S9 — Dirty-worktree rollback**: Given a transaction begun from a worktree with *unrelated*
pre-existing changes, when reverted, then transaction-owned mutations are removed **and the
unrelated pre-existing changes survive intact**.

**S10 — Concurrent overlapping edit**: Given a transaction and a later overlapping out-of-band
edit that makes ownership ambiguous, when revert is attempted, then it **halts with a conflict**
and destroys nothing.

**S11 — Ambiguous relocation**: Given an edit after which a symbol's span cannot be resolved
unambiguously, then the affected observation is `stale`/ambiguous — never silently rebound.

**S12 — Provider failure vs. empty**: Given a provider that errors, then the record is a failure
distinct from a successful empty result.

**S13 — Secret-bearing output**: Given tool output containing a secret, then it is not persisted
to durable state by default (redaction/retention per imported §*Constraints*).

## 5. Deferred to implementation (settled by the skeleton, not by more design)

- **The conservative dependency-scope default** for undeclared claims (whole-file? one-hop
  call-graph?) — the empirical crux: it trades F1 against alarm fatigue and **must be measured**.
- Fingerprint granularity per evidence type (command / target / file / repository).
- How the coverage boundary is declared and discovered.
- The Pi interface form (extension / MCP server / direct package).
- Cognitive proprioception and motivational braking — a **research question**, out of MVP scope.

## 6. Acceptance

**Tier A — contract-level (binary, must pass before closure):** zero occurrences of F1–F9 over a
**fixed adversarial fixture suite**. Every safety invariant has at least one scenario above.

**Tier B — dogfooding (empirical, earns continuation):** on representative maintenance tasks,
report *with sample size and known instrumentation gaps*: observed false-current rate; bytes/
tokens ingested vs. raw on a fixed outcome-equivalent task; repeated reads; restart-recovery
time; and the conservative-invalidation (alarm-fatigue) rate — which must stay low enough to
remain usable. A first implementation of Git + tree-sitter + retained tool results + careful
joins is acceptable; **algorithmic novelty is not the criterion — measured behavioral change
is.** If these numbers do not move, the workspace has not earned continuation regardless of
sophistication.
