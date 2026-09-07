---
type: Research Proposal
title: Relocatable exact-text selectors
description: Proposes reducing offset-drift and whole-file freshness costs through conservatively relocatable exact text regions, with optional structural hints that never broaden observed support.
generated: { by: agent/cli, at: 2026-09-07T22:38:56Z }
---

# Proposal — relocatable exact-text selectors

> **Status:** proposal for a bounded experiment, not an accepted design or
> scheduled implementation. Written after the third `plot` foreign-dogfood
> slice. It narrows the earlier symbol-selector idea to a language-independent
> relocation problem and deliberately does not revive semantic/CST freshness.

## Motivation

Agent Workspace already captures partial reads honestly as
`ObservationSelector::ByteRange { start, end }`. That is substantially better
than binding every claim to a whole file: a change outside the selected range
can leave the observation current. The remaining weakness is that the selector
is positional. If unchanged observed text moves because lines are inserted or
removed before it, reconciliation applies the old offsets to different bytes
and reports stale (or unknown if the old range no longer fits).

Foreign dogfood exposed the user cost behind this distinction. Whole-file
claims became stale after unrelated flag and documentation edits, and the agent
had to re-read enough source to answer whether the asserted behavior had really
changed. During the subsequent bar-mark slice, partial observations reduced
that cost, but discussion converged on a general question: can a selected text
unit follow harmless surrounding motion without requiring a universal AST or
silently broadening what the model saw?

## Thesis

Treat relocation and relevance as separate operations while keeping relevance
conservative:

1. **Relocation asks where the exact observed unit is now.**
2. **Freshness asks whether that relocated unit has the same recorded identity
   under its existing normalizer.**

The first cut should relocate **exact model-visible text**, not infer semantic
equivalence. A uniquely relocated unchanged unit may remain current. Changed,
missing, or ambiguously relocated material must not. This is useful for source,
Markdown, configuration, and other UTF-8 text without a parser zoo.

Tree-sitter or LSP may later contribute optional location hints and readable
labels, but they are not the authority for what supported the claim. The exact
captured region remains that authority.

## Proposed record shape

Preserve `WholeFile`. Evolve byte-range observations with a versioned relocation
record rather than introducing a language-specific selector as the foundation:

```rust
struct ExactTextRegionV1 {
    original_start: usize,
    original_end: usize,
    byte_length: usize,
    raw_fingerprint: String,
    before_anchor: Option<BoundaryDigest>,
    after_anchor: Option<BoundaryDigest>,
    occurrence_context: OccurrenceContext,
}
```

The event need not retain the model-visible payload. Given the recorded byte
length and raw digest, the kernel can scan bounded candidate windows in the
current file and cryptographically confirm matches. A recorded rolling-hash
prefilter may make that scan linear without becoming identity authority: only
the existing cryptographic raw fingerprint can confirm an exact candidate.
This preserves the current no-native-payload default and avoids inventing a new
CAS retention path merely for relocation.

`BoundaryDigest` describes a bounded byte window adjacent to—but explicitly
outside—the mediated unit using length and digest, not retained source text. It
is relocation evidence only. It must never enlarge claim support or affect the
unit fingerprint. `OccurrenceContext` records what made the match unique at
capture time (for example, occurrence count and bounded left/right digests) so
reconciliation does not silently choose the first copy.

Existing `ByteRange` records retain their historical positional semantics. New
captures opt into the versioned selector only after the experiment passes.

## Reconciliation algorithm

For a relocatable exact-text region:

1. Read the current file and preserve its exact container fingerprint.
2. Try the original byte range first. If its raw fingerprint matches, use the
   existing fast path.
3. If the original range differs, scan same-length, UTF-8-boundary candidate
   windows in the current file and retain only those matching the recorded raw
   fingerprint. Use a bounded rolling-hash prefilter if measurement requires it.
4. Filter candidates using the bounded before/after anchors and recorded
   occurrence context.
5. Accept relocation only when one candidate is uniquely justified.
6. Apply the observation's already-recorded normalizer to that exact relocated
   unit and compare the existing input fingerprint.
7. Include the resolved current range and relocation outcome in the
   reconciliation fingerprint and reason.

Suggested verdicts:

- **current** — original range still matches, or one exact unit was uniquely
  relocated and its recorded normalized identity matches;
- **stale** — the location is uniquely identified but the selected unit changed,
  or the supporting file disappeared;
- **unknown** — the unit cannot be found safely, multiple candidates remain,
  required relocation material is unavailable, or verification fails.

The difficult `stale` case is a changed unit: exact-text search alone cannot find
an edited region. V1 may conservatively return `unknown` unless boundary anchors
uniquely identify the gap where the unit used to be. A later version can add
structural hints, but must not guess identity from proximity alone.

## Optional structural hints

A provider may attach a hint such as:

```text
language: rust
kind: function
symbol_path: svg::chart
provider: tree-sitter-rust@<version>
```

This improves navigation, stale explanations, and relocation after the unit
itself changes. It does not change mediated coverage and is never sufficient by
itself for a `current` verdict. If the parser is missing, its version differs,
the file does not parse, or the symbol is ambiguous, reconciliation falls back
to exact-text relocation or returns `unknown`.

This boundary is important because the previous structural-freshness experiment
was rejected. Its CST relevance projection introduced per-grammar soundness
risk and recovered fewer formatter rewrites than formatter canonicalization.
This proposal does not use a CST as relevance identity, omit syntax, or claim
semantic equivalence. It uses exact text to survive coordinate drift; existing
normalizers continue handling formatting equivalence.

## Safety and scope invariants

1. **No unseen support.** Anchors and enclosing symbols relocate; they do not
   become claim evidence.
2. **No first-match policy.** Duplicate exact units without decisive anchors
   yield `unknown`.
3. **No fail-open parser path.** Structural hints can narrow candidates but
   cannot independently produce `current`.
4. **No transaction weakening.** Candidate state, validation evidence,
   mutation application, and rollback remain byte-exact.
5. **Versioned replay.** Historical `WholeFile`/`ByteRange` and normalizer
   records retain their original meaning.
6. **Bounded cost.** Anchor size, candidate count, search bytes, and projected
   explanation are capped with explicit failure/omission behavior.
7. **Same freshness core.** Observations, claims, findings, and working-set
   locations continue sharing one location-freshness implementation.

## Relationship to diff-on-stale

Relocation and diff-on-stale solve different parts of the cost:

- relocation avoids a stale verdict when the exact support merely moved;
- diff-on-stale makes a legitimate stale verdict cheap to investigate.

Neither should block the other. Diff-on-stale is likely the cheaper immediate
product improvement. Relocatable selectors are worthwhile only if measured
coordinate-drift cases are common enough and unique relocation succeeds often
without ambiguity.

## Falsifiable experiment

Build a kernel-only spike over the existing read-selection and reconcile path.
Use checked-in UTF-8 fixtures covering Rust, TypeScript, Markdown, TOML, and
plain text:

- insertions/deletions strictly before and after a selected window;
- the same exact unit moved within a file;
- duplicate functions, repeated paragraphs, and repeated one-line reads;
- changes inside the selected unit with stable boundary anchors;
- changes to anchors only;
- range movement across multibyte UTF-8 text;
- whole-file, zero-length, cut-token, and end-of-file windows;
- missing files and capture/reconcile drift; and
- files large enough to exercise search bounds.

Advance only if:

1. there are zero false-current results in adversarial duplicate/edited cases;
2. unchanged moved regions become current when uniquely identifiable;
3. ambiguity and unavailable evidence produce unknown, never guessed current;
4. mediated coverage never expands beyond the original model-visible unit;
5. old event logs replay unchanged;
6. observation/finding freshness remains identical for the same location;
7. reconciliation latency and event-size deltas stay within explicit bounds;
   and
8. transaction and evidence acceptance remain byte-exact.

Report unique-relocation rate, ambiguous rate, stale→current recovery rate,
latency by file size, persisted-byte overhead, and the context cost of projected
relocation reasons. If exact units are usually too short or repetitive to
relocate uniquely, reject the feature or restrict it to sufficiently strong
captures rather than adding increasingly semantic heuristics.

## Open questions

- What minimum unit length or entropy should permit relocation search?
- Which bounded candidate scan or rolling-hash prefilter gives acceptable
  latency without treating a non-cryptographic hash as identity authority?
- Should `stale` require a uniquely relocated changed region, leaving all other
  failures `unknown`?
- Should a successful relocation append a reconciliation event carrying the new
  range, or remain a derived projection from the original selector?
- How should Git rename information interact with path identity? This proposal
  addresses movement within one file only.

## Recommendation

Do not begin with a universal symbol abstraction. First measure a narrowly safe,
language-independent exact-text relocation scheme over existing partial reads.
Add structural providers later only as optional relocation/navigation hints with
explicit versioning and exact-text fallback. Pair the work with diff-on-stale so
the system both avoids coordinate-only invalidation and lowers the cost of
legitimate re-verification.

# Related Concepts
- [Second foreign dogfood — write-loop papercuts and their fixes](../evaluations/plot-foreign-dogfood-write-loop.md): Turns the field report's sub-file selector idea into a bounded exact-text relocation experiment.
- [Structural freshness without formatter coupling](structural-freshness-without-formatter-coupling.md): Preserves the rejected CST experiment's lessons while separating coordinate relocation from relevance identity.
- [Sequence freshness-cost work as diff-on-stale, then drift instrumentation, then relocatable selectors](../decisions/freshness-cost-sequencing.md): Deferred and re-sequenced by this decision; build diff-on-stale and drift instrumentation before any relocation spike.
