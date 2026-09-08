---
type: Decision
title: Durable constraints are a portable intent entity, distinct from claims
description: Models decisions/constraints as a repo-scoped, freshness-exempt intent entity above the objective, held natively by the kernel as a thin headline with optional detail and optional references, so it works in a bare repo and defers to Clearhead/OKF only when present.
tags: [architecture, constraints, intent, portability, continuation]
generated: { by: claude-code/opus-4.8, at: 2026-09-07T00:00:00Z }
---

# Decision — durable constraints are a portable intent entity, distinct from claims

**Status:** agreed 2026-09-07 as *captured design direction, not a build
green-light*. The cheap prerequisite — a bounded checkpoint-note excerpt in brief
status — **shipped 2026-09-07** (`BriefCheckpoint.note`, a 200-char word-boundary
excerpt via `BriefCheckpoint::from_marker`, surfaced in both status and delta;
regression + budget tests in `tests/walking_skeleton.rs`). The constraint entity itself is
**gated on friction**: build it only if salience misses persist in further
dogfooding after the excerpt ships, mirroring the measure-before-engineering
sequencing in [freshness-cost-sequencing](freshness-cost-sequencing.md). When it
is built, lead with the binding (see Consequences), and let that pull the entity
fields rather than treating headline/detail/references as frozen here.

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
OKF installed. So the kernel must hold a complete, self-sufficient model natively;
external tools are optional ceilings it can defer to, never dependencies. This is
exactly how the objective binding already behaves (an intent string that stands
alone, plus an *optional* `external_reference`).

A constraint entity is therefore, uniform with the objective:

- **headline** — required, terse; the binding line the continuation capsule shows;
- **detail** — optional, short inline rationale for the bare-repo case;
- **references** — optional, zero or more, out to an OKF decision, a Clearhead
  item, or another authority, resolved only when those tools are present.

Bare repo: headline (+ maybe detail) stands alone. Rich repo: references climb to
the authoritative rationale in OKF/Clearhead. The "must every constraint back to
OKF?" question is thereby settled — it **cannot** be required, because that would
break portability; OKF/Clearhead backing is pure enrichment.

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
  intent entity; constraints are the same class generalized. We are fleshing out a
  layer, not inventing a category.
- **Duplication is pay-as-you-go and self-policing.** In standalone mode there is
  one home and no drift; the capsule is renderable purely from kernel-local state,
  so wake never depends on an external tool being reachable. Duplication appears
  only when a reference is added. A reference whose cached excerpt no longer
  matches its source is *conceptually* the cross-tool version of staleness, but
  detecting it is not free: diff-on-stale is git-byte-range diffing, so it maps
  cleanly onto a git-native OKF concept and not at all onto external Clearhead
  state. Treat cross-tool reference-rot detection as aspirational, to be designed
  if and when references are actually built — not as machinery already in hand.

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
- **Deferred, needs its own pass — the binding.** How a constraint attaches to the
  pending work it governs drives capsule selection and lazy culling, but it
  crosses the same portability seam (next-actions live in Clearhead when present,
  kernel-local when not). Do not bolt it on here.
- **Deferred — scope attribute.** A genuinely temporary "for this objective only"
  constraint has no home under strict repo-scoping. A `scope` attribute (default
  repo, optional objective-bound) covers it, but is not built until the case
  actually appears.
- **No retrospective inference.** Constraints are authored at write time, never
  inferred from arbitrary prose; a renderer may rank and omit with a count, but
  must not invent a decision that was never recorded.

# Related Concepts

- [From change explanation to continuation insight](../evaluations/plot-continuation-insight.md): The field report whose salience finding motivated this entity.
- [External workspace state and the Clearhead boundary](external-workspace-and-clearhead-boundary.md): Establishes objective-binding-with-optional-reference and the sibling-authority rule this constraint entity mirrors.
- [Use OKF for curated project knowledge](okf-curated-knowledge-layer.md): The authoritative home for full decision rationale a constraint may reference.
- [S7 bounded perception](../design/s7-bounded-perception.md): The bounded continuation surface that selects and renders these constraints at wake.
