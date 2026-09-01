# Design note — S7 bounded perception (commentary, not spec)

*Written while dogfooding the workspace on a cold resume. Assesses how to make S7
falsifiable with the thinnest possible kernel change. Anchored to
`executable-contract.md` §S7 and the three-axis structs in `src/lib.rs`
(`FreshnessReport`, `ScopeAssurance`, `OperationalCoverage`).*

## S7 restated

Fixed repo task, run **raw** vs **workspace-assisted**. Pass requires **all** of:
1. both arms complete the task (mechanically checkable — a test passes);
2. the assisted arm **ingests fewer bytes/tokens** under a *defined accounting boundary*;
3. full provider detail stays **retrievable on demand**.

Teeth: an outline that cuts bytes but fails the task does **not** pass.

## The load-bearing realization: no new freshness axis is needed

Bounded perception looks like it needs a new concept, but the existing three-axis
model already separates the two questions it raises:

- *"Did the part I actually perceived change?"* → `freshness_within_scope`.
- *"Can I rule out a relevant change in the part I did **not** perceive?"* →
  `scope_assurance.completeness` (stays `not-asserted` for a slice).

So bounded perception is **not** a new axis. It is a change to *what a mediated
unit is*: today `OperationalCoverage.mediated_paths` is a whole path fingerprinted
whole; S7 needs the unit to be a **sub-file region carrying two fingerprints**.

## Proposed observation shape (dual fingerprint)

A bounded observation records:

- `container_path` + `container_fingerprint` — the whole file's digest. **Retained**
  so a future reconciliation can still detect drift *outside* the slice. Dropping
  this is how F1 re-enters through the efficiency door.
- `unit_selector` — how the slice was chosen (v1: a byte/line range; later: a
  provider-defined node). Provider-agnostic to start.
- `unit_fingerprint` — digest over the perceived slice. This is what drives
  `freshness_within_scope`: the claim is `stale` iff the *slice* changed.
- a retrievable handle so `reveal` can return full detail on demand.

Mapping to the model: `freshness_within_scope` keys on `unit_fingerprint`;
`scope_assurance.completeness` stays `not-asserted` (we did not assert the rest of
the file is irrelevant); `container_fingerprint` is what lets a later, stricter
reconciliation escalate if a claim's coverage actually needed the remainder.

## Thinnest falsifiable experiment (no tree-sitter required)

- **Task:** add a correct call to an existing function `foo` at a new site. The
  needed information is `foo`'s *signature* — a small, localized slice — not its
  body nor the other N functions in the file. Success = the crate compiles / a
  target test passes.
- **Raw arm:** observe the whole file → ingest all N bytes.
- **Assisted arm:** observe only the signature slice → ingest k ≪ N bytes; body
  `reveal`-able on demand.
- **Accounting boundary:** *bytes returned into the agent's context by observe/reveal*,
  summed per arm. (Bytes on disk are not the boundary; bytes *ingested* are.)
- **Pass:** both arms yield a passing test **and** assisted ingested-bytes < raw
  ingested-bytes **and** `reveal` returns the full file.
- **Fail-guard (contract teeth):** if the assisted edit fails the test — e.g.
  because the signature slice was too small to call `foo` correctly — it does
  **not** pass. This is what stops "reduce bytes by guessing."

## Minimal kernel work this implies

1. A bounded `observe` (v1: `--range`) producing the dual-fingerprint observation.
2. A `reveal --observation <id>` verb returning full container detail.
3. Byte accounting on observe/reveal (an `ingested_bytes` the experiment can sum).

Deliberately **not** in v1: tree-sitter / semantic units (a later provider), token
(vs byte) accounting (byte proxy is enough to falsify first).

## Why this is the right first cut

It makes S7 *measurable* with a range and a counter — the smallest thing that can
be **false**. It reuses the three-axis model instead of growing it. And it forces
the F1-honest invariant up front: bounded perception is a **lens over** a full
observation, never a lossy replacement of it.
