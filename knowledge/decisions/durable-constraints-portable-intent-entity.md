---
type: Decision
title: Durable constraints are a portable intent entity, distinct from claims
description: Defines thin repo-scoped constraint and knowledge bindings; the 2026-09-15 cold probe fired the friction gate for contract work while preserving external canonical knowledge and a bare-repository fallback.
tags: [architecture, constraints, intent, portability, continuation]
generated: { by: agent/cli, at: 2026-09-15T09:41:32Z }
---

# Decision — durable constraints are a portable intent entity, distinct from claims

**Status:** revised 2026-09-15. The cheap prerequisite — a bounded
checkpoint-note excerpt in brief status — **shipped 2026-09-07**
(`BriefCheckpoint.note`, a 200-char word-boundary excerpt via
`BriefCheckpoint::from_marker`, surfaced in both status and delta; regression +
budget tests in `tests/walking_skeleton.rs`). The 2026-09-15 cold Claude probe
showed that retention still required expensive repository archaeology, so the
friction gate has fired. The owner authorized the active
`knowledge-continuity` charter and its first action,
`knowledge-pulse-contract`; that executable contract remains the gate before
implementation and may revise the entity shape below.

**Date:** 2026-09-07

**Participants:** user + assistant, reacting to the `plot` continuation-insight
field report ([plot-continuation-insight](../evaluations/plot-continuation-insight.md)).

## Context

The foreign `plot` dogfood surfaced a **salience**, not retention, failure. A
prior decision — "axis label formatting = SI suffixes" — was recorded in a
checkpoint note and never lost, but it was not promoted into the bounded wake
surface, so a resuming agent re-derived the wrong interpretation (generic
scientific notation) until an expensive full status recovered it. The system is
now credible at capturing facts, establishing beliefs, and explaining change; the
next bottleneck is preserving decisions and surfacing what is decision-relevant
for the *next* action.

That report proposes a third bounded surface (a continuation capsule) and,
optionally, structured handoff fields on checkpoints. This decision resolves the
prior question it raises: what class of thing carries a decision, and where does
it live, given the workspace must stay portable into a bare repository.

## Decision

### Constraints are intent, not beliefs about the artifact

There are two distinct classes of durable state, separated by their truth
semantics:

- **Claims** — beliefs about the artifact, cited to `file@revision`,
  fingerprinted, freshness-tracked. They answer "is this still true?" and go
  stale when code changes.
- **Constraints / decisions** — records of *intent* ("bar + log scale is
  invalid", "axis = SI suffixes", "x-log deferred, x may be temporal"). They are
  "ever true" (a historical agreement), not "still true"; they do **not** go
  stale when code changes and carry no fingerprint.

The freshness dimension keys off the presence of a code citation, so this
distinction is intrinsic to the data rather than something enforced by a parallel
verb family. Constraints therefore need no `amend`; their lifecycle is
establish → supersede (reverse) / retire (lift), reusing the existing lifecycle
verbs. Verb accretion is avoided.

### Constraints are repo-scoped and higher-order than the objective

Constraints attach to the **repository**, not to an objective. The objective is
the transient thing; a constraint is the durable backdrop that outlives many
objectives (architectural invariants and domain decisions are never
sprint-scoped). Objectives (and the next-actions under them) may *reference* the
constraints that bind them; constraints do not depend on any objective. This
makes the reframe test trivial: rebinding the objective cannot disturb a
constraint. This generalizes the existing intent layer — the workspace
`Objective` binding — from a singleton to `objective + a set of independently
lifecycled constraints`, and it fills the "decisions, risks" workstream slot the
[external-workspace-and-clearhead-boundary](external-workspace-and-clearhead-boundary.md)
scope table already anticipated.

### Portability is the governing constraint: the kernel is the floor, not a wrapper

The tool must work when dropped into a bare repository with neither Clearhead nor
OKF installed. The kernel therefore holds a usable thin binding natively;
external tools are optional ceilings it can defer to, never dependencies. This is
how the objective binding already behaves: an intent string that stands alone,
plus an optional `external_reference`.

The contract starts from three fields and lets executable scenarios pull any
others:

- **headline** — required and terse; the binding line the bounded wake pulse shows;
- **detail** — optional, short inline rationale for the bare-repository case;
- **references** — optional opaque/native links to an OKF decision, ordinary
  documentation, a Clearhead item, or another authority.

In a bare repository, the headline and optional detail are the authoritative
record. When a curated source is referenced, that source is canonical and the
workspace owns only the situated relationship to it: why it applies, which source
version was relied upon, and whether that source changed or became unavailable.
OKF and Clearhead cannot be required, but neither may a cached workspace excerpt
silently compete with their referenced canonical text.

## Rationale

- **Portability forbids reference-only ownership.** An earlier draft leaned on
  "reference the authoritative tool"; a bare repo has no such tool, so the kernel
  must own enough to stand alone.
- **The antidote to becoming a generic wrapper is thinness, not abstinence.**
  Native ownership is required, so the discipline is to keep the native version
  deliberately minimal — a headline plus a sentence, never a rationale essay that
  competes with OKF, never a plan that competes with Clearhead. The reference is
  the "want more? go there" escape hatch. This honors the boundary's
  "sibling authorities, not a wrapper" rule while staying portable.
- **The intent/claim split already exists.** The objective is a freshness-exempt
  intent entity; constraints are the same class generalized. We are fleshing out
  a layer, not inventing a category.
- **Duplication is explicit and asymmetric.** In standalone mode there is one
  home and no drift. With a reference, the workspace headline is a bounded wake
  aid and the source remains canonical. The pulse must identify provider,
  reference, observed source version when available, and honest
  current/changed/unavailable/unknown state; it must not present a cached excerpt
  as independently authoritative. Git-native sources can support stronger change
  assessment than opaque external systems, whose unavailable or unversioned state
  must remain explicit.

## Consequences

- **Culling by lazy selection, not release-conditions.** Repo-scoped constraints
  are the one entity with neither a freshness nor a completion signal, so the
  store can accrete. Rather than couple a constraint's expiry to a claim's
  freshness (which re-entangles the layers we deliberately separated), the
  continuation capsule surfaces a constraint only when it binds a *pending*
  next-action; obsolete constraints fall off the working surface on their own, and
  store hygiene becomes an occasional chore, not a correctness requirement.
- **Immediate cheap step, shipped independently:** include a bounded excerpt of
  the latest checkpoint note in brief `workspace_status`. Status already carries
  the checkpoint label/sequence; adding a bounded note slice would have prevented
  the observed miss and does not require the constraint entity.
- **Binding design is now active, contract-first.** The
  `knowledge-pulse-contract` action defines how a constraint attaches to current
  work, how explicit applicability is ranked and bounded, and how `status` and
  `delta` project source transitions. Implementation must wait for those
  scenarios rather than freezing this prose as a schema.
- **Deferred — scope attribute.** A genuinely temporary "for this objective only"
  constraint has no home under strict repo-scoping. A `scope` attribute (default
  repo, optional objective-bound) covers it, but is not built until the case
  actually appears.
- **No retrospective inference.** Constraints are authored at write time, never
  inferred from arbitrary prose; a renderer may rank and omit with a count, but
  must not invent a decision that was never recorded.

## 2026-09-15 friction evidence

An isolated fresh Claude Code session started in a detached `plot` worktree and
was given the owner-confirmed principle that a cold successor should inherit
accepted project context without requiring the human to re-teach it. The prompt
did not name OKF, Clearhead, Agent Workspace as the destination, or a memory
plugin. Claude correctly followed `plot/README.md` to the sibling project,
consulted its instructions, ran six OKF searches, found the existing
`okf-curated-knowledge-layer` decision at commit `fcde3c0`, made no duplicate
write, and explained the authority boundary correctly.

The behavior passed placement and no-duplication, but not cheap inheritance: 18
turns, 15 tool calls, 82.5 seconds, and $0.672. Workspace wake supplied situated
`plot` state but did not pulse the governing curated decision. The prompt also
supplied the principle itself, so the run tested destination discovery rather
than independent relevance discovery. This is enough to authorize bounded
contract work, not to claim the eventual model or implementation succeeds.

## Related concepts

- [From change explanation to continuation insight](../evaluations/plot-continuation-insight.md): The field report whose salience finding motivated this entity.
- [External workspace state and the Clearhead boundary](external-workspace-and-clearhead-boundary.md): Establishes objective-binding-with-optional-reference and the sibling-authority rule this constraint entity mirrors.
- [Use OKF for curated project knowledge](okf-curated-knowledge-layer.md): The authoritative home for full decision rationale a constraint may reference.
- [S7 bounded perception](../design/s7-bounded-perception.md): The bounded continuation surface that selects and renders these constraints at wake.
