---
type: Decision
title: Sequence freshness-cost work as diff-on-stale, then drift instrumentation, then relocatable selectors
description: Deprioritizes the relocatable exact-text selector proposal behind diff-on-stale and a pure drift-frequency diagnostic, so trust-critical reconcile machinery is built only after the coordinate-drift cost is measured.
tags: [architecture, freshness, reconcile, sequencing]
generated: { by: claude-code/opus-4.8, at: 2026-09-07T22:36:15Z }
---

# Decision — sequence the freshness-cost work; do not adopt the relocation record shape yet

**Status:** agreed 2026-09-07; no implementation started.
**Date:** 2026-09-07
**Participants:** user + assistant, reacting to the pi/gpt-5.4 proposal.

## Context

The [relocatable exact-text selectors proposal](../research/relocatable-exact-text-selectors.md)
(authored by a foreign gpt-5.4 dogfood instance, committed as authoritative-looking
research) targets a real cost: `ObservationSelector::ByteRange` is positional, so
unchanged observed text that *moves* because of insertions/deletions before it
reconciles against the wrong bytes and reports `stale`/`unknown`.

The proposal is careful work — it keeps the exact captured region as the sole
identity authority, refuses to revive the rejected CST/structural-freshness
experiment, and gates itself behind a falsifiable spike. Those qualities are worth
preserving.

## Decision

**Do not adopt the relocation record shape now.** Sequence the freshness-cost work:

1. **diff-on-stale first.** It only makes a legitimate `stale` verdict cheap to
   investigate; it never touches the accept path, so it carries near-zero risk to
   the "stale outranks memory" trust guarantee and pays off regardless of how
   often coordinate drift actually occurs.
2. **Instrument drift as a pure diagnostic.** When reconcile returns `stale`,
   cheaply record whether the old exact bytes still exist elsewhere in the file —
   verdict unchanged, just a counter. Dogfooding then answers the question the
   proposal cannot: how common is pure coordinate drift?
3. **Relocation spike third, gated on that number.** Build the candidate-scan /
   anchor / occurrence-context machinery only if measured drift is frequent enough
   to justify adding cleverness to the one path where a false-`current` is fatal.

## Rationale

- **The proposal argues itself out of being next.** It concedes diff-on-stale is
  "likely the cheaper immediate product improvement" and that relocation is
  "worthwhile only if measured coordinate-drift cases are common enough" — and that
  frequency is unmeasured. Building trust-critical reconcile machinery for an
  unquantified problem violates measure-before-engineering.
- **The acute pain is already mitigated.** Partial `ByteRange` reads already stopped
  whole-file claims from staling on unrelated edits. Relocation is a second-order
  refinement on top of that.
- **The safe band may be thin.** Units that relocate *uniquely and safely* are
  large; units that actually move around are often small one-line reads — exactly
  the ones the proposal admits must return `unknown`. The intersection may be small.
- **Reconcile is the trust-critical path.** `raw_fingerprint` + two `BoundaryDigest`
  anchors + `OccurrenceContext` + rolling-hash prefilter + uniqueness arbitration is
  a large new surface on the one path whose false-positive breaks the tool's premise.

## Consequences

- The committed relocation proposal is **not** the roadmap; this record is the
  counter-signal a cold agent must read alongside it.
- The proposal's safety invariants stay on the shelf, to be reused if step 3 runs.

# Related Concepts
- [Relocatable exact-text selectors](../research/relocatable-exact-text-selectors.md): The proposal this decision defers and re-sequences.
