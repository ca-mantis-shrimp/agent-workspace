# Structural freshness without formatter coupling

## Abstract

Agent Workspace currently decides whether a source observation remains fresh by
fingerprinting selected bytes, optionally after running a language formatter as
a canonicalizer. This prevents layout-only edits from producing false-stale
claims, but it gives freshness an accidental dependency on formatter discovery,
configuration, availability, and version determinism. Extending the mechanism
from Rust to TypeScript would make the kernel—or worse, an adapter—know how to
invoke Prettier even though formatting is neither the workspace's authority nor
the property freshness intends to measure.

This paper proposes separating four identities that byte fingerprints currently
partly conflate: capture identity, structural location identity, relevance
identity, and candidate identity. For successfully parsed source code,
freshness should be determined by a versioned, kernel-owned tree-sitter
projection. Exact byte identity remains at provenance and transaction
boundaries, but byte inequality no longer makes a structural code observation
stale. Formatters disappear from freshness configuration and adapters remain
transport-only. The proposal is deliberately conservative: its first identity
is trivia-insensitive concrete syntax, not general semantic equivalence. A
falsifiable Rust and TypeScript experiment must establish soundness and useful
coverage before it replaces the shipped normalizer path.

## 1. Research question

How should Agent Workspace determine that an agent's source-code belief may be
reused after the repository changes, without:

1. reporting `current` after a relevant in-scope change;
2. reporting `stale` for ordinary formatting reflow;
3. depending on rustfmt, Prettier, Black, or another formatter at runtime;
4. moving language policy into Pi, Claude Code, Neovim, or other adapters; or
5. weakening exact mutation, rollback, and validation guarantees?

The question is specifically about **freshness of source observations and their
claims**. It is not about keeping the worktree formatted, producing clean diffs,
or proving two arbitrary programs semantically equivalent.

## 2. Existing mechanism and the category error

The current kernel records an `ObservationSelector` and fingerprints the
selected bytes. `Normalizer::Rustfmt` can transform those bytes before hashing,
and `.agent-workspace/normalizers.toml` now selects a registered normalizer by
extension. Reconciliation repeats the recorded normalization scheme and compares
the result. A raw fingerprint provides a fast path when the selected bytes have
not changed.

This solved a real trust problem: formatter reflow repeatedly made valid claims
look stale. It also exposed the wrong abstraction. A formatter answers:

> How should this tool render this program under this configuration and version?

Freshness needs to answer:

> Did the part of the program on which this observation relied change in a way
> represented by the observation's declared relevance policy?

Formatter output is an indirect and environment-sensitive approximation to
that question. Adding Prettier would require invocation logic, package discovery,
configuration discovery, version pinning, missing-tool behavior, and
cross-environment determinism. None belongs in an adapter, and most does not
belong in a language-independent coordination kernel.

The opposite overcorrection is also unsafe: a generic "semantic AST hash" may
drop punctuation, comments, directives, macro material, or whitespace whose
significance varies by language. Agent Workspace's failure model makes
false-current more dangerous than conservative false-stale. Structural identity
must therefore state and version its equivalence relation instead of calling
itself semantic.

## 3. Thesis: separate four identities

One fingerprint should not carry four different promises.

<!-- markdownlint-disable MD013 -->

| Identity | Question answered | Initial representation |
| --- | --- | --- |
| Capture identity | What exact source did the native read expose, and did it drift during capture? | Exact digest of model-visible source and its container provenance |
| Location identity | Where is the observed code after surrounding edits? | Versioned tree-sitter structural anchor |
| Relevance identity | Did the observed code change under the declared freshness policy? | Versioned CST projection digest |
| Candidate identity | What exact worktree bytes were mutated, tested, accepted, or reverted? | Exact Git/content fingerprints |

<!-- markdownlint-enable MD013 -->

The identities may be derived from the same file snapshot, but they have
different semantics and failure behavior.

A formatting-only edit changes capture and candidate identity. It need not
change structural location or relevance identity. Therefore it need not stale a
structural code claim, while evidence recorded against an earlier exact
candidate still cannot silently attest a later candidate.

## 4. Authority and component boundaries

### 4.1 Adapters

Adapters remain transport-only. A read adapter supplies:

- repository and path;
- native provider identity;
- the exact model-visible source window after removing adapter-specific chrome;
- line-window metadata when the native tool exposes it; and
- complete model-visible accounting required by the existing capture contract.

An adapter does not parse, normalize, invoke a formatter, choose a grammar, or
decide freshness.

### 4.2 Kernel policy

The kernel chooses an input identity scheme at capture time and persists the
resolved scheme on the observation. A minimal model is:

```rust
enum InputIdentityScheme {
    ExactTextV1,
    TreeSitterCstV1 {
        provider: ProviderIdentity,
        grammar: GrammarIdentity,
        projection_version: u32,
    },
}
```

Persisting the resolved scheme preserves the current event-sourced,
forward-only property: old byte and formatter-normalized observations retain
their recorded meaning; new configuration never retroactively reinterprets
them.

### 4.3 Structural provider

The kernel calls a narrow structural-provider interface:

```rust
trait StructuralProvider {
    fn capture(
        &self,
        language: LanguageId,
        source: &[u8],
        visible_range: Range<usize>,
    ) -> Result<StructuralObservation, StructuralFailure>;

    fn reconcile(
        &self,
        source: &[u8],
        recorded: &StructuralObservation,
    ) -> Result<StructuralMatch, StructuralFailure>;
}
```

The first implementation may compile pinned Rust and TypeScript tree-sitter
grammars into the kernel. A later trusted helper process is possible, but the
provider contract—not an arbitrary command string—remains authoritative.
Repository policy maps file kinds to registered grammar/projection identities;
it never names formatters.

Every structural record stamps:

- provider identity and version;
- language and grammar revision;
- projection scheme and version;
- the selector and relocation anchors;
- the relevance digest; and
- capture/container provenance without claiming unseen bytes were observed.

If the stamped provider cannot be reproduced, the record is not silently
reinterpreted by a newer grammar.

## 5. Conservative structural relevance

### 5.1 First equivalence class

`TreeSitterCstV1` promises **trivia-insensitive concrete-syntax equality**. Its
digest includes:

- full topology of the projected tree or token interval;
- named and anonymous syntax nodes;
- field relationships where the grammar exposes them;
- terminal token contents;
- comments and documentation comments;
- attributes, directives, and macro-relevant syntax; and
- explicit error or missing-node state.

It omits only source gaps that the audited grammar does not represent and that
the language profile classifies as formatting trivia.

This is intentionally more conservative than semantic equivalence. Prettier may
insert optional semicolons, remove redundant parentheses, or alter quote and
escape spelling while preserving runtime behavior. A lossless CST projection
may report those changes as stale. That is acceptable in the first scheme:
false-stale bounds the benefit; dropping significant syntax generically risks
false-current and violates the executable contract.

Broader equivalence, if later justified, belongs in a new versioned,
language-specific projection such as `TypeScriptSyntaxV2`. It must earn its
rules through adversarial evidence rather than through a generic "named nodes
only" shortcut.

### 5.2 Parse errors

Tree-sitter error recovery can produce plausible trees for invalid input.
Capture may fall back to `ExactTextV1` when a new source snapshot contains
`ERROR` or `MISSING` nodes in the observed unit. Once an observation is stamped
structural, a later parse error yields `unknown`, not `current` and not a guessed
text comparison under a different identity scheme.

### 5.3 Unsupported files

New observations without a registered structural provider use `ExactTextV1`.
This preserves useful freshness for prose, data, generated formats, and
unsupported languages. `Unknown` is reserved for an already-recorded identity
that cannot be safely reproduced or relocated.

## 6. Bounded reads and scope honesty

The model often sees an arbitrary line window rather than a whole file or a
complete syntax node. Promoting that window to its enclosing function would
falsely imply that unseen source supported the observation.

A structural capture must represent only model-visible source. The proposed
selector is a visible token interval with structural boundary anchors:

```rust
struct StructuralSelector {
    first_visible_token: TokenAnchor,
    last_visible_token: TokenAnchor,
    projected_coverage: CoverageDigest,
    boundary_mode: BoundaryMode,
}
```

Rules:

1. Whole-file and exact-node observations can project directly.
2. Token-aligned windows project only the visible token interval.
3. Ancestors and sibling context may be relocation evidence but never expand
   claim support.
4. A window cutting through a token uses exact boundary material or falls back
   to `ExactTextV1`; it cannot pretend to have observed the complete token.
5. Non-unique relocation yields `unknown` under invariant 9, never the first
   plausible match.

The usefulness of token-interval relocation is empirical. If ordinary reads
produce excessive ambiguity, structural tools should preferentially capture
symbols while native arbitrary reads retain exact-text freshness. That outcome
would narrow the proposal without invalidating the identity separation.

## 7. Reconciliation algorithm

For a structural observation:

1. Read the current repository file at the reconciliation boundary.
2. Resolve the exact stamped provider, grammar, and projection version.
3. Parse the current file.
4. Reject the structural path if the observed region contains parse errors or
   missing nodes.
5. Relocate the recorded visible unit using its structural anchors.
6. Reject ambiguous or absent relocation.
7. Project exactly the relocated visible coverage.
8. Compare its relevance digest with the recorded digest.
9. Include the resulting material in the existing reconciliation fingerprint.

Verdicts are:

- equal relevance identity: `current`;
- unequal relevance identity: `stale`;
- missing/incompatible provider, parse failure, or ambiguous relocation:
  `unknown`.

Raw content equality remains a valid optimization: if the exact selected source
is unchanged, the recorded structural projection necessarily remains valid.
Raw inequality, however, is no longer itself a stale verdict.

## 8. Transactions and validation remain exact

Structural freshness must not replace transaction candidate identity.
Transactions promise that:

- mutations apply to known before/after contents;
- rollback restores transaction-owned changes without destroying unrelated
  work;
- passing evidence consumed the candidate later accepted; and
- post-validation drift cannot be silently committed as tested.

Those are exact-state claims. The current candidate fingerprint derived from
mutation after-fingerprints and the materialization checks should remain
byte-exact. A formatter running after validation changes the tested candidate;
acceptance should continue to stop until checks run against the new candidate.
The kernel need not know that a formatter caused the change.

A future evidence type could explicitly attest a structural equivalence class,
but that would be a separate, weaker contract requiring check-specific and
language-specific justification. It is not part of this proposal.

## 9. Migration

This design does not require rewriting the event log.

1. Preserve `Normalizer::None` and `Normalizer::Rustfmt` replay for existing
   observations and claims.
2. Add the versioned input-identity scheme with serde defaults mapping old
   records to their historical behavior.
3. Use structural identity only for new supported captures.
4. Stop configurable-normalizer work before registering Prettier or other new
   formatters.
5. Remove formatter-selection configuration only after the structural experiment
   passes and historical replay is covered.
6. Retain old formatter code as a compatibility path until no supported replay
   horizon requires it; do not reinterpret old fingerprints.

## 10. Falsifiable experiment

The next slice is a kernel-only research spike, not an adapter change.

### 10.1 Corpus

Use pinned Rust and TypeScript grammars with checked-in before/after fixtures:

- rustfmt and Prettier whitespace/reflow outputs;
- optional semicolon, parenthesis, trailing-comma, quote, and escape changes;
- identifier, operator, literal, type, and control-flow changes;
- comments, doc comments, `#[cfg]`, safety comments, and TypeScript directives;
- Rust macro invocations and TypeScript ASI/newline-sensitive cases;
- valid-to-invalid and error-recovered syntax;
- duplicate functions or blocks that challenge relocation;
- whole-file, exact-node, token-aligned, and cut-token windows; and
- edits exclusively before and after an observed window.

Formatter-produced fixtures may be generated once and checked in with their
provenance. No formatter is a runtime or adapter dependency.

### 10.2 Acceptance criteria

The structural path advances only if:

1. no adversarial relevant edit retains the same identity;
2. ordinary demonstrated reflows retain identity;
3. formatter transformations outside the declared equivalence class merely
   bound the benefit through false-stale results;
4. no bounded observation gains support from unseen source;
5. parse, version, and relocation ambiguity produce `unknown`;
6. unsupported new captures retain exact-text behavior;
7. historical normalizer records replay unchanged; and
8. post-validation byte drift still blocks transaction acceptance.

A single demonstrated false-current collision blocks adoption of that projection
version. High ambiguity or low formatting coverage does not prove unsoundness,
but may show that the added machinery is not worth replacing byte freshness.

### 10.3 Measurements

Report:

- false-current count across adversarial pairs;
- formatter-pair equality rate, separated by whitespace-only and
  syntax-rewriting transformations;
- `current`, `stale`, and `unknown` rates for representative bounded reads;
- unique relocation rate after before-window and within-container edits;
- capture and reconciliation latency; and
- event-size and context-projection deltas.

## 11. Alternatives considered

### Raw bytes only

Safe but produces repeated false-stale results from layout changes, eroding
trust in the workspace's prime freshness signal.

### Formatter canonicalization

Effective for each supported formatter, but imports formatter invocation,
configuration, availability, and determinism into freshness. It scales by tool
integration rather than by a stable structural contract.

### Generic named-node AST hash

Temptingly small but unsafe. It can omit anonymous punctuation, comments,
directives, and other material without proving irrelevance.

### Full semantic equivalence

Outside the workspace's authority and impractical across languages. It would
require compiler- or interpreter-level semantics and would still not replace
exact transaction identity.

### Formatter provenance events

Knowing that a formatter made an edit does not prove the edit was exclusively
formatting, and raw edits can produce equivalent layout changes. Freshness must
judge the recorded input, not trust the alleged actor.

## 12. Recommendation

Adopt the four-identity architecture as the target model. Pause the Prettier
normalizer slice. Implement the structural experiment before deleting the
shipped normalizer compatibility path.

The key architectural statement is:

> Exact bytes establish provenance and transaction state. A versioned structural
> projection establishes freshness relevance for supported code. Adapters own
> neither decision, and formatters are not freshness providers.

This does not eliminate bytes from the system. It eliminates the mistake of
using byte inequality—and formatter-normalized byte equality—as the primary
meaning of code freshness.

## 13. Council record

A bounded advisory council reviewed the proposal on 2026-09-04. Both advisors
converged after one independent pass, so no cross-examination was needed.

- `oracle`, forked/context-aware, run
  `15a6f1c1-eac2-49bc-9553-b47ba7f001a6`;
- `reviewer`, fresh context, run
  `b2f358c3-9f37-4a65-93c3-1a35e5b6abc0`;
- workflow `8fd49172-a4d2-488e-8e1d-fe3bf4338717`.

No project `council-*` profiles were installed, so both were documented
fallbacks under the council protocol. Their shared recommendation was to adopt
the identity split, keep exact transaction state, reject formatter dependencies
in adapters and freshness, and require the falsifiable structural experiment.

## References

- [`executable-contract.md`](executable-contract.md), especially invariants 1–3,
  8–11 and failures F1, F2, F4, and F9.
- [`design-note-configurable-normalizers.md`](design-note-configurable-normalizers.md),
  documenting the formatter-based path and its determinism problem.
- [`implementation-notes.md`](implementation-notes.md), sections “Normalized
  fingerprinting,” “Auto-normalize default,” and “Candidate-state evidence.”
- [`../src/reconcile.rs`](../src/reconcile.rs), current capture,
  normalization, and reconciliation primitives.
- [`../src/model.rs`](../src/model.rs), observation, claim-input, evidence, and
  transaction identity records.
