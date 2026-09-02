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
- Reconciliation appends a reconcile event only when the recomputed verdict
  differs from the last persisted one (no-op suppression, 2026-09-01); verdicts
  are always recomputed on every status request. Compaction and materialized
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
- First review exposed a reachable multi-path consistency bug: revert originally
  validated and restored in one reverse loop, so a conflict on a later path
  could leave earlier paths restored and permanently wedge the open transaction.
  Revert now has a read-only validation phase for every owned path before any
  write. Write or event-append failures trigger best-effort compensation back to
  the owned after-state. The conflict fixture proves an overlapping edit leaves
  every path untouched and the transaction open.

## 2026-09-01 — S7 bounded perception

- A bounded filesystem observation uses a zero-based, half-open UTF-8 byte range.
  This is deliberately a provider-neutral experiment, not yet a semantic symbol
  selector or relocation mechanism.
- Every new observation records two SHA-256 fingerprints: the selected unit and
  its whole-file container. Unit bytes drive `freshness_within_scope`; a change
  elsewhere in the container leaves the bounded observation `current` but changes
  its reconciliation fingerprint, reports the outside-unit drift in the reason,
  and keeps scope completeness `not-asserted`. A unit change is `stale`.
- Supporting claims copy the observation selector with the unit fingerprint, so
  a bounded claim reconciles the same mediated unit rather than accidentally
  comparing its range hash with a whole-file hash. Operational coverage now names
  `mediated_units` as path-plus-selector as well as compatibility-oriented paths,
  and claim/evidence reconciliation fingerprints hash selectors. Equal bytes at
  different ranges are therefore not represented as the same scope. Declared and
  conservative dependencies remain whole-file inputs.
- `observe` returns the selected UTF-8 content and an `ingested_bytes` count. The
  accounting boundary is source-content bytes returned to the caller; JSON
  metadata, event-log bytes, filesystem reads, and retained payload storage are
  excluded. `reveal --observation <id>` returns the exact observed container and
  its own byte count.
- Full provider detail is retained only with explicit `--retain-payload true`,
  capped at one MiB, in a content-addressed `payloads/<sha256>` store outside the
  JSONL log. Default observation does not retain the container. The event carries
  any relative payload reference plus provider, revision, and container hash.
  Repository reads require canonical containment, and payload directories/files
  are rejected when symlinked. Legacy observations replay with whole-file
  selectors but have no retained payload, so reveal fails explicitly rather than
  returning current bytes as if they were historical provider output.
- The outcome-equivalent fixture compiles and runs the same generated Rust task
  in raw and assisted arms. Each edit is constructed from the source content its
  arm actually received. The raw arm receives the entire source; the assisted arm
  receives only `foo`'s callable signature, ingests fewer bytes, and still
  produces the same passing executable. The optional reveal probe occurs after
  the measured task boundary and reproduces the pre-edit full source; counting a
  reveal as part of the assisted task would correctly erase its byte advantage.
  An outside-range task edit preserves scoped freshness while changing the
  container reconciliation, and a later signature edit stales a supporting
  bounded claim.
- This proves the byte-accounted range experiment, not useful semantic
  navigation. Range shifts, ambiguous relocation, token accounting, tree-sitter
  providers, and empirical agent task trials remain later work.
- Explicitly retaining whole source containers still creates S13/privacy debt:
  size bounding and opt-in prevent default/unbounded retention, but no secret
  redaction exists. Retention must not be enabled for secret-bearing provider
  output until that scenario is implemented.

## 2026-09-01 — Claim supersession for handoff correctness

- Claim lifecycle is now orthogonal to freshness. An active claim may be
  `current`, `stale`, or `unknown`; a superseded claim retains its last freshness
  report but is explicitly retired with a required replacement claim ID and a
  non-empty human-readable reason. Input drift therefore no longer doubles as a
  retirement signal.
- `supersede-claim --id <old> --claim <replacement> --reason <why>` requires both
  claims to exist and be active, rejects self-replacement and empty reasons, and
  refuses to retire an acceptance claim belonging to an open transaction.
  Replacement chains are permitted, but cycles are prevented because an already
  superseded claim cannot become a replacement.
- Resume reconciliation touches both active and superseded claims because
  freshness remains independent of lifecycle: history must not preserve an
  inherited `current` verdict after its inputs drift. `WorkspaceStatus.claims` is
  the live belief set; `superseded_claims` is explicit, freshly reconciled history.
  The observation-based `working_set` is unchanged, so “leaves the live working
  set” means leaving the active claim projection rather than deleting support.
- Superseded claims may be reconciled, but cannot begin a transaction or receive
  new evidence. Existing evidence remains historical; supersession does not
  rewrite prior provenance. Supersession itself reconciles the retiring claim
  before persisting lifecycle state, so its immediate response cannot expose an
  inherited verdict.
- `ClaimSuperseded` is an additive schema-v2 event. Claims replayed from older
  logs default to active. Replay revalidates lifecycle invariants rather than
  trusting command-layer checks: reasons are non-empty, open transaction claims
  cannot retire, transactions require active claims, and evidence must match a
  current active acceptance claim. The fixture keeps two drifted beliefs without
  pre-reconciling them: supersession discovers one claim's drift, while cold
  status discovers the other's. They appear in different sections with the
  replacement link and reason; safety coverage also exercises chains, attempted
  cycles, transaction guards, and invalid event ordering.
- This slice curates claims only. Objective completion/replacement, checkpoint
  deltas, observation focus retirement, and semantic classification of formatting-
  only drift remain separate work.

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

## 2026-09-01 — Checkpoint and delta-oriented resume view

Closes two of the lifecycle gaps the supersession entry deferred: objective
completion (recorded, not overwritten) and the "changes since checkpoint" view
both cold-resume runs asked for. It is the slice the run-2 reviewer sequenced
after supersession, on the grounds that there is no meaningful delta without a
checkpoint notion to diff against.

- **A checkpoint is a remembered sequence boundary, not new entity state.**
  `Checkpointed { label, note, git_revision }` is an additive schema-v2 event;
  replay records the marker together with the sequence it landed at and a
  snapshot of the objective then in force. Because the marker carries the
  objective, replacing an objective after a checkpoint no longer erases the fact
  that the previous one was completed — the checkpoint preserves it.
- **The delta is projection-twice-and-diff, introducing no new freshness axis.**
  `delta_since` projects the log up to the checkpoint's sequence (a pure read via
  `project_upto`, which never appends reconciliation events) and projects the log
  to now (`resume_status`, which does reconcile), then diffs the two states.
  "Staled since" therefore means a claim whose *recorded* freshness at the
  checkpoint was `current` and is `stale` now — derived, not a stored flag.
- **This design was forced by a pre-existing seam.** `resume_status` reconciles
  every claim/observation/evidence and `reconcile_claim` appends unconditionally,
  so the log grows on every `status` and a stale verdict is re-emitted each call.
  A naive "stale-reconcile event after sequence S" delta would therefore be
  noise. Diffing two projected states sidesteps it. No-op suppression remains the
  right separate fix; the live workspace log is already ~750 events, most of them
  redundant reconciliations.
- **`delta` defaults to the latest checkpoint, with `--since <label>` to
  override.** This is the cold-resume ergonomic ("what changed since I last drew a
  line"); the explicit override keeps it unmagical when a specific baseline is
  wanted. Labels are unique so a `--since` reference is unambiguous.
- **No §4 contract scenario was added.** The contract's scenarios are the F1–F9
  adversarial safety fixtures; checkpoint/delta is a lifecycle/orientation feature
  already covered by vocabulary (§0) and invariant 13 ("stopping is success"). It
  is proven by walking-skeleton tests instead — the same precedent supersession
  set. The full `status` output is unchanged; the delta is a strictly additive
  surface (`checkpoints` is now listed in `status` so labels are discoverable).
- **Still open after this slice.** A checkpoint records the objective but there is
  no explicit "objective completed" disposition distinct from "replaced"; the
  delta reports objective *change* structurally, not intent. Focus/working-set
  retirement and semantic (formatting-only) drift classification remain separate;
  no-op event suppression landed later the same day (see below).

## 2026-09-01 — No-op reconcile event suppression

- **`status` no longer re-emits unchanged verdicts.** Each of the three
  reconcile seams (`reconcile_observation`, `reconcile_claim`,
  `reconcile_evidence`) computes its `(freshness, reason,
  reconciliation_fingerprint)` verdict exactly as before, then compares it to
  the verdict already persisted in the item's report. An event is appended only
  when any of the three differ; otherwise the stored item is returned as-is.
  All other report fields (scope assurance, mediated paths/units) are set only
  by record events and never touched by `*Reconciled` events, so an unchanged
  verdict proves the stored item equals what re-projection would return —
  the no-op path skips a redundant re-projection as well.
- **The F9 guard is preserved by construction.** Suppression conditions only the
  *persistence* of a verdict, never its computation: every status still reads
  current inputs and recomputes every freshness. The test
  `suppressed_status_still_recomputes_and_emits_changed_verdicts` proves an
  out-of-band edit after a fully suppressed status is still detected, persisted,
  and reported stale.
- **One-time normalization per observation.** `ObservationRecorded` seeds the
  report reason "supporting input recorded"; the first reconcile after recording
  changes it to "supporting input unchanged" (or the drift verdict) and appends
  once. Claims and evidence are recorded through the same `assess_claim_inputs`
  reconciliation uses, so their first reconcile can already be a no-op. This
  keeps "recorded, never reconciled" distinguishable from "reconciled" in the
  durable record at the cost of exactly one event per observation, once.
- **Measured on the live dogfood workspace:** after landing, one status over
  genuinely changed inputs appended the 24 verdict changes my edits caused; the
  next status appended zero (previously every status re-emitted every verdict —
  the log had reached ~970 events, most of them redundant reconciliations).
- **No contract scenario was added.** Suppression is an efficiency property
  that leaves every F1–F9 scenario's observable semantics unchanged; it is
  proven by walking-skeleton tests
  (`status_suppresses_redundant_reconcile_events`,
  `suppressed_status_still_recomputes_and_emits_changed_verdicts`), the same
  precedent as checkpoint/delta.
- **Still open after this slice.** Materialization efficiency (replay cost is
  still linear in log length; suppression slows growth but does not shrink
  replay) and writer locking (concurrent statuses still collide as `CorruptLog`)
  remain the kernel action's open items.
