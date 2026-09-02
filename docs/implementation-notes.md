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
- **Dogfood note (delta window semantics).** `delta_since` diffs the active
  claim sets, so a claim recorded *and* superseded entirely within one window
  (a transient belief) appears in neither `claims_recorded` nor
  `claims_superseded` — the durable log retains it, but the resume view does
  not surface it. Bounded for now: resumption cares about live beliefs; a
  cold reader who wants the full story can still replay the log.
- **Still open after this slice.** Materialization efficiency (replay cost is
  still linear in log length; suppression slows growth but does not shrink
  replay) and writer locking (concurrent statuses still collide as `CorruptLog`)
  remain the kernel action's open items.

## 2026-09-01 — Writer locking

- **The race, precisely.** Every mutating command is a read-modify-write with
  nothing held across it: it calls `project()` to compute a new entity id (e.g.
  `next_observation_id`) and then `append()`, which independently re-reads
  `next_sequence` before writing. Two concurrent CLI processes could therefore
  write the *same* sequence (→ `CorruptLog` on replay, at the projection's
  sequence check) or the *same* entity id (→ silent overwrite, no error at all).
  This closes the writer-locking item left open by the no-op-suppression slice.
- **Lock at the outermost boundary, not per method.** `resume_status` is itself
  a writer — it reconciles, appending `*Reconciled` events — and calls other
  mutating methods (`reconcile_*`), as do `supersede_claim` and
  `accept_transaction`. Locking inside each method would self-deadlock (advisory
  `flock` on a second fd in the same process blocks). So the lock is acquired
  exactly once, in `main.rs::run()` right after `Workspace::open`, and held for
  the whole command. `append()` stays lock-free.
- **`std` file locking, no new dependency.** `Workspace::lock_exclusive` opens
  `.agent-workspace/events.lock` (gitignored) and takes a blocking exclusive
  advisory lock via `File::lock` (stable since Rust 1.89). The returned
  `WorkspaceLock` guard releases on drop or process exit — a crashed holder
  frees the lock, so there is no stale-lock hazard. Blocking means concurrent
  invocations *serialize*; swapping to `File::try_lock` would instead *fail
  loud* with a lock error, a one-line change if that policy is ever preferred.
- **Test with teeth.** `concurrent_writers_serialize_without_corrupting_the_log`
  fires 16 `observe` processes at one workspace simultaneously, then asserts the
  log's sequences are exactly `0..=N` (contiguous, unique — catching a sequence
  collision) and every observation id is distinct (catching a silent overwrite),
  and that `status` replays without `CorruptLog`. Verified to fail without the
  lock (6/6 runs, including `expected sequence 4, found 3`).
- **Residuals.** The lock lives at the CLI boundary, so a library embedder that
  drives `Workspace` methods directly must hold `lock_exclusive` around its own
  mutation sequence. Pure-read commands (`delta`, `reveal`) also take the
  exclusive lock, so all invocations fully serialize — simpler than a shared/
  exclusive split and it also rules out torn reads, at the cost of some read
  concurrency that does not matter for a local CLI.

## 2026-09-01 — Normalized fingerprinting (semantic freshness, first slice)

- **The problem it kills.** The freshness signal fingerprints raw bytes, so a
  pure reformat (whitespace/layout, no meaning change) reads as `stale`. This
  fired three times in one session — a GLM reformat of `walking_skeleton.rs`
  staled claim 17 for zero semantic reason. A signal that cries wolf on cosmetic
  noise erodes the prime directive (*believe `stale` over your own memory*), so
  this is signal-trust work, not an optimization.
- **The reframe that kept it small.** Not an AST/semantic-diff engine — fingerprint
  the *formatter-canonical* form and delegate canonicalization to the language's
  existing formatter (`rustfmt`). "Leverage existing tools." The unit fingerprint
  is computed in exactly two places (`capture_file_observation` and
  `read_observation_fingerprints`, the latter serving both observation-reconcile
  and `assess_claim_inputs`), so routing both through `normalize_unit` is the
  whole mechanism.
- **The normalizer rides beside the selector.** A `Normalizer` enum (`None`
  default, `Rustfmt`) sits next to `ObservationSelector` on `Observation`, the
  `ObservationRecorded` event, and `ClaimInput`. Because the selector already
  travels wherever an input is fingerprinted, record-time and reconcile-time
  normalization stay identical *for free* — the key to this being a small slice
  rather than a sprawling one. All fields are `#[serde(default)]`, so existing
  logs replay unchanged.
- **Opt-in, default byte-identical.** `--normalize rustfmt` on `observe`; default
  `None` changes nothing. `rustfmt_canonical` shells `rustfmt --emit stdout` over
  stdin and **falls back to raw bytes** when rustfmt is absent or the unit does
  not parse (a mid-edit file, or a byte-range fragment that is not a standalone
  item) — so it degrades to today's behavior, never to an error.
- **Proof.** `rustfmt_normalized_observation_ignores_reformat_but_catches_semantics`:
  a normalized observation stays `current` across a reformat while the byte
  observation stales, and a semantic edit (`x = 1` → `x = 2`) stales even the
  normalized one. End-to-end smoke reproduced the exact GLM incident (multi-line
  `assert!` → chained one-liner): normalized `current`, byte `stale`.
- **Deferred (honest residuals).** (1) *Opt-in has an adoption gap* — nobody
  benefits unless they pass the flag, so the claim-17 recurrence is only
  prevented once observations adopt it; auto-normalizing recognized source types
  is the natural follow-up but was rejected here to avoid silently changing
  existing fingerprints' meaning and taxing every reconcile with a subprocess.
  (2) *Version skew* — the canonical form depends on the rustfmt version, so two
  environments with different rustfmt still disagree; this is Pareto-no-worse
  than today's byte baseline, and pinning rustfmt (`rust-toolchain.toml`) would
  close it. (3) The normalizer is not folded into the reconciliation-fingerprint
  material (the unit fingerprint already encodes the normalized content).

## 2026-09-01 — Auto-normalize default (semantic freshness, second slice)

Inverts the normalizer default per the run-6 forward guidance: the adoption gap
is now closed — a plain `observe` of recognized source gets canonical
fingerprinting with no flag to remember.

- **`auto` is the default; records persist the *resolved* scheme.**
  `--normalize` accepts `auto` (default), `none`, `rustfmt`. `auto` resolves
  at capture time via `Normalizer::detect_for_path` (extension-based: `.rs` →
  `Rustfmt`, else `None`) and the concrete normalizer is what lands on the
  record — reconcile never re-resolves, so a future extension to the detection
  table cannot change an existing record's meaning. `none` is the explicit
  raw-byte escape hatch.
- **Dependencies auto-detect kernel-side.** `record_claim_with_scope` routes
  declared and conservative-sibling dependencies through
  `fingerprint_dependency` (detect + fingerprint) instead of hardcoded
  byte-mode. The escape hatch for a byte-exact dependency: capture it as a
  supporting observation with `--normalize none` and cite the observation.
- **Raw-byte fast path.** `Observation` and `ClaimInput` carry an optional
  raw-unit fingerprint (recorded only when the normalizer makes it distinct
  from the input fingerprint). `read_observation_fingerprints` hashes the raw
  unit first and skips the formatter subprocess entirely when bytes match —
  a deterministic normalizer maps identical bytes to an identical canonical
  form, so the unchanged case (the overwhelmingly common one in `status`)
  pays one read + one SHA-256, no subprocess. Records without the field
  (all pre-slice records, all `None` records) simply never fast-path.
- **No fingerprint-scheme version bump — deliberately.** Run-6 suggested a
  bump for the one-time migration; it is unnecessary because records are
  self-describing (each carries its own normalizer and optional raw
  fingerprint) and the default change affects only *new* records. Old logs
  replay byte-identically through `#[serde(default)]`, and old claims keep
  their byte semantics forever. A global bump would have invalidated honest
  byte-mode history for no gain.
- **Proof.** Three new tests: `auto_normalizer_detects_rustfmt_for_rust_and
  _none_otherwise` (resolution + persisted scheme + raw-fingerprint presence),
  `reconcile_fast_path_skips_formatter_when_bytes_unchanged` (reconcile under
  a PATH without rustfmt stays `current` on a non-canonical file — without the
  fast path the fallback-to-raw would false-stale it), and
  `claim_dependency_auto_detects_normalizer` (reformatted dependency stays
  `current`, semantic edit stales). The pre-existing rustfmt test's
  byte-contrast arm now passes `--normalize none` explicitly — the one honest
  test edit the inversion required. 29 tests green, fmt/clippy clean.
- **Residuals.** (1) Detection is extension-only — a `.rs` extension on
  non-Rust content gets rustfmt attempted and falls back to raw bytes, honest
  degradation. (2) Byte-range units on recognized types pay the subprocess on
  every *changed-container* reconcile even when the range itself is unchanged,
  because the raw fingerprint covers the unit only — acceptable; range
  observations are rare. (3) `assess_claim_inputs` still re-reads every input
  on every status (materialization efficiency remains the open kernel item;
  the fast path removes the subprocess tax, not the I/O tax).

## 2026-09-01 — Brief-default status

- **Orientation is now the default projection.** `status` returns a bounded
  `BriefStatus`: objective, every active claim's id/freshness/scope and
  one-line headline, aggregate lifecycle and freshness counts, and the latest
  checkpoint. `status --full` retains the complete audit projection.
- **Scope assurance remains inseparable from freshness.** The concise claim row
  still carries both scope source and completeness, preserving the projection
  obligation established by S3 rather than presenting `current` as complete.
- **Statements are bounded at a UTF-8-safe character boundary.** Headlines are
  single-line and truncated without splitting multi-byte characters; the full
  statement remains available in the audit view.
- **Measured on the dogfood workspace:** the default response fell from roughly
  176 KB to 5.6 KB while retaining the objective, every live belief's freshness
  and scope, and counts. This closes the context-cost half of the status-cost
  objective without weakening reconciliation.

## 2026-09-01 — Single-pass status materialization

- **Project once, reconcile many.** `resume_status` now projects the append-only
  log once, computes each observation, claim, and evidence verdict against that
  snapshot, appends only changed reconciliation events, and updates the in-memory
  projection as those events are accepted. It no longer calls a public
  reconcile method that replays the full log for each entity.
- **F9 remains inviolable.** The optimization caches inputs/projection only for
  one status operation; it never caches freshness verdicts across calls. Every
  invocation rereads current mediated inputs and recomputes verdicts, so an
  out-of-band edit between consecutive statuses is still detected.
- **Event invariants remain centralized.** Newly appended reconcile events flow
  through the same projection transition logic used by replay rather than
  mutating materialized entities through a second ad-hoc path.
- **Measured proof:** the live status path fell from 214 event-log reads to one.
  Acceptance coverage asserts one projection pass, unchanged repeated verdicts,
  no-op event suppression, and detection of an intervening out-of-band edit.
  This closes the final walking-skeleton kernel item; persistent snapshots or
  compaction remain unnecessary until measured log replay cost justifies them.

## 2026-09-01 — Pi read auto-capture (first interface slice)

- **The adapter observes; it does not replace authority.** The project-local Pi
  extension leaves the built-in `read` tool untouched. It remembers a read's
  arguments at `tool_call`, then captures its finalized `toolResult` from Pi's
  `context` event, after `tool_result` and `message_end` middleware. The measured
  text is therefore the model-boundary projection this extension receives, not
  an intermediate native result or a reimplemented reader. A later-loaded
  `context` handler can still alter that projection; Pi exposes no after-context
  canonical-message hook, so extension ordering remains an explicit limit.
- **Line selections become durable UTF-8 byte selectors.** The adapter maps
  Pi's one-indexed `offset`/`limit` selection onto the kernel's half-open byte
  range, verifies the finalized source prefix against the current file, and
  accepts Pi's exact pagination notice separately. Unicode fixtures prove the
  line-to-byte conversion. Whole, unpaginated reads retain whole-file scope.
  Files are decoded with fatal UTF-8 handling; invalid bytes record nothing
  rather than producing lossy offsets into unrelated raw bytes.
- **The accounting boundary is now explicit and persisted.** Observation events
  carry additive, backward-compatible `model_visible_bytes: Option<usize>`.
  `ingested_bytes` remains source-unit bytes; `model_visible_bytes` includes
  model-visible pagination text. Uninstrumented CLI captures replay as `None`.
- **Privacy and failure policy are fail-closed.** The adapter records provider
  `pi.read`, path, selector, fingerprints, and byte counts, but never retains the
  native payload by default. Failed, image, native-truncated, selection-drifted,
  invalid-UTF-8, out-of-repository, `.agent-workspace`, and conventionally
  sensitive-path reads record nothing. Paths are canonicalized before both
  containment and sensitive-target checks, so an innocuous symlink cannot bypass
  either boundary. Capture failures are swallowed so workspace bookkeeping can
  never turn a successful native read into a failed read.
- **Concurrency and recursion reuse existing boundaries.** Extension-side
  `pi.exec` is not a Pi tool call, so it cannot trigger its own hook; parallel
  captures serialize through the kernel's existing workspace lock.
- **Selected-byte races fail closed.** The adapter hashes the selected bytes it
  matched to the context result and passes `--expected-raw-fingerprint`; the
  kernel verifies that digest against its own capture read before appending.
  A selected-unit edit in the extension→kernel window therefore records nothing.
  A concurrent edit *outside* a bounded selection can still change the recorded
  container fingerprint without changing the selected unit. Closing that
  container-provenance window requires a native read result/container fingerprint
  or a provider-snapshot import API; the adapter does not pretend otherwise.
- **Proof.** Five TypeScript tests cover UTF-8 range mapping, finalized-visible
  pagination accounting, containment/sensitive-path rejection, fail-closed
  drift/truncation/invalid UTF-8, and hook-to-kernel arguments including the
  expected selected-byte fingerprint. The extension type-checks and
  loads through Pi. The Rust walking skeleton persists and restart-projects the
  model-visible count; all 33 Rust tests pass. A real print-mode Pi dogfood
  read of `README.md` (`offset=2`, `limit=3`) produced observation 77 with
  provider `pi.read`, byte range `18:216`, 198 source bytes, 250 finalized
  model-visible bytes (including pagination), current freshness, and no retained
  payload.

## 2026-09-01 — Pi orientation tools (second interface slice)

The first Pi slice made ordinary *reads* populate the workspace; this slice made
*orientation* itself available without leaving the agent surface. The extension
now registers two custom tools and the dogfood question — "can a fresh agent
orient without shelling to the CLI?" — was answered by the tool used to pose it.

Design decisions:

- **Projection, not reimplementation.** `workspace_status` and `workspace_delta`
  exec the kernel binary and return its JSON verbatim (`resume_status().brief()`
  and `delta_since()`). Every semantic decision stays in the kernel: freshness,
  scope assurance, checkpoint selection. The tools add argument mapping only
  (`full`, `since`) and must drift in lockstep with the CLI rather than
  inventing a second status vocabulary.
- **Throwing is the error channel.** Pi custom tools return `AgentToolResult`,
  which has no `isError` field — a failed kernel invocation throws and surfaces
  as a tool error. Expected environment conditions are not errors: a directory
  outside a Git checkout returns plain text telling the agent no runtime exists,
  so it can adapt instead of treating orientation as broken.
- **The prime directive is encoded in tool metadata, not just the skill.**
  `promptGuidelines` on `workspace_status` states that a claim reported stale
  outranks the agent's remembered belief. A fresh agent that never loads the
  skill still receives the rule in its system prompt whenever the tool is
  active — the adoption problem from the first reflection, attacked at the
  prompt layer.
- **Bidirectional dogfood arrived on its own.** During this session's cold
  start, the auto-capture slice recorded the agent's reads of `SKILL.md`,
  `index.ts`, and `main.rs` (observations 87–89) before any manual `observe`.
  The workspace now watches its own development sessions.

Proof. Eight TypeScript tests (three new): status projection arguments brief by
default and `--full` on request; delta passes `--since` through and omits it by
default; graceful no-runtime text plus throwing on kernel failure. Strict
typecheck, 33 Rust tests, fmt, clippy pass. A fresh `pi -p` session oriented via
the tools alone — objective, one current claim, latest checkpoint — with zero
CLI shelling. Commit `e6c8ef0`; claim 38 records the slice, claim 39 supersedes
the run-8 handoff umbrella (claim 37) now that its next objective was consumed.

## 2026-09-02 — Claude Code adapter: live orientation dogfood

The Claude Code adapter now closes the same two-organ loop as Pi: a
`PostToolUse(Read)` hook records ambient reads through the kernel-owned
`observe-read` planner, and a `SessionStart` hook pushes durable orientation
back into a cold model context. Both hooks are transports over kernel semantics;
they do not replace Claude Code's native tools.

The first real cold-session drive found a boundary the synthetic hook drive did
not: Claude bounds the inline preview of command-hook stdout. The hook emitted
17,282 bytes (2,659-byte brief status plus a 14,350-byte checkpoint delta), and
the model-visible preview ended partway through claim 44. Claim 45 and the latest
checkpoint were absent. Exiting 0 and containing both sections in raw stdout was
therefore necessary but not sufficient acceptance evidence.

The repair keeps the complete kernel status and delta verbatim, but precedes
them with a compact preview index containing the exact kernel objective and
latest checkpoint plus every active claim's id, freshness, scope, and a
48-character transport-truncated version of its already-bounded kernel
headline. This is adapter framing, not a second freshness model: no verdict is
computed or changed. Claude can reveal the full saved hook output when it needs
the audit projection.

Executable acceptance now checks 16 conditions: exact objective and checkpoint,
exact claim id/freshness/scope rows, an essential preamble below 1,800 bytes,
verbatim status and delta sections, and harmless silence for no-Git,
empty-workspace, and malformed-input cases. A second genuinely fresh
`claude -p` session, forbidden from calling tools or reconstructing from files,
reported the exact objective, checkpoint `run-11-orientation-on-wake-shipped`,
and all six active claims grouped correctly (current 40/44; stale 38/41/42/45),
with no requested field missing from inline context.

Residual: the preview index is bounded per headline but not yet by active-claim
cardinality. If the standing claim set grows enough to cross Claude's preview
budget, the kernel should gain an explicit bounded wake projection rather than
letting the adapter invent claim-prioritization semantics.

## 2026-09-02 — Adapter consolidation and kernel-bounded orientation

The live-preview repair above exposed the remaining architectural smell: Claude
owned an adapter-local index only because the kernel's supposedly "brief"
projections were not cardinality-bounded. That is now corrected at the authority
boundary instead of normalized as adapter behavior.

- **Status is bounded and honest.** Default status ranks stale, then unknown,
  then current claims; emits at most eight 80-character headlines; and reports
  `claims_omitted` plus aggregate counts over the complete active set. Compact
  transport on the dogfood workspace is about 1.4 KB and executable coverage
  holds a twelve-claim fixture below the 1,800-byte model-preview boundary.
- **Delta is a bounded reveal index.** Default delta carries the checkpoint,
  bounded before/after objective headlines, and total/recent-id/omitted groups
  for recorded, superseded, or staled claims plus observations and opened or
  closed transactions. Each id group retains the sixteen most recent ids.
  `delta --full` preserves complete entities; `--compact` only changes JSON
  transport. The live bounded delta is about 0.8 KB instead of growing with
  observation payloads.
- **Claude is thin again.** `orient-session.py` deleted its preview-index
  semantics and forwards compact kernel status then compact kernel delta
  verbatim. Its acceptance drive compares both sections byte-for-byte with
  direct kernel output, holds essential status below 1,800 bytes and combined
  wake output below 3,000, and retains all harmless/silent failure cases.
- **Pi no longer plans captures.** The duplicate `capture.ts` and its parallel
  tests are deleted. The extension strips only Pi-owned pagination chrome,
  preserves the original model-visible byte count, and streams selected text on
  stdin to kernel `observe-read`; no read payload enters argv or a temporary
  file. Because Pi's extension `exec` API has no stdin channel, this one path
  uses a directly spawned kernel process with timeout and abort propagation.
- **Runtime absence is truthful.** Pi checks that the built kernel exists before
  advertising an active runtime, does not cache a missing-binary result (a later
  build activates without restart), and tests both no-Git and Git-without-binary
  cases.
- **Strict validation is restored.** `ReadCaptureOutcome::Captured` boxes its
  large payload, clearing Rust 1.97's `large_enum_variant`; 38 Rust tests, strict
  clippy, four Pi integration/tool tests plus typecheck, and sixteen Claude
  adapter-drive checks pass.

The kernel request now distinguishes stripped matching text from total
model-visible bytes. This preserves the accounting boundary established in run
8 without teaching the kernel any harness presentation format. A fresh `pi -p`
model-boundary redrive then read `README.md` through the native tool and, without
Bash or a manual kernel call, produced observation 154 (`pi.read`, whole-file,
5,004 source/model-visible bytes); bounded delta advanced from 17 to 18 recorded
observations and retained only its sixteen most recent ids.

## 2026-09-02 — Semantic working set (first attention-model slice)

Run 13 ended with 166+ observations but only provenance-oriented access: the
system sensed and remembered well and did almost nothing to *direct attention*.
This slice turns the focused-observation stream into a bounded, ranked,
restart-safe attention model, projected through a new `working-set` command.

- **A semantic location is a projection, not a stored entity.** The observation
  a `WorkingSetEntry` cites already persists path, selector, revision, and
  container fingerprint. Materializing `SemanticLocation` by joining entry →
  observation at status time keeps one source of truth that cannot drift, mirrors
  exactly how `BriefClaim` is derived, and honors the charter's "small durable
  vocabulary" constraint. A projection is trivially promotable to an entity later
  if that proves too thin; the reverse would be a migration. `relocation_fingerprint`
  is the observation's container fingerprint — a file-level relocation anchor, not
  a symbol identity (perfect cross-refactor identity is a declared non-goal).
- **No event-schema change; the trail is derived.** `ObservationFocused` still
  carries only `{observation_id, reason}`. A monotonic `focus_sequence` is
  stamped at projection time from event order, so the append-only log is
  untouched and every existing log (172 live observations, 16 pre-existing focus
  events) replays identically — verified by running `working-set` against the
  live workspace, which recovered a 16-entry trail with no migration.
- **Working set vs navigation trail.** The deduped `working_set` map answers
  "what am I attending to" (latest focus per observation wins on a revisit); the
  ordered `navigation_trail` Vec answers "in what order did I get here" (revisits
  included). The map's key ordering cannot express the latter, so both are kept,
  both replayed from the same stream.
- **Every section is hard-bounded with an explicit omission count.** Locations
  (cap 12, ranked stale-first then most-recently-focused, so a cap can never
  preferentially hide invalidated attention), uncited candidates (cap 12), and
  trail (cap 16) each pair their bound with a `_omitted` count — the same
  visible-truncation contract `BriefStatus` keeps for claims. No whole-file
  payload is retained: locations point at observations, where reveal already lives.
- **Uncited = current, uncited, unfocused.** The attention-candidate surface is
  the current observations no active claim supports and that are not already
  focused — the raw material a `focus` turns into a location.
- **Coverage.** Two acceptance tests: one drives the full contract (join
  coordinates, latest-reason-wins, recency ranking, uncited exclusion, ordered
  trail, out-of-band edit → stale-first, cold-restart trail recovery); the other
  proves all three omission counters at once with a 13/13/17 over-cap fixture.
  40 Rust tests, strict fmt and clippy pass.
- **Pi projection (criterion 7).** A `workspace_working_set` Pi tool mirrors the
  existing `workspace_status`/`workspace_delta` orientation tools: it shells the
  bounded `working-set --compact` command through the shared `runKernel` helper
  and forwards the kernel JSON verbatim, adding no semantics. Parameterless — the
  view is inherently bounded, so there is no brief/full split to expose. 5 Pi
  tests plus typecheck pass. This closes all seven acceptance criteria; the
  `working-set` action is complete.
