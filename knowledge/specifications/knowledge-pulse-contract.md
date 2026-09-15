---
type: Specification
title: Knowledge pulse contract
description: Defines thin knowledge bindings (headline, optional detail, at most one pinned reference), explicit repository/path applicability, current/changed/unavailable/unknown source state, and the bounded status/delta pulse, with fail-closed writes and executable scenarios.
tags: [knowledge, bindings, continuity, wake, contract]
generated: { by: claude-code/claude-opus-5, at: 2026-09-15T16:34:48Z }
---

# Knowledge Pulse Contract

**Status:** normative contract for the `knowledge-continuity` charter
(`knowledge-pulse-contract` action). It extends the
[workspace MVP contract](executable-contract.md) and the
[contextual coordination contract](contextual-coordination-contract.md); both
continue to govern claims, evidence, transactions, and worktree context. It
selects one concrete shape from the
[durable-constraints decision](../decisions/durable-constraints-portable-intent-entity.md)
and supersedes that decision's field list where they differ.

## 0. Why, in evidence

- **Salience, not retention.** The 2026-09-15 cold Claude probe in `plot` found
  the governing decision (`decisions/okf-curated-knowledge-layer`) and placed
  its belief correctly, but only after 18 turns, 15 tool calls, 82.5 s, and
  $0.672 of archaeology. Wake carried `plot` state but not the decision.
- **The category error is already in use.** `plot` claim 6 is a durable
  operating rule ("orientation reflex…") written as a *claim*. Its author had to
  rest it on `.claude/settings.json` because resting it on `README.md` made it
  false-stale on an unrelated edit. A rule does not become false when code
  changes; a claim cannot express that. This contract gives such records a home
  with the right truth semantics.

## 1. Vocabulary and identities

- **Knowledge binding** — a durable, project-scoped, kernel-owned record that a
  piece of knowledge governs work here. One entity; "constraint" and "knowledge
  binding" in earlier documents both name it. Fields:
  - `headline` — required, terse. Always shown at wake.
  - `detail` — optional, short. Carries the rationale only when there is no
    reference.
  - `reference` — optional, **at most one** (see §1.2).
  - `scope` — `repository` (default) or `paths` (§1.3).
  - lifecycle — `active` → `superseded` (by a new binding) | `retired` (with a
    reason). Never amended in place, never freshness-tracked.
- **Authority** — derived, never declared:
  - `workspace` when there is no reference. The headline and detail *are* the
    record (bare-repository case).
  - `reference` when a reference exists. The referenced source is canonical;
    the headline is a wake pointer the binder wrote, not a quotation.

### 1.1 Why one reference, not many

Two references create an unanswerable question: which one is canonical, and
what does it mean when only one changes? If knowledge really lives in two
places, that is two bindings. More reference kinds and multiplicity are
deferred until a scenario needs them (§9).

### 1.2 Reference kinds

- **`repository_file { repository?, path }`** — a whole file in a Git
  repository.
  - With `repository` omitted, the file is in this project. `path` is
    repository-relative and passes the same containment, workspace-internal, and
    sensitive-path rules as reads (`validate_relative_path`,
    `is_sensitive_repository_path`).
  - With `repository` present, the file is in another local Git repository.
    `repository` is a locator as the binder wrote it (e.g. `../agent-workspace`),
    resolved against **this project's repository root, never the process working
    directory**. The kernel records that repository's canonical Git
    common-directory identity at establish time.
  - An OKF concept is simply a repository file (`knowledge/decisions/….md`).
    There is no OKF-specific or Clearhead-specific kind: Git owns bytes, and a
    provider plugin surface is not needed to pin a file.
- **`opaque { locator }`** — a string such as a URL or a tracker id. It is stored
  verbatim, never resolved, and its source state is always `unknown`.

### 1.3 Applicability

A binding is **applicable** when:

- `scope = repository`: always; or
- `scope = paths [prefix…]`: some prefix matches a path in the kernel's own
  records of current work. Those records are active claim inputs, the working
  set, and open transaction mutations. A prefix is an exact repository-relative
  file, or a directory prefix ending in `/`. No globs, no semantic matching.

Every served binding carries `why`, naming the rule that selected it:
`repository`, or `path <p> via claim <id>` / `via working set` /
`via transaction <id>` (first match in that order).

### 1.4 Source pin and source state

At establish time a `repository_file` binding records its **pin**:

- the owning repository's identity (§1.2);
- `HEAD` of the owning repository;
- the SHA-256 of the file's raw bytes. There is no normalizer, because prose has
  no canonical formatter and any byte change is a change.

**Source state** is a report distinct from claim freshness, and is deliberately
never called `stale`:

| State | Meaning |
| --- | --- |
| `current` | The resolved file's bytes match the pin. |
| `changed` | The file resolves and its bytes differ. The pinned revision is served so `git diff <rev> -- <path>` explains it natively. |
| `unavailable` | The locator does not resolve, is not a Git repository, has a different identity from the pin, or the file is missing or unreadable. A bounded native reason is retained. |
| `unknown` | Opaque reference, or no coherent read could be established. |

`workspace`-authority bindings have no source state; the field is absent.

## 2. Invariants

1. **Bindings are intent, not beliefs.** No code, revision, or worktree change
   alters a binding's lifecycle. Source state is a separate report, and no
   surface renders it as claim freshness.
2. **The canonical source wins.** The kernel stores only the pin, never any part
   of the source body. A `reference`-authority headline is always served
   alongside its authority, reference, and source state, never bare.
3. **Source state is served only after assessment.** Every surface that serves
   source state assesses the binding first:
   - a this-project file is assessed against the querying worktree (contextual
     contract §2);
   - a foreign file is assessed against that repository's checkout, named in
     the reason.
   Another worktree's or an earlier session's assessment is never served in
   place of a new one.
4. **Assessments are events when they change.** A
   `KnowledgeSourceAssessed { binding_id, state, fingerprint?, reason,
   worktree_identity }` event is appended only when the state differs from this
   worktree's last assessment. No-op suppression works as it does for claims.
   This lets delta diff source state against a checkpoint baseline by replay,
   as it already does for claim freshness.
5. **No silent retargeting.** A moved, renamed, or deleted file, or a locator
   that now names a different repository, becomes `unavailable`. It is never
   followed to a guessed new location.
6. **Relevance is explicit.** Nothing is served without a §1.3 match. Headline,
   detail, intent text, and source content are never used for selection.
7. **Thinness is enforced when writing.** Over-bound fields are rejected, not
   truncated, so the binder learns the limit (§3). Wake projections truncate
   anyway.
8. **One kernel policy.** CLI and MCP serve identical knowledge JSON for the
   same state. Adapters do not filter, rank, re-word, or cache bindings.
9. **Providers are optional.** Status and delta succeed without OKF, Clearhead,
   a network, notifications, or the foreign repository. Absence degrades only
   to `unavailable` or `unknown`.

## 3. Bounds

**Rejected at write** if exceeded:

| Field | Limit |
| --- | --- |
| `headline` | 1–120 chars |
| `detail` | 400 chars |
| `path`, `repository`, `opaque.locator` | 256 chars each |
| `paths` scope | 1–8 prefixes |

**Brief `status`** adds:

- `knowledge`: at most **3** applicable bindings, ranked (§4);
- `knowledge_omitted`: applicable bindings not shown;
- `counts.active_bindings`: all active bindings, applicable or not.
  `active_bindings − shown − omitted` is therefore the number filtered out by
  scope, which keeps filtering visible.

Each entry has these fields:

| Field | Content |
| --- | --- |
| `id` | binding id |
| `headline` | up to 100 chars |
| `authority` | `workspace` or `reference` |
| `reference` | display form, up to 120 chars, e.g. `../agent-workspace:knowledge/decisions/okf-curated-knowledge-layer.md` |
| `source` | source state; absent for `workspace` authority |
| `why` | up to 60 chars |

**Brief `delta`** adds three `BriefIdSet`s:

- `knowledge_established`;
- `knowledge_ended` (superseded or retired);
- `knowledge_source_changed`: active bindings whose served source state differs
  from their baseline state at the checkpoint, in any direction, including into
  or out of `unavailable`.

**`--full`** serves every active binding with detail, full reference, pin,
scope, and the latest assessment reason. Ended bindings are served with their
successor or retirement reason.

**Wake byte budget.** The compact brief status must stay under the existing
1800-byte inline-preview budget for this worst case: 5 max-length claims, the
maximum intent, the maximum checkpoint note, and 3 max-length bindings.

> **Known pressure — owner review requested.** Three bindings add roughly
> 1 KB to a status already near budget. If field compaction cannot fit this
> worst case, the claim cap drops from 5 to 4 **before** the knowledge cap
> drops below 3. The baseline failure was a missing governing decision, and
> claims beyond the window stay one `--full` away with `claims_omitted`.

## 4. Ranking

The ranking is a deterministic total order over applicable bindings:

1. source state: `changed`, `unavailable`, `unknown`, then `current` or none.
   This is the same "invalidated first" rule claims use, so a cap never hides a
   drifted source.
2. scope: `paths` matches before `repository`, because the more specific scope
   ranks first;
3. newest establish sequence first.

## 5. Operations

Exactly two new verbs, available in both CLI and MCP:

- **`bind-knowledge` / `workspace_bind_knowledge`**
  - Inputs: `headline`, `detail?`, `reference?`, `paths?`, `supersedes?`.
  - Establishes a binding, pins its source, and records the first assessment.
    With `supersedes`, the named active binding ends atomically in the same
    event.
  - Returns the compact receipt `{ id, authority, source? }`.
  - **Re-affirming after a source change** is `bind-knowledge` superseding the
    old binding with the same reference, which re-pins it. No separate
    acknowledge verb exists.
- **`retire-knowledge` / `workspace_retire_knowledge`**
  - Inputs: `id`, `reason`.

**Rejected alternative:** generalizing `supersede_claim` and `retire_claim`
across entity kinds. The id namespaces collide, and one verb over two truth
semantics is the conflation invariant 1 forbids.

**Fail-closed writes** reject, with a named reason, when:

- a bound in §3 is violated, or the headline is blank;
- a `repository_file` path is absolute, escapes its repository, is
  workspace-internal, or is sensitive;
- a `repository_file` path is missing or not a regular file at establish time
  (use `opaque` to point at something that cannot be pinned);
- a `repository` locator does not resolve to a Git repository;
- a `paths` prefix is absolute or escapes the repository;
- `supersedes` names an unknown or inactive binding;
- another active binding, other than the one being superseded, already
  references the same resolved source (repository identity + path). The error
  names the existing id. This is the kernel-level guard against duplicate
  knowledge.
- `retire-knowledge` names an unknown or inactive binding.

## 6. Replay, compatibility, privacy

- New events: `KnowledgeBound` (carries `supersedes?`), `KnowledgeRetired`,
  `KnowledgeSourceAssessed`.
- Journals without them replay with zero bindings. Brief fields are additive and
  always present (`knowledge: []`, `knowledge_omitted: 0`), and delta against a
  pre-binding checkpoint works.
- `KnowledgeSourceAssessed` events without a worktree identity are never served
  (contextual contract §2).
- Bindings live in the project's external state, shared across linked
  worktrees. Foreign locators are local paths and never enter Git.
- No source body, excerpt, or front-matter is read into durable state.
  Assessment hashes bytes and discards them.

## 7. Prohibited failures

- **K1 — false authority:** a `reference`-authority headline served without its
  authority and source state, or source text copied into workspace state.
- **K2 — silent source drift:** a `changed` or `unavailable` source served as
  `current`, or source state inherited without assessment in the querying
  context.
- **K3 — truth conflation:** a code or revision change supersedes, retires, or
  stales a binding, or source state is rendered as claim freshness.
- **K4 — invented relevance:** a binding served without a §1.3 match, or with a
  `why` that does not name the actual match.
- **K5 — silent retarget:** a moved or renamed source, or a re-pointed locator,
  followed automatically.
- **K6 — hidden or unbounded wake:** entries dropped without
  `knowledge_omitted`, or source content on the default path.
- **K7 — required provider:** status or delta fails, or omits a binding, because
  an optional provider or foreign repository is absent.
- **K8 — duplicate binding:** two active bindings to one resolved source.
- **K9 — adapter policy:** an adapter changes which bindings appear, their
  order, or their wording.

## 8. Executable scenarios

Fixtures are temporary Git repositories. "Foreign" means a sibling repository
reached by a relative locator.

**KP1 — wake pulse, no recap.**
- *Given* project P with a repository-scope binding whose reference is foreign
  file `F` (`../kb:decisions/boundary.md`),
- *when* a new process requests compact brief status for P,
- *then* `knowledge[0]` carries the headline, `authority = reference`, the
  display reference, `source = current`, and `why = repository`, with no other
  call required.

**KP2 — progressive disclosure.**
- *Given* KP1,
- *then* the brief entry carries no `detail` and no bytes of `F`;
- `status --full` carries the pin revision and fingerprint;
- the display reference resolves to a readable file through ordinary filesystem
  access.

**KP3 — source revision.**
- *Given* KP1 and checkpoint `c0`,
- *when* `F` is edited and committed in the foreign repository,
- *then* status serves `source = changed` with the pinned revision, and
  `delta --since c0` lists the id in `knowledge_source_changed`;
- the binding stays active with an unchanged headline;
- *when* the binder supersedes it with the same reference,
- *then* the new binding is `current`, and delta lists the new id as
  established and the old id as ended.

**KP4 — unavailable provider.**
- *Given* KP1 and checkpoint `c0`,
- *when* the foreign repository is moved away,
- *then* status succeeds and still serves the headline and reference with
  `source = unavailable` and a reason, and delta lists a source change;
- *when* it is restored, the source is `current` again.
- **Variant:** a different repository at the same locator is also `unavailable`
  (identity mismatch), never `current`.

**KP5 — bare repository.**
- *Given* a repository with no `knowledge/`, no `.clearhead/`, and no foreign
  locator,
- *when* a headline-plus-detail binding is established over MCP,
- *then* brief status over MCP and over CLI both serve `authority = workspace`
  with no `source` field.

**KP6 — false-relevance guard.**
- *Given* a binding scoped to `src/legend.rs` and no claim, working set, or
  transaction touching it,
- *then* brief status omits it without counting it in `knowledge_omitted`, while
  `counts.active_bindings` includes it;
- *when* a claim citing `src/legend.rs` is recorded, it appears with
  `why = path src/legend.rs via claim <id>`;
- *when* that claim is retired, it disappears.

**KP7 — ranking and omission.**
- *Given* five applicable bindings: one with a changed source, one path-matched,
  three repository-scoped,
- *then* brief status shows exactly three, in the order changed, path-matched,
  newest repository-scoped, with `knowledge_omitted = 2`, identically across
  repeated runs.

**KP8 — lifecycle and duplicates.**
- Superseding to a different reference removes the old binding from brief status
  and keeps it in `--full` with its successor.
- Retire keeps its reason.
- Retiring or superseding an unknown or inactive id is rejected.
- Establishing a second active binding to the same resolved source is rejected,
  naming the existing id.

**KP9 — code change does not touch bindings.**
- *Given* a `workspace`-authority binding and a `reference` binding to a
  this-project file,
- *when* unrelated source files change and `HEAD` advances,
- *then* both stay active, the first has no source state, and the second stays
  `current`.
- This is the inverse of `plot` claim 6's false-stale.

**KP10 — worktree context.**
- *Given* a this-project reference and two linked worktrees,
- *when* the referenced file is edited only in worktree B,
- *then* A serves `current` and B serves `changed`, and neither is served from
  the other's assessment.

**KP11 — legacy replay.**
- *Given* a journal fixture with no knowledge events and a checkpoint,
- *then* status serves `knowledge: []` and `knowledge_omitted: 0`, and delta
  since that checkpoint succeeds with empty knowledge sets.

**KP12 — surface parity.**
- *Given* one state,
- *then* the knowledge fields of compact brief status and delta are identical
  JSON over CLI and MCP.

**KP13 — fail-closed writes.** Each §5 rejection case is a table-driven case
asserting a named error and no appended event.

**KP14 — wake budget.** The §3 worst case fits under 1800 bytes.

## 9. Deliberately out of scope

Each item has its reopen trigger.

| Out of scope | Reopen when |
| --- | --- |
| Provider enrichment, e.g. reading an OKF `description` as the headline | Dogfood shows binder-written headlines drifting from their sources. |
| Heading or byte-range references, multiple references per binding | Whole-file pins produce `changed` noise that dogfood can score as alarm fatigue. |
| Intent-scoped bindings (the decision's deferred `scope` attribute) | A temporary objective-only rule appears in practice. |
| Semantic relevance, automatic promotion from checkpoint notes or claims, notifications | Only through a new evidence-backed decision. |
| Assessing `Intent.external_reference` with the same pin machinery | A separate slice; noted because it is the obvious next reuse. |

Converting `plot` claim 6 into a binding is a dogfood step, not a kernel
migration.

## 10. Acceptance

- **Tier A (closes `knowledge-pulse-kernel`):** KP1–KP14 pass, and K1–K9 are
  never observed over the fixture suite.
- **Tier B (`knowledge-pulse-dogfood`):** the predeclared `plot` experiment
  against the 2026-09-15 baseline. The baseline prompt supplied the principle
  itself, so the Tier B prompt must not, or it tests destination discovery
  again rather than inherited relevance.

## Related concepts

- [Durable constraints are a portable intent entity](../decisions/durable-constraints-portable-intent-entity.md): The design direction this contract makes concrete; §1 selects its entity shape.
- [Use OKF for curated project knowledge](../decisions/okf-curated-knowledge-layer.md): The authority boundary a `reference` binding preserves.
- [Contextual coordination contract](contextual-coordination-contract.md): Worktree-relative assessment rules source state inherits.
- [Selection disposition — linked, not built-in](../decisions/coordination-selection-disposition.md): The link principle; references are links, never dependencies.
