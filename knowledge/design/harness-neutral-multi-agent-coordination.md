---
type: Design Note
title: Harness-neutral multi-agent coordination
description: Refines a harness-neutral Agent Workspace subsystem around session attribution, advisory scope declarations, overlap and repository-divergence signals, and structured handoff; defers leases, heartbeats, and general messaging.
tags: [design, coordination, multi-agent, mcp, harness-neutral]
generated: { by: pi/gpt-5.4, at: 2026-09-14T05:00:02Z }
---

# Design Note — Harness-neutral multi-agent coordination

**Status:** historical design input. The accepted identities, failure semantics, and executable scenarios now live in the [contextual coordination contract](../specifications/contextual-coordination-contract.md); where this note differs, that contract governs.

## Motivating observation

A live experiment on the ClearHead platform repository exposed a useful boundary. One
Claude Code agent was actively modifying the `clearhead-core` submodule while a separate
Pi agent inspected the same Agent Workspace state.

The second agent could recover substantial shared context without asking for a recap:

- the current `direct-delivery` intent and its external charter;
- the latest coherent checkpoint, including completed Phase 1 work and named remaining
  phases;
- the claims and source locations that had become stale as the first agent edited them;
- the dirty submodule and the live set of modified files through Git.

But it could not determine from Agent Workspace itself:

- which agent owned the work;
- whether that agent was active, paused, or finished;
- what it intended to edit next;
- whether another writer could safely touch a path or symbol;
- how to ask for, transfer, or decline ownership of overlapping work.

No observations, claims, findings, or transactions had been recorded after the latest
checkpoint, even though the filesystem showed substantial in-progress mutation. The
workspace therefore supported excellent **historical and epistemic orientation**, while
lacking a **live coordination protocol**.

The important corrective observation is that this was not a wholesale coordination
failure. Agent Workspace plus Git supplied roughly 80% of the orientation needed. The
missing 20% was primarily **attribution**: which session or workstream was responsible for
the current divergence, what scope it expected to mutate, and whether that scope
intersected another actor's intended work. The design should close that narrow gap before
attempting a general multi-agent platform.

## Proposed boundary

Multi-agent coordination should live in the same Agent Workspace service and share its
repository/workspace identity, but remain a distinct bounded context with a different
lifecycle from knowledge records.

```text
Agent Workspace
├── knowledge
│   ├── intent, observations, claims
│   ├── findings and evidence
│   └── checkpoints
├── transactions
│   ├── proposed changes
│   ├── affected locations
│   └── acceptance and rollback
└── coordination
    ├── attributed sessions / workstreams
    ├── declared mutation and review scopes
    ├── overlap and repository-divergence signals
    └── checkpoint handoffs
```

Beliefs, findings, checkpoints, and selected handoff/conflict decisions are durable.
Session liveness is only an estimate derived from recent kernel interaction. Coordination
records may reference an existing intent, Clearhead action, claim, finding, transaction,
semantic location, Git revision, branch, or worktree without duplicating those entities.
Expiring leases and persistent messages remain possible later layers, not assumptions of
the first slice.

This separation matters: a belief means "an actor currently accepts this proposition on
these cited inputs." It must not also mean "an agent owns this file." Freshness and
ownership answer different questions and have different expiry rules.

## Harness neutrality is a correctness requirement

The protocol must not depend on remote-pi, Claude Code hooks, Pi events, or any other
particular harness. Those may be experiments or optional notification adapters, but they
cannot be the coordination authority.

The canonical surface should be ordinary MCP backed by kernel-owned persistent state, in
line with [`mcp-centered-tool-surface`](../decisions/mcp-centered-tool-surface.md). The
smallest likely vocabulary is:

```text
workspace_coordination_status
workspace_declare_scope
workspace_release_scope
workspace_handoff
```

The server should establish an opaque session lazily on first use rather than requiring a
bookkeeping-only `workspace_join`. Richer session metadata can be supplied deliberately
when it adds meaning. A harness that supports only request/response MCP must be fully
capable of coordinating. Push notifications may improve immediacy, but losing them must
not change correctness.

> Persistent shared state is the protocol; notification is an optimization.

This suggests a pull-safe loop:

1. implicitly establish or resume a workspace session through normal tool use;
2. inspect coordination status before mutation;
3. declare the intended work item and advisory write or review scope;
4. inspect overlap and repository-divergence signals at natural boundaries;
5. checkpoint and release or hand off the scope.

## Sessions, identity, and liveness

The kernel should lazily assign an opaque session identity rather than trusting a
harness-provided display name as authority or requiring a ritual join call. A session may
then declare:

- an informative actor/harness label;
- repository, worktree, branch, and base revision;
- current intent or externally authoritative work-item reference;
- whether it is observing, reviewing, or mutating;
- its proposed path and semantic-location scope.

Recent kernel interaction provides a useful `last_seen` signal, but not proof that an
agent is active, paused, or dead. Avoid a mandatory heartbeat protocol: agents will omit
it under pressure, and a delayed harness may appear dead while still mutating. Liveness
must therefore be projected as an estimate. Git and transaction state still require
inspection regardless of apparent session age.

Identity is local coordination identity, not authentication. A future distributed design
would need a separate trust model and is outside this proposal.

## Declared scopes and conflict semantics

A declared scope should be more expressive than a file lock. Useful selectors include:

- Clearhead action or other external work-item reference;
- repository-relative paths or directory prefixes;
- semantic locations such as symbols;
- an Agent Workspace transaction;
- review-only versus mutation intent.

Before accepting or reporting a declaration, the kernel can compare it with other active
scopes and open transaction paths. Overlap should be explicit and ranked rather than
reduced to a boolean: two readers do not conflict, a reviewer may overlap a writer, and
two writers on the same symbol are more dangerous than writers in unrelated files under
one directory.

The first implementation should not call these declarations **leases**. Agent Workspace
cannot guarantee that native editors or shell commands honor them, and expiry would risk
creating false safety. Begin with advisory, fail-visible overlap reporting. A later
experiment may earn leases if ownership and expiry materially change behavior.

Concurrent mutation should normally use one Git worktree per writer. The coordination
layer helps agents discover and negotiate scope; Git worktrees provide actual isolation.
Multiple agents writing one worktree should remain an exceptional mode requiring narrow
scope declarations and explicit overlap handling.

## Repository divergence

The observed `workspace_delta` was empty even while Git showed seventeen modified files
and four cited locations had become stale. That result was semantically correct—the event
log had not changed—but incomplete for operational orientation. A coordination projection
should join, without conflating:

```text
workspace events since checkpoint: none
repository divergence since checkpoint: 17 modified files
cited locations now stale: 4
associated transaction/workstream: unknown
```

The workspace must call this **repository divergence**, not agent activity: a human,
formatter, editor, or uninstrumented process may have produced it. Likewise, no recorded
findings or transactions means "none recorded," not "clean" or "idle."

## Handoff first; mailbox deferred

A general persistent mailbox is probably premature. It risks becoming a second chat
protocol and another queue agents must remember to poll. The first slice needs structured
handoff and conflict state attached to stable workspace entities; general messages should
be earned by a concrete cross-harness experiment.

A handoff should bind:

- the relinquished scope and receiving session, if known;
- the relevant checkpoint and Git/worktree revision;
- open findings and transaction state;
- residual risks and unverified assumptions;
- whether the previous lease was released, transferred, or merely expired.

The receiving agent still verifies freshness. A handoff transfers responsibility, not
truth.

## Relationship to existing authorities

- **Clearhead or another task system** remains authoritative for what work exists,
  priority, predecessors, and lifecycle.
- **Agent Workspace knowledge** remains authoritative for recorded epistemic execution
  state: provenance, freshness, findings, evidence, and checkpoints.
- **Agent Workspace coordination** becomes authoritative only for declared local sessions,
  claimed scopes, messages, and handoffs.
- **Git** remains authoritative for repository content, revisions, diffs, worktrees, and
  rollback.
- **Harness adapters** provide capture, presentation, and optional notification. They own
  no coordination semantics.

This extends rather than weakens the boundary in
[`external-workspace-and-clearhead-boundary`](../decisions/external-workspace-and-clearhead-boundary.md):
coordination may reference Clearhead work, but must not become a second task manager.

## Smallest falsifiable slice

Do not begin with a general distributed-agent platform. A useful first slice can remain
local-first and repository-scoped:

1. Attribute every new event to a lazily established session/workstream.
2. Project actor label, worktree, base revision, intent, and last kernel interaction.
3. Allow a session to declare and release an advisory mutation or review scope.
4. Return a bounded view of sessions, overlaps, repository divergence, and explicit
   omission counts.
5. Support a structured checkpoint handoff without adding general messaging.
6. Expose all operations through the same MCP server used by every harness.

A minimal acceptance experiment uses two different harnesses in separate worktrees:

- both see the same active intent and coordination projection;
- agent A declares a narrow mutation scope;
- agent B receives an explicit overlap warning before declaring an intersecting scope;
- agent B can safely declare a disjoint scope;
- unassociated Git changes appear as repository divergence rather than invented activity;
- A or a replacement agent can checkpoint and hand the scope off;
- the scenario works without explicit heartbeats, remote-pi, harness hooks, or push
  delivery.

Only after that experiment should leases, persistent inboxes, semantic-symbol overlap,
automatic notifications, or distributed synchronization be promoted.

## Open questions

1. Is a declared scope attached primarily to a session, workstream, transaction, or work
   item? A session supplies attribution, while a workstream may be the more durable owner.
2. Which coordination events deserve permanent audit retention, and which are only a
   bounded projection of current state?
3. When, if ever, does an advisory declaration earn enforceable or expiring lease
   semantics?
4. Can transaction affected paths seed a declared scope without making transactions
   mandatory before the first edit?
5. ~~How should untracked files and submodule-owning repositories participate in overlap
   checks?~~ Resolved by the contextual coordination contract: recursively join initialized
   submodule state into the project namespace, retain owning-repository provenance, and
   fail unknown rather than clean when nested coverage is unavailable.
6. What bounded projection is sufficient at session start without recreating the oversized
   wake-status problem?
7. Should bypass detection create a finding, a coordination warning, or both?

## Position

The experiment supports adding a narrow attribution-and-overlap subsystem, but not yet a
general coordination platform, mailbox, heartbeat protocol, or enforceable lease model.
The promising architectural statement is:

> Same service and workspace identity; separate bounded context and lifecycle.

The critical constraint is broader than any one agent runtime: independent harnesses must
coordinate through kernel-owned MCP state. Runtime-specific transports may improve the
experience, but the system must remain correct when they are absent.

# Related Concepts

- [Center the tool surface on MCP](../decisions/mcp-centered-tool-surface.md): Builds coordination on the canonical harness-agnostic MCP surface rather than a runtime-specific transport.
- [External workspace state and the Clearhead boundary](../decisions/external-workspace-and-clearhead-boundary.md): Extends external epistemic workspace state with local coordination while preserving Clearhead as the work-lifecycle authority.
- [First superproject dogfood — the submodule boundary is narrower than feared](../evaluations/platform-superproject-dogfood.md): The platform dogfood supplied the live case where checkpoint and staleness were visible but agent ownership and liveness were not.
