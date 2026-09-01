# Implementation Notes

This document records choices settled by walking-skeleton evidence. The
executable contract remains authoritative for behavior; these choices may be
revised when later scenarios expose a better boundary.

## 2026-09-01 — S1 observation staleness

- The kernel begins as a harness-neutral Rust library and standalone executable.
  Pi, Claude Code, other agent harnesses, and Neovim will remain projections or
  adapters rather than dependencies of the kernel.
- Persisted events use versioned JSON Lines. The format is language-neutral even
  though the first producer is Rust.
- The first implementation is synchronous and replays its append-only log to
  materialize state. A daemon framework, database, watcher, and agent-specific
  protocol are deliberately deferred.
- An observation records a repository-relative path, provider identity, Git
  revision, and SHA-256 input fingerprint.
- Reconciliation fingerprints are scoped to the observed input plus Git
  revision. They do not claim to fingerprint the entire repository.
- S1 is exercised through the standalone executable: record an observation,
  mutate its source out of band, reconcile, and recover a reasoned `stale`
  verdict from the persisted events.
