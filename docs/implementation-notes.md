# Implementation Notes

This document records choices settled by walking-skeleton evidence. The
executable contract remains authoritative for behavior; these choices may be
revised when later scenarios expose a better boundary.

## 2026-09-01 — S1 observation staleness

- The kernel begins as a harness-neutral Rust library and standalone executable.
  Pi, Claude Code, other agent harnesses, and Neovim will remain projections or
  adapters rather than dependencies of the kernel.
- Persisted events use versioned JSON Lines. The format is language-neutral even
  though the first producer is Rust. The audit-surface rename from
  `repository_fingerprint` to `reconciliation_fingerprint` advanced writes to
  schema version 2; replay still accepts version 1 through an explicit field
  alias.
- The first implementation is synchronous and replays its append-only log to
  materialize state. A daemon framework, database, watcher, and agent-specific
  protocol are deliberately deferred.
- An observation records a repository-relative path, provider identity, Git
  revision, and SHA-256 input fingerprint.
- Reconciliation fingerprints are scoped to the observed input plus Git
  revision. They do not claim to fingerprint the entire repository; the public
  and persisted field is therefore named `reconciliation_fingerprint`.
- A file observation reports a `declared`, `asserted-complete` scope because the
  observe operation explicitly names the file and captures all bytes in that
  bounded payload. This assertion describes observation capture only. A later
  claim must establish its own dependency scope and must never inherit the
  observation's completeness.
- A missing observed input is stale because its former support disappeared. An
  input that cannot be read for another reason is `unknown`, because
  reconciliation could not determine whether it changed.
- Full event-log replay on each operation and the absence of a writer lock are
  accepted S1 limitations. Replay is currently quadratic over repeated
  operations. Concurrent writers may choose the same sequence and make replay
  fail loudly as corrupt rather than silently diverging; locking and efficient
  materialization belong to later kernel slices.
- S1 is exercised through the standalone executable: record an observation,
  mutate its source out of band, reconcile, and recover a reasoned `stale`
  verdict from the persisted events.

## 2026-09-01 — S2 scoped claim invalidation

- A claim records its statement, supporting observation IDs, and a deduplicated,
  path-ordered set of supporting and explicitly declared dependency inputs.
- Claim scope is `declared` and defaults to `not-asserted`. It is established
  independently and never inherits an observation's `asserted-complete` payload
  capture.
- Recording a claim is itself a reconciliation boundary. Supporting inputs are
  reread before the claim may report `current`; an observation that changed
  before claim creation produces an initially stale claim.
- Operational coverage is reported per result as the paths actually mediated
  for that claim. An edit outside the recorded scope does not alter scoped
  freshness or its reconciliation fingerprint; the omitted path remains visible
  by its absence from `mediated_paths`.
- The standalone CLI adds `claim` and `reconcile-claim` solely to drive the
  executable scenarios. It is not yet the eventual agent-facing protocol.
- Coverage is currently *passive*: an out-of-scope path is visible only by its
  absence from `mediated_paths`. S2 proves honest scoping ("we do not lie about
  what is in scope") but does **not** exercise F3's active half — surfacing that
  an out-of-band change landed on an uncovered-but-possibly-relevant path. That
  awaits the conservative dependency-scope decision deferred in
  `executable-contract.md` §5, and must not be treated as covered yet.

## 2026-09-01 — S3 conservative dependency scope

- The first empirical conservative default is `conservative-siblings`: include
  every regular file with the same extension in each supporting observation's
  immediate directory. This is intentionally simple, provider-independent, and
  likely over-broad; dogfooding must measure its invalidation noise.
- Conservative expansion happens when the claim is recorded. Its concrete input
  paths and fingerprints are persisted, ordered, and inspectable. The report is
  `conservative`, `not-asserted`; it does not imply that dependencies outside
  those sibling directories were discovered.
- Inputs added by the strategy are distinguished as
  `conservative_dependency`. Explicit dependencies remain independently visible
  as `declared_dependency`.
- Claim events written before S3 did not contain `scope_strategy`; replay assigns
  those records the only strategy that existed then, `declared`. The additive
  field remains compatible with schema version 2.
- S3 proves the active part of coverage only within this named conservative
  boundary: changing an unmentioned sibling helper invalidates the claim. New
  files or dependencies outside the recorded expansion remain unresolved future
  cases, not silently claimed coverage.
- `--scope conservative-siblings` is a walking-skeleton switch, not a commitment
  to this strategy as the eventual agent-facing default.
- Conservative scope makes contract invariant I2 load-bearing at the projection
  layer. A conservative claim is honest only because it reports
  `conservative` / `not-asserted`: a cross-directory dependency it missed is a
  disclosed gap, not a false-current. The first projection (Pi, Neovim) that
  renders `freshness_within_scope` without its `scope_assurance` silently
  reintroduces F1 — and no kernel test will catch it, because the kernel is
  behaving correctly. Whoever builds that projection inherits this obligation.

## 2026-09-01 — S4 evidence invalidation and acceptance gating

- Evidence records a transaction and acceptance claim, check name, exact
  invocation, provider identity, outcome, copied claim inputs, and the complete
  scoped freshness report. The current slice imports outcomes; executing checks
  through an authoritative validation provider remains deferred.
- Evidence may be recorded only for a current claim belonging to an open
  transaction. A failed check is retained but cannot satisfy acceptance.
- Transaction acceptance is a reconciliation boundary for every acceptance
  claim and associated evidence item. Acceptance requires each claim to remain
  current and to have current passing evidence; rejection is itself persisted
  with a reason.
- The minimal transaction boundary records the Git revision and a SHA-256
  fingerprint over current tracked and untracked worktree contents. Mutation
  ownership, rollback, and overlapping-write behavior remain S6 work.
- The fixture proves both sides of the gate: unchanged passing evidence accepts,
  while an out-of-band relevant edit makes the claim and evidence stale and
  leaves the transaction open with a persisted rejection.
- **Open seam for S6 — mutation vs. acceptance timing.** Acceptance requires each
  claim `current`, but a change transaction mutates files, and mutating a file
  invalidates any acceptance claim scoped to it. The S4 stale fixture is literally
  this — editing the claim's own input blocks acceptance. As built, only
  transactions whose acceptance claims concern *untouched* files can ever accept; a
  transaction that changes the code it claims about self-invalidates. Conservative
  scope worsens it: a claim scoped to a directory dies on the transaction's own edit
  to any same-extension sibling.
- **Recommended resolution (guidance for S6, not yet binding).** Compute
  acceptance-claim freshness relative to the transaction's *owned post-mutation
  baseline*, not the original observation fingerprints. When the transaction applies
  its owned mutations, re-fingerprint the affected inputs (including any
  conservative-sibling expansion) to establish that baseline and anchor the
  acceptance claims/evidence to it. A claim is then `stale` at acceptance only if an
  input changed *relative to that baseline* — i.e. an out-of-band edit after the
  transaction settled. The transaction's own owned edits are expected and must not
  count as staleness; only drift outside the ownership boundary should. This is
  precisely what S6's mutation-ownership / transaction-owned delta boundary is for:
  it is the mechanism that lets acceptance subtract the transaction's own changes
  from the staleness computation. The alternative — instructing the agent to
  re-observe after each edit — is a workflow band-aid that cannot distinguish the
  transaction's change from a concurrent out-of-band one, which is the exact
  distinction the ownership boundary exists to make.
- **Qualification for S6 — ownership must not silently rebase belief or
  evidence.** The seam above is real, but automatically replacing a descriptive
  claim's or existing evidence item's input fingerprints with the post-mutation
  baseline would make old support appear current for new code. S6 must distinguish
  a durable, normative acceptance criterion (what the result should satisfy) from
  a descriptive claim about observed code (what was found to be true). Owned
  mutations establish the candidate post-state and its drift boundary; they do
  not refresh prior observations or evidence. Validation must run against that
  candidate state and produce new evidence anchored to it. Any later mutation —
  owned or out of band — invalidates that evidence and requires validation again.
  Transaction ownership is then used to distinguish safe rollback and concurrent
  drift, not to exempt pre-mutation evidence from freshness rules.
- **Open modeling question for S6 — descriptive vs. normative.** Today there is one
  `Claim`, and it is purely descriptive (a statement over supporting observations
  and inputs). The qualification above needs the normative acceptance criterion and
  the descriptive claim to be *distinguishable in the model*, not only in prose —
  otherwise one structure will be made to carry both meanings and the distinction
  will erode where it matters most. Whether that is a separate type, a flag, or a
  relation is left to whoever builds S6; the requirement is only that acceptance can
  tell "what the result must satisfy" from "what was observed to be true."
- **S6 modeling direction — use a separate acceptance-criterion type.** A flag on
  `Claim` would leave one structure with incompatible validity rules. A descriptive
  claim is current or stale relative to observations; an acceptance criterion is
  normative and is satisfied or unsatisfied by current evidence against a specific
  transaction candidate state. Transactions should reference criterion IDs, and
  evidence should state which criterion it supports while retaining its own input
  freshness. The current S4 `acceptance_claim_ids` field is therefore provisional
  walking-skeleton scaffolding and must be migrated before S6 acceptance semantics
  are considered complete. No descriptive claim may become current merely because
  a criterion or transaction was rebased.

## 2026-09-01 — S5 restart recovery

- Objective binding and working-set focus are now durable events. The initial
  working set is deliberately small: observation IDs plus explicit reasons for
  focus, ordered deterministically by ID.
- `status` is the resume boundary. It replays the log in a fresh process,
  reconciles every observation, claim, and evidence item against current inputs,
  persists those reconciliation results, and returns one coherent projection of
  objective, working set, observations, claims, evidence, and transactions.
- The acceptance fixture invokes every operation as a separate process, then
  resumes twice and receives equal status projections. This proves recovery from
  durable records rather than in-memory state or chat history.
- Reconciliation currently appends events on every status request, even when
  verdicts do not change. Compaction, no-op event suppression, and materialized
  checkpoints remain kernel work; the append-only log is still authoritative.
- This is restart coherence, not yet a useful orientation experience. Manual
  dogfooding should begin once checkpointing and the S6 transaction boundary can
  preserve a real in-progress change safely.
- **`status` must recompute, never replay (F9 guard).** The write-on-read here is
  not a smell to remove: `status` reconciles before reporting because serving a
  last-known verdict without re-checking current inputs would be exactly F9 (an
  inherited verdict) and could report `current` for a since-changed file. The only
  sanctioned optimization is no-op suppression — always recompute, persist an event
  only when a verdict changes. Do not "optimize" `status` into a pure replay; that
  silently reintroduces F9.
- The no-writer-lock deferral is sharper now that the resume/read path writes.
  `status` is the call a projection or agent loop will hit often and possibly
  concurrently; two concurrent `status` processes collide on sequence and fail
  **loud** as `CorruptLog` (not silent divergence). As built, `status` is strictly
  single-writer until locking lands.

## 2026-09-01 — S6 clean-base transaction rollback

- S6 deliberately supports one narrow ownership boundary: a tracked file must
  still byte-match the transaction's Git base before the transaction may mutate
  it. Dirty initial files, new files, deletion, and multiple writes to one path
  remain later scenarios.
- A mutation event retains repository-relative path plus before/after SHA-256
  fingerprints, not source bytes. Clean-base rollback reconstructs the original
  payload through `git show <base>:<path>`, verifies both fingerprints, and writes
  through a same-directory temporary file plus atomic rename.
- Revert refuses to proceed when current bytes differ from the transaction's
  recorded after-fingerprint. S10 will broaden and harden this conflict behavior;
  S6 proves the clean, unambiguous path.
- Applying and reverting both cross reconciliation boundaries, so descriptive
  claims and evidence become stale after the owned mutation and current again
  only when clean rollback restores their exact recorded inputs.
- If appending `MutationApplied` fails after the atomic file write, the operation
  attempts immediate compensation. A process crash between the filesystem write
  and durable event append remains an unclosed recovery window; prepared/applied
  mutation events or equivalent journaling are required before this can be
  called crash-atomic.
- This rollback slice does not resolve the descriptive-claim versus normative-
  criterion acceptance seam. Post-mutation acceptance remains provisional and
  must not be presented as complete until `AcceptanceCriterion` replaces S4's
  `acceptance_claim_ids` scaffolding.

## 2026-09-01 — CLI ergonomics, surfaced by first dogfooding pass

A hands-on shakeout of the full CLI loop (`bind-objective → observe → claim →
status`, plus an out-of-band edit to confirm a claim goes `stale` and back to
`current`) confirmed the mechanism end to end. It also exposed two rough edges
that belong in the tool, not worked around in the accompanying skill:

- **`bind-objective` returns no id or handle** — it echoes the intent back only,
  so a caller cannot confirm state was persisted or later reference the objective
  by id. Every other create verb returns an `id`; this one should too.
- **IDs are zero-indexed and unforgiving.** `observe` returns `id: 0`, but the
  natural 1-based instinct (`--observation 1`) fails hard with `not found`. The
  ergonomic fix is open (echo ids more prominently, or accept and report a
  clearer error). Until then the skill documents the hazard, but documenting a
  papercut is not the same as removing it.

These are ergonomics only — the freshness and rollback cores behaved exactly as
specified. Recorded here so the dogfooding-versus-fix boundary stays honest: the
skill captures durable *judgment* (defer to a `stale` verdict, declare scope
honestly), not the tool's fixable quirks.
