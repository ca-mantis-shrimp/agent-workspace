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
