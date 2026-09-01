# Review — S1 Implementation (2026-09-01)

*Commentary on the first walking-skeleton slice (`src/lib.rs`, `src/main.rs`,
`tests/s1_observation_staleness.rs`, commit `dee05a8`). Reviewed against
[`executable-contract.md`](executable-contract.md) rev. 2. This is commentary, not a patch:
finding #1 is a design decision that belongs to the implementer, not something a reviewer
should quietly impose.*

## Verification performed

Not a read-only review. `cargo test` builds clean and S1 passes; the CLI was also driven by
hand (observe → out-of-band edit → reconcile) against a throwaway Git repo, and the persisted
`events.jsonl` was inspected and replays coherently. The observations below are grounded in that
real output.

## What is sound

The skeleton makes the right structural bets and should not be relitigated:

- Event-sourced: append-only JSONL, events store raw facts, the projection *derives* the view.
- Replay is defensive — strict monotonic `sequence`, `schema_version` check, and duplicate-
  observation detection all fail **loud** as `CorruptLog` rather than silently (good F5 posture).
- Path-traversal validation, `fsync` on append, harness-neutral library boundary with a thin CLI.
- The scoped fingerprint correctly makes S1's "fingerprint changed" assertion meaningful without
  claiming to fingerprint the whole repository.

The deferrals in `implementation-notes.md` (synchronous, no daemon/DB/watcher) are the right
calls for this slice.

## Findings

### 1. Default `completeness: asserted-complete` — a latent F1 landmine *(design decision, surfaced not imposed)*

A freshly recorded single-file observation is projected (`lib.rs:315–324`) with:

```json
"scope_assurance": { "source": "derived", "completeness": "asserted-complete" }
```

The three-axis model exists precisely to honor rev. 2's rule *"never upgrade a scoped result
into an unqualified truth claim"* — and the first record asserts the **strongest** assurance by
default.

There is a narrow defense: an observation *qua raw bytes* has a scope that genuinely is "exactly
this file," so byte-completeness is true. The risk is not in S1; it is at the seam S2/claims will
land on:

- The distinction "observation-scope (byte-complete) ≠ claim-scope (dependency-complete)" is
  **undocumented**. An agent reading the field will not reconstruct it.
- If a **claim** ever inherits this default, that is F1 by construction — a claim asserting its
  dependency scope is complete when nothing established that.

So this is a representation question for you, not a bug to be patched blindly: *should
observations even carry a `completeness` assertion, or is that field meaningful only at the claim
layer?* One honest option is to default observations to `not-asserted` — it costs nothing and
removes the trap for readers who won't make the subtle distinction. Whatever you choose, record
the reasoning, because this is the load-bearing seam for the next slice.

Mitigating: the value is *derived in the projection*, not persisted, so any change here is
projection-only with no log migration.

(Secondary: `source: "derived"` is questionable — nothing derived the scope; it is "the file
named." Minor beside `completeness`, but it suggests `ScopeSource`'s meaning for observations is
underspecified.)

### 2. I/O errors collapse to `stale`; they should be `unknown`

`reconcile` (`lib.rs:208–211`) maps *any* read failure to `Stale / "supporting input
unavailable"`. A genuinely deleted file → `stale` is defensible, but a transient or permission
error means *could not check* — which is exactly `unknown`. Conflating "changed" with "could not
verify" erases the distinction the three states exist for. Note the irony: `unknown` is fully
defined in the types yet produced **nowhere**, and its most natural first home — an unreadable
input — is currently mislabeled `stale`. Small, real, worth fixing in this slice.

### 3. `repository_fingerprint` is a misleading name in the audit surface

The field is a *scoped* fingerprint of `(revision, path, input-hash)` — which
`implementation-notes.md` explicitly says is **not** a repository fingerprint. Yet both the
struct field and the persisted event-log key are named `repository_fingerprint`. For a project
whose stated value is transparency, a field that overstates its coverage *in the inspectable log*
should be renamed (e.g. `scoped_fingerprint` / `coverage_fingerprint`). Cheap; do it before the
name ossifies across more events.

### 4. Correctly deferred — but record them as decisions, not accidents

- Every operation replays the full log; `record` triggers ~3 replays (id lookup, `append`'s
  sequence lookup, re-fetch) → O(n²) over a session. Fine for the skeleton — name it as a known
  limitation.
- No writer lock on the append log. Concurrent writers collide on `sequence` and are caught as
  `CorruptLog` on replay, so it *fails safe* — but it is unlocked. Appropriate to defer; name it.

## Routing

- **Fix in this slice:** #2 and #3 (both small and unambiguous).
- **Decide and document in this slice:** #1 — it is the representation seam S2 depends on.
- **Record as named deferrals:** #4, in `implementation-notes.md`.

Nothing here blocks the *behavior* S1 was meant to prove — the slice is correct for its scope.
These are about not letting the honesty guarantees erode at the exact point the next slice will
build on.
