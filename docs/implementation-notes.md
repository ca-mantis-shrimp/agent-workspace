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
