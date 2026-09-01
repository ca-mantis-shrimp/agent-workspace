# Review — S6 Implementation (2026-09-01)

*Commentary on the clean-base transaction rollback slice (`src/lib.rs`
`apply_file_mutation` / `revert_transaction`, `tests/walking_skeleton.rs`,
commit `562f5b9`). Reviewed against [`executable-contract.md`](executable-contract.md)
invariant I10 and failure-model item F8 (destructive revert), and the S9/S10 scenarios.
Verified: `cargo test` 12 green, clippy clean, code read end-to-end.*

## The F8 safety verdict: structurally prevented, not merely untested

F8 (a revert that destroys unrelated/committed work, or reports success on an ambiguous
revert) is the scariest item in the contract. It is closed by construction, not by luck:

- **Revert is surgical.** It writes *only* each owned `mutation.path`, reconstructing base
  content via `git show <base>:<path>` and an atomic same-dir rename. There is no wholesale
  `git reset`/`checkout`. Unrelated files — including pre-existing dirty ones — are never
  touched, so **S9's property (unrelated changes survive) holds as a design consequence**, even
  though S9 has no dedicated test yet.
- **Revert refuses on ambiguity.** Per owned path it checks current bytes ==
  `after_fingerprint` (else "revert conflict") *and* base bytes == `before_fingerprint` (else
  corrupt-log). A concurrent overlapping edit makes it **halt, not clobber** — S10's no-destroy
  property, again structural. The mutation side likewise refuses to write a file that does not
  already byte-match its git base, so uncommitted work cannot be overwritten in the first place.

This is the correct I10 posture — halt on ambiguity, never destroy — and it is the right call
for the highest-stakes slice.

## Finding: partial-revert atomicity on multi-path transactions

`revert_transaction` (`lib.rs:790–810`) checks-and-writes in a **single loop**, so it can
mutate the worktree for some owned paths before discovering a conflict on a later one. Multi-path
transactions are reachable today: `apply-mutation` is a single CLI verb (and library call) that
may be invoked repeatedly on one open transaction with different paths (only a *duplicate* path
is refused).

**Failure scenario:**

1. Open a transaction; `apply-mutation` to `A` (clean at base), then to `B` (clean at base).
   Both are now owned.
2. An out-of-band edit changes `A`.
3. `revert`: iterates reversed `[B, A]`. `B` passes both guards → restored to base. Then `A`
   fails guard 1 → returns `Err("revert conflict on A")`.
4. Result: `B` is at base, `A` is still at the edited state, the transaction is **still Open**,
   and re-running `revert` now fails guard 1 on `B` (no longer at its `after` state) — the
   transaction is **wedged, permanently unrevertable.**

**Severity:** this is *not* F8 — no committed or unrelated work is lost (base states are safe,
and `B`'s reverted content was the transaction's own in-progress edit, which revert exists to
remove). It is a genuine **consistency hazard in shipped, reachable code**: a half-reverted
transaction with no clean state. S6 deliberately deferred conflict *hardening* to S10, which is
fair — but the multi-path revert path is reachable *now* and can wedge, which is more than a
deferred scenario.

**Recommended fix (cheap, and the foundation S10 needs anyway): two-phase revert.** Validate
every mutation's guards read-only first; only if *all* pass, perform *all* the writes. A
conflict then aborts before touching disk, making revert all-or-nothing. (A narrower
alternative that matches S6's stated scope: gate transactions to a single owned mutation until
S10 hardens multi-path. Two-phase is preferred — more general, and directly reusable by S10.)

## Honestly handled, no action needed

- **Crash window.** The gap between the atomic filesystem write and the durable event append is
  flagged as an open recovery window; the best-effort compensation (rewrite `before` on append
  failure) is not crash-atomic, and the notes correctly say prepared/applied journaling is
  required before it can be. Correct disposition.
- **Acceptance seam.** The descriptive-claim-vs-normative-criterion seam from S4 is explicitly
  kept open ("post-mutation acceptance remains provisional … until `AcceptanceCriterion`
  replaces S4's `acceptance_claim_ids` scaffolding") rather than papered over.
- **Freshness on mutate/revert.** Both cross reconciliation boundaries, so descriptive claims go
  stale after an owned mutation and current again only when clean rollback restores their exact
  recorded inputs — consistent with the S4-seam guidance.

## Bottom line

S6 is safe on the path it proves, and F8 is genuinely closed by conservative refusal. The single
thing worth fixing before S10 builds on the conflict path is the **two-phase revert**, so a
multi-path revert can never leave a transaction half-reverted and wedged.
