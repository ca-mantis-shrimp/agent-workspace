---
id: 01a09e5a-1e60-7d81-a5ef-832eaea56380
alias: multi-agent-coordination
parent: agent-workspace
state: Active
---
# Harness-neutral multi-agent coordination

**Current disposition (2026-09-15):** the entity-heavy build thesis below is a
historical hypothesis, not a current implementation commitment. The 2026-09-14
selection cancelled workstreams, sessions, declarations, and structured handoff;
the 2026-09-15
[plural-agency transition](../../knowledge/decisions/plural-agency-charter-transition.md)
replaces the broad swarm dogfood with a separate research-only charter. This
charter remains active only for the independently selected
`orientation-worktree-context` correctness fix and
`coordination-authority-boundary` design action. Its original completion criteria
do not silently reopen cancelled entities.

The normative identities, failure semantics, and executable scenarios are defined in the
[contextual coordination contract](../../knowledge/specifications/contextual-coordination-contract.md).

Build a local-first coordination layer in which agents working through different
harnesses and Git worktrees share one repository-level knowledge substrate while
retaining honest session, workstream, and worktree context.

## Product thesis

The useful unit is not a globally authoritative agent or a private workspace per
worktree. It is a shared repository event history projected through each actor's
local execution context:

- claims, findings, decisions, and checkpoints are collectively discoverable;
- durable workstreams advertise intended outcomes, external work items,
  integration targets, dependencies, and advisory mutation or review scopes;
- ephemeral, resumable sessions attribute current interaction without pretending
  that recent activity proves liveness;
- Git worktrees isolate concurrent writers and supply the local bytes against
  which shared knowledge is assessed;
- Git remains authoritative for revisions, divergence, branches, worktrees, and
  whether an integration actually landed.

This is both defensive and generative coordination. Agents should avoid unsafe
mutation overlap, but they should also discover when another workstream is
already producing a capability they need, reshape their plan around its expected
integration, and re-verify shared claims after that integration lands.

## Core model

- **Repository/project** — one shared workspace identity. Linked Git worktrees
  share it; independent clones do not merge merely because remotes match.
- **Workstream** — the durable coordination owner for an intended outcome. It
  may reference a Clearhead action or other external authority and survives
  individual agent sessions.
- **Session** — an opaque, resumable attribution context attached to a
  workstream and worktree. Actor and harness labels are informative, not
  authentication.
- **Worktree context** — branch, base/head revisions, local divergence,
  transactions, and the filesystem used to assess shared records.
- **Assessment** — applicability is relational, not global:
  `freshness(claim, worktree context)`. One claim may be current in one worktree
  and stale or unknown in another without either verdict overwriting the other.
- **Declaration** — an advisory statement of intended capability, mutation or
  review scope, and integration target. It is not an enforceable lock or lease.
- **Dependency and handoff** — structured responsibility and integration state.
  A handoff transfers responsibility, never truth; the receiver still verifies
  freshness.

## Authority boundaries

- Clearhead or another work system owns what work exists, priority,
  predecessors, and lifecycle.
- Agent Workspace owns recorded epistemic and local coordination state.
- Git owns repository content, worktrees, revisions, diffs, and merges.
- Harness adapters own capture and presentation only. The canonical protocol is
  kernel-owned MCP state; notification is an optional optimization.

## Required properties

1. Every projection is bounded and reports explicit omission counts.
2. Scope declaration and its overlap snapshot are atomic under the kernel's
   serialized writer, even though the declaration remains advisory.
3. Session `last_seen` is presented as an estimate, never proof of active,
   paused, finished, or dead state.
4. Repository divergence is never mislabeled as agent activity.
5. Capability/dependency discovery complements path overlap: disjoint files can
   still implement the same outcome or satisfy another workstream's need.
6. Legacy unattributed events remain replayable; the system does not invent
   historical actors or sessions.
7. A request/response-only MCP client can coordinate correctly without
   heartbeats, runtime-specific hooks, push delivery, or remote-pi.

## Walking-skeleton scenario

Two agents use different harnesses in linked Git worktrees. Agent A declares a
workstream producing a durability capability, with a narrow mutation scope and
an integration target. Agent B independently discovers that it needs the same
capability, sees A's estimated activity and integration state, records a
dependency rather than duplicating the work, and continues disjoint work. Both
see the same claims, assessed independently against their own worktrees. A
checkpoints and hands off or marks the work ready; Git lands the integration; B
updates its worktree, verifies the landed revision, re-assesses the shared
claims, and resumes without a conversational recap.

## Non-goals for the first charter

- General persistent chat or mailbox semantics.
- Enforceable locks, leases, expiry, or mandatory heartbeats.
- Hosted presence, remote execution, multi-user authentication, or distributed
  synchronization.
- Automatic merging or treating a declared merge as Git truth.
- Semantic-symbol overlap before path and work-item/capability overlap prove
  insufficient.
- Autonomous organizational task selection or replacing Clearhead.

## Completion criteria

This charter can close when the walking skeleton passes across two harnesses and
two linked worktrees; shared claims receive independent worktree-relative
assessments; durable workstreams survive session replacement; atomic advisory
declarations expose path and capability overlap; dependencies and handoffs are
restart-safe; and Git-verified integration lets a dependent agent safely resume.
The result must be useful without notifications and must not introduce a second
task manager or chat system.
