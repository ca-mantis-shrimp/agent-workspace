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

## 2026-09-02 — Findings, sub-slice A (record + provenance + freshness)

First slice of the findings/validation-evidence action: a persistent,
quickfix-like queue of provider-reported issues. Evidence already existed; this
adds the Finding concept.

- **Disposition is orthogonal to freshness — a correction to the design doc.**
  `initial-design.md` folds `stale` into the finding lifecycle enum alongside
  `open/resolved/suppressed`. That is the exact conflation claim supersession
  already un-made: input-drift (`stale`, from reconcile) is not a decision-state.
  So `Finding` carries both a `FreshnessReport` (current/stale/unknown) and a
  separate `FindingDisposition` (open + resolved/deferred/suppressed/false-positive,
  each with actor + rationale). Sub-slice A only ever records `Open`; the
  transitions land in sub-slice B.
- **A finding's freshness is single-location, identical to an observation's.** A
  finding binds to one location, so "did the input under this change" must be
  decided exactly as a same-location observation decides it. Rather than
  duplicate the verdict, the observation reconcile body was extracted into a
  shared free fn `location_freshness_verdict`; `observation_reconcile_event` and
  the new `finding_reconcile_event` both call it, so they can never drift. The
  existing observation suite guards the extraction as behavior-preserving.
- **Native payload is the provider's output, not the source file (S8).** A
  finding's native payload is the provider's own raw result (e.g. diagnostic
  JSON), supplied by the caller on stdin like `observe-read`'s text — distinct
  from the source file at `path`, which is only the freshness binding. It is
  retained in the CAS keyed by its own digest (`native_payload_fingerprint`,
  separate from the file's container fingerprint), and `reveal-finding` verifies
  bytes against that digest, failing closed on a missing payload or tampering.
- **Surface.** `record-finding` (severity/rule/message/location + optional stdin
  payload) and `reveal-finding`; findings ride the single-pass reconcile in
  `resume_status`, appear in `status --full`, and `status` gains an
  `open_findings` count. Covered by two acceptance tests (S8 provenance
  round-trip; edit → stale + fail-closed reveal). 42 Rust tests, strict fmt and
  clippy. Deferred to B/C: disposition transitions, and the bounded queue
  projection + `workspace_findings` Pi tool.

## 2026-09-02 — Findings, sub-slices B + C (disposition + bounded queue + Pi)

Completes the findings action.

- **Disposition transitions (B).** `dispose-finding --disposition
  resolved|deferred|suppressed|false-positive --actor --rationale` records a
  `FindingDispositionChanged` event; the kernel refuses an empty actor or
  rationale (invariant 8) and refuses disposing back to `Open` (a reopen verb, if
  wanted, is a separate future transition). The `FindingDisposition` value itself
  carries actor+rationale, so the event is just `{finding_id, disposition}`.
  Disposition is applied in the projection and survives restart. It never touches
  freshness — a disposed finding keeps reconciling — which is the whole point of
  keeping the two axes separate.
- **Bounded quickfix queue (C).** `findings` projects `FindingsView`: open
  findings ranked most-severe-first (a cap must never hide an error under a hint;
  `FindingSeverity` derives `Ord` for exactly this), then stable by id, hard-capped
  at 12 with an explicit `open_omitted`, plus a freshness histogram over the open
  set and a `disposed` count for the audit tail. Freshness rides each row rather
  than the ranking, so a severe issue is never demoted because an edit landed
  near it. No native payload retained — `reveal-finding` is the escape hatch.
  Pure projection over the reconciled status, like `working_set_view`/`brief`.
- **Pi (C).** A parameterless `workspace_findings` tool shells `findings
  --compact` through the shared `runKernel`, mirroring the other orientation
  tools.
- **Coverage.** One Rust test drives severity ranking, disposition-with-actor/rationale,
  queue removal on disposition, invariant-8 refusal, and restart persistence; one
  Pi test asserts the tool wiring. 43 Rust tests, 6 Pi tests + typecheck, strict
  fmt and clippy. All findings action criteria — normalized queue, native payloads,
  provenance, revision, dispositions, and edit-driven invalidation — are met.

## 2026-09-02 — Transaction associations, intent, and preview

An audit first: `begin`/`apply`/`revert` and the `accept` verb (which is BOTH
"validate" and "commit" — it accepts only when every acceptance claim is current
with current passing evidence) already existed (S6/S9/S10). The gap the action
named was the *associations*: intent, findings, residual risks, and a preview.

- **Intent is now required at `begin`.** `Transaction.intent` (Option for
  backward-compatible replay; required for new transactions). This is the
  charter's first-listed transaction field and the anchor a preview reads.
- **Findings and residual risks associate onto an open transaction.**
  `associate-finding` (link only — it never disposes the finding, which stays a
  separate actored act; idempotent) and `record-risk` add `finding_ids` /
  `residual_risks`, via `TransactionFindingAssociated` / `TransactionResidualRiskRecorded`
  events. Both refuse a closed transaction.
- **`preview-transaction` is the review-before-accept surface.** A pure
  `WorkspaceStatus::transaction_preview` projection: intent, affected locations
  (distinct mutation paths in first-touch order), associated findings (with
  freshness), bearing evidence, acceptance claims, residual risks, and
  `ready_to_accept` + reason. Readiness is computed by `acceptance_readiness`,
  which evaluates the SAME rule `accept` enforces, read-only — a test drives both
  directions (preview says not-ready → accept rejects; preview says ready →
  accept succeeds) so the advisory mirror can't drift from the authority.
- **Deferred as charter non-goals:** true changed-symbol identity and
  dependency-level blast radius need the symbol model the charter explicitly
  declines to over-invest in. "Affected locations = mutation paths" is the
  honest, derivable version shipped here.
- **Surface + coverage.** `begin-transaction --intent`, `associate-finding`,
  `record-risk`, `preview-transaction`, and a `workspace_transaction_preview` Pi
  tool (the first tool taking a parameter). One Rust test covers intent-required,
  associations, preview↔accept agreement, and restart persistence; one Pi test
  covers the id passthrough. 44 Rust tests, 7 Pi tests + typecheck, strict fmt
  and clippy.

## 2026-09-02 — Live transaction symlink hardening

The final dogfood pass was the first transaction begun against the live project.
It failed before opening: `worktree_fingerprint` enumerated the tracked symlink
`.claude/skills -> ../.agents/skills`, then `fs::read` followed it into a
directory and returned `EISDIR`.

- Worktree fingerprinting now uses `symlink_metadata`. Symlinks contribute a
  type marker plus their link-target bytes and are never followed; directory
  entries contribute an explicit type marker rather than failing. Regular files
  retain byte-content fingerprinting and missing entries retain the existing
  marker.
- A regression fixture commits a symlink to the tracked `src` directory, records
  an acceptance claim, and proves `begin-transaction` returns an open
  transaction. The suite now has 45 Rust tests.
- The same run exposed a separate acceptance limitation rather than hiding it:
  markdown autoformat changed the final-newline bytes after mediated apply, yet
  claim/evidence readiness still allowed acceptance because candidate mutation
  bytes are not re-verified. That work is tracked by the
  `candidate-state-evidence` action, not folded into this symlink fix.

## 2026-09-02 — Bounded orientation and working-set curation

Closes the `orientation-hardening` action. The dogfood measurements (6.1–6.5 s
compact status; a syscall trace with 281 `git rev-parse` and 114 rustfmt
attempts) showed the default surfaces were bounded in *output* but not in
*work*: they reconciled every observation, claim, evidence, and finding —
including 53 superseded claims and hundreds of retired observations — to serve
projections that only present active state.

- **Selective reconciliation by served surface.** `resume_brief_status`
  reconciles only active claims; `delta_brief_since` only active claims (lifecycle
  transitions and ids are replay facts, so superseded-claim deltas need no
  freshness verdict); `resume_findings_view` only open findings;
  `resume_transaction_preview` exactly the claims, evidence, and findings one
  preview exposes; `resume_working_set_view` focused observations plus a bounded
  recent uncited-candidate window (24 newest ids). The exhaustive paths
  (`resume_status`, `delta_since`) are unchanged and remain the audit defaults
  behind `--full` and `checkpoint`.
- **F9 is preserved by construction.** Every verdict a bounded surface emits is
  recomputed from current inputs before serialization; nothing outside that set
  is emitted at all. The new test proves both directions in one fixture: an edit
  under an active claim surfaces as `stale` through the bounded path, while the
  event log shows exactly one `claim_reconciled` — the retired claim beside it
  was never touched. `uncited_omitted` in bounded mode counts outside-window
  observations as omitted because their stored verdicts are inherited and
  cannot be served without reconciling them; inside-window stale observations
  are known non-candidates and are excluded without inflating the count.
- **Working-set curation.** When stale history fills the 12-location cap, the
  last slot is reserved for the newest current focus, so active attention can
  no longer be crowded out by invalidated entries (the omitted stale entries
  stay counted in `locations_omitted`). Uncited candidates are now served
  newest-first, matching the window they come from.
- **Measured on the live workspace** (6,669+ events, 198 observations, 53
  claims): status 6.1–6.5 s → 0.29–0.69 s; delta 6.8 s → 0.33 s; working-set
  → 0.56 s. The exhaustive audit path now costs 11.6 s and is strictly
  opt-in (`--full`). The remaining cost is one full log replay per invocation
  plus bounded reconciliation; snapshot/tail-replay materialization is the
  next lever if interactive budgets tighten further.
- **Coverage.** 48 Rust tests (three new: bounded-status F9 + retired-history
  skip with event-log proof, current-focus reservation under a stale cap,
  unverified-candidate omission), strict fmt/clippy, 7 Pi tests + typecheck.
  The single-pass log-reads test now settles through `status --full`, since
  the default bounded status intentionally no longer settles retired state.

## 2026-09-02 — Candidate-state evidence (accept re-verifies owned bytes)

The dogfood proved a soundness hole: `accept` checked that acceptance claims
were current with passing evidence, but never re-read the mutated files. So a
formatter reflowing a file between `apply` and `accept` went undetected — the
committed bytes could differ from the bytes the checks consumed, while the
workspace still reported "accepted". Evidence, meanwhile, reconciled only the
*claim's cited inputs*, which need not even be the mutated paths; nothing tied a
passing check to the candidate being committed.

Considered and rejected: re-founding on parse trees so formatter drift stops
mattering. Bytes fail toward false *staleness* (fail-closed, safe); a semantic
tree normalizer fails toward false *freshness* (fail-open) the first time it
drops a token that turns out to be significant (`// SAFETY`, `#[cfg]`, macro
whitespace). For a "never silently go stale" substrate that failure direction
is disqualifying at the core, and it would couple a language-agnostic kernel to
a parser zoo while still needing a byte fallback. The existing
`Normalizer::Rustfmt` seam already neutralizes formatter drift for locations
that opt in, degrading to raw bytes — the wise version. Semantic *anchoring*
(claims surviving line-shifts) is a real but separate axis for the location
layer, opt-in and fail-closed; deferred to its own action.

- **Candidate fingerprint = pure projection over the mutations.**
  `Transaction::candidate_fingerprint()` folds the sorted `(path,
  after_fingerprint)` set into one content address. No stored field — one source
  of truth, derived like semantic-location. Empty mutations → stable
  empty-candidate digest.
- **Evidence binds to the candidate it was recorded against.** New
  `Evidence.candidate_fingerprint` (serde-default `""` = legacy unbound, which
  never satisfies the gate). `record_evidence` also gains a **materialization
  gate**: every owned path must already hash to its `after_fingerprint` at
  record time, else the check could not have consumed this candidate and the
  evidence is refused. That is the honest, locally-checkable reading of "prove
  each passing check consumed the candidate" — the candidate was materialized on
  disk when the check ran, and (below) has not moved since.
- **`accept` fails closed on two gates.** (a) **Disk re-verification**
  (`candidate_drift`) re-reads each owned path and rejects, naming the path, if
  the bytes no longer match — catching post-apply formatter drift. (b)
  **Candidate binding** in `acceptance_readiness`: the passing evidence must
  carry `candidate_fingerprint == the transaction's current candidate`, so a
  check recorded before a further path was mutated no longer counts. Binding is
  orthogonal to the evidence's own input-freshness — same freshness-vs-
  disposition split used elsewhere.
- **One rule, two callers — parity is now structural.** `accept` previously
  *inlined* its acceptance check, duplicating `acceptance_readiness`; that
  duplication was itself the drift risk this action targets. Both `accept` and
  `preview` now compose `candidate_drift` (disk) with `acceptance_readiness`
  (pure), so the advisory preview cannot promise a readiness the authority would
  deny. `transaction_preview` takes the drift verdict as an injected parameter,
  since the projection has no filesystem.
- **Native provenance preserved.** The check still runs natively; the kernel
  only binds its result to the candidate. `provider`/`invocation`/`check_name`
  are untouched.
- **Coverage.** 50 Rust tests (two new: disk re-verification rejects drift with
  byte-exact recovery and preview↔accept parity in both directions; evidence
  candidate-binding + the record-time materialization gate). Strict fmt/clippy.

## 2026-09-02 — External state resolution (foreign-dogfood, portability slice 1)

The project-local prototype hardwired workspace state to `<repo>/.agent-workspace`:
every adapter passed `--workspace <repo>/.agent-workspace`, and `Workspace::open`
trusted that path verbatim. That is fine while the workspace only ever observes
its own repository, but foreign dogfood needs an installed kernel that resolves
*one logical workspace per project* under an external, XDG-style local state
root — dynamic state must not travel implicitly inside the observed repo's Git
tree (see `docs/decision-external-workspace-and-clearhead-boundary.md`).

This is the first, deliberately thin cut: location only. Registry, global Pi
projection, and workstream/worktree/session partitioning are follow-up slices,
pulled next by the foreign handoff.

- **New leaf module `src/locate.rs`.** `resolve_state_root(repository_root,
  workspace_override, state_root_override)` owns resolution and nothing else, in
  keeping with the ongoing lib.rs modularization. Precedence: an explicit
  `--workspace` (legacy, verbatim) short-circuits everything so existing adapters
  and fixtures keep working while they are repointed; otherwise a state-root base
  is joined with a project identity subdirectory.
- **State-root base.** `--state-root` → `$AGENT_WORKSPACE_STATE` →
  `$XDG_STATE_HOME/agent-workspace` → `$HOME/.local/state/agent-workspace`.
  Resolved manually (empty env values treated as absent) rather than pulling a
  `dirs`/`xdg` crate — the XDG spec is trivial here and the dep is not worth it.
- **Project identity = content address of the git *common* directory.**
  `git rev-parse --git-common-dir`, made absolute against the repo and
  canonicalized, then `hex_digest`-ed (reusing the kernel's existing SHA-256
  helper). Linked worktrees of one repository share the common dir → share state;
  independent clones each have their own → stay separate; the remote URL is
  deliberately ignored, so matching remotes never silently merge. A non-git
  target falls back to the canonical repository path so ad hoc directories remain
  usable. The digest is opaque on purpose — the human-readable name↔identity
  mapping is exactly what the deferred registry slice provides.
- **CLI.** `--workspace` is now optional (a legacy override); `--state-root` is
  new; `run` resolves before `Workspace::open`. Usage string updated.
- **Coverage.** 52 Rust tests (two new: worktree-shares / clone-separates against
  a real `git worktree`, and the `--workspace` override bypassing resolution).
  Env-fallback branches are left to the linear `non_empty_env` reads rather than
  racy in-process env mutation under parallel tests. Additionally driven live
  through the real binary: `bind-objective` with only `--state-root` landed state
  under `<state-root>/<hash>/` (not in the repo), status round-tripped, and a
  linked worktree resolved to the same hash dir and read back the same objective.
- **Residual.** Adapters (`.claude/hooks/*.py`, Pi `index.ts`) still pass
  `--workspace <repo>/.agent-workspace` and are unchanged this slice; repointing
  them at `--state-root` is the next step, alongside the registry and global Pi
  projection.

## 2026-09-02 — Claude Code adapter repointed to external state (portability slice 2)

Slice 1 taught the kernel to resolve an external, project-scoped state root but
left every adapter still passing `--workspace <repo>/.agent-workspace`, so
nothing actually used the portable path. This slice moves the first live
interface — the Claude Code adapter — onto it.

- **The repoint is one function.** All three hooks funnel through
  `workspace_runtime.runtime_for`, which used to return `(root, binary,
  workspace_dir)` and hand each hook the in-repo `.agent-workspace`. It now
  returns `(root, binary)`; `capture-read.py` and `orient-session.py` pass only
  `--repository` and let the kernel resolve where state lives. A thin transport
  must not second-guess resolution — that was the whole point of slice 1.
- **New `state-path` kernel command.** Pure resolution: prints the state root the
  kernel *would* use for a repository, without opening or creating it or taking a
  lock. Two reasons it earns its keep: transparency (a human or adapter can ask
  "where did my workspace go?"), and it gave the one-time migration an exact
  target computed by the kernel's own hash rather than a fragile bash
  reimplementation of `sha256(realpath(git-common-dir))` — the kind of duplicated
  logic that drifts.
- **Migration, not reset.** The live `.agent-workspace` (8925 events, the whole
  self-dogfood history my own SessionStart orientation reads) would have been
  orphaned by the flip. So it was **copied** to
  `~/.local/state/agent-workspace/<hash>` — the path `state-path` reports — and
  the original left in place as a frozen backup. Non-destructive and reversible:
  deleting the external copy restores the prior world. Verified the migrated
  state reads back through the new resolution (objective, claim 61, and the
  slice-1 checkpoint all present).
- **The orientation drive had to move too.** `orient_session_drive.py` recomputed
  its "expected" status/delta by shelling the kernel with the pinned in-repo
  `--workspace`; against the repointed hook that compares two different
  workspaces. Both reference commands now drop `--workspace` so the drive and the
  hook resolve the same place. 16/16 checks green.
- **Verification.** Orientation drive 16/16 (reading external); a synthetic
  `PostToolUse(Read)` drove `capture-read.py` to record observation 288 into the
  external workspace while the frozen backup stayed at 8925 events; 53 Rust tests
  (one new: `state-path` prints the resolved root without creating it); fmt +
  clippy clean.
- **Residual.** The Pi adapter (`.pi/extensions/agent-workspace/index.ts` and its
  tests) still passes `--workspace` and is deliberately unchanged this slice, to
  keep the change reviewable. Repointing Pi, the project registry, and the global
  Pi projection are the next cuts before the real foreign-repo task.

## 2026-09-02 — Pi adapter repointed to external state (portability slice 3)

Mirror of slice 2 for the second interface. `index.ts` had exactly two
`--workspace` sites — `captureRead`'s `observe-read` and `runKernel`'s
orientation invocation — plus a `workspace` field on `RepositoryRuntime` built as
`join(root, ".agent-workspace")`. All three are gone: the runtime carries only
`{ root, binary }`, and both invocations pass `--repository` alone and let the
kernel resolve.

- **No second migration.** Pi observes *this* repository, so it resolves the same
  git-common-dir identity as the Claude adapter and lands in the workspace slice 2
  already migrated to. The two adapters now share one substrate — which is the
  decision doc's "multiple agents share the repository/workstream substrate
  without sharing a working set" claim getting exercised for the first time.
- **The hermeticity worry was a false alarm.** I expected Pi's tests to need an
  `AGENT_WORKSPACE_STATE` tempdir to avoid polluting real `~/.local/state`. They
  don't: `index.test.ts` runs against a *fake* kernel (a stub `pi.exec` and a
  throwaway Node script as the binary) and asserts on the argv the extension
  builds, never touching real state. So the test change was purely updating four
  arg-assertions to no longer expect `--workspace` (the `workspace_delta` and
  degradation tests never asserted it). Reading the tests before acting saved a
  needless env-plumbing detour.
- **Verification.** 7/7 Pi tests + `tsc --noEmit` clean. Then the integration the
  unit tests can't reach, driven against the real binary: the exact arg vector
  `captureRead` now builds (`observe-read --repository . --provider pi.read
  --model-visible-bytes N --offset --limit`, no `--workspace`) recorded a
  `pi.read` observation into the shared external workspace, and `status` there
  returned the same objective and active claims (including 61/62 recorded by the
  Claude adapter) — a Pi agent resuming here inherits the shared orientation.
- **State of the objective.** Both interfaces are now portable and share one
  external substrate. Still ahead before trust is actually measured: the project
  registry (human-readable name↔identity, explicit cross-clone identity), the
  global Pi projection, and the genuine multi-session foreign-repo task.
