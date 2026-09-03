# Decision — External workspace state and the Clearhead boundary

**Status:** Accepted direction; implementation and foreign-repository dogfood pending

**Date:** 2026-09-02

## Context

The MVP proved useful while observing its own development, but that is a recursive
and unusually forgiving test. Honest evaluation now requires using the workspace
on another repository, where reconstructing unfamiliar context is expensive and a
freshness verdict has an opportunity to earn reliance.

The project-local prototype currently assumes that both the kernel binary and its
state live inside the repository being observed. General use therefore forces two
related decisions:

1. where operational workspace state belongs and how several agents share it;
2. where the boundary lies between Agent Workspace objectives and Clearhead work.

## Decision

### Dynamic workspace state is external, local, and project-scoped

The canonical event log, payload store, checkpoints, and live transactions do not
travel implicitly with Git. An installed kernel and native client projections
resolve a project workspace under an XDG-style local state root.

This is not one global commingled workspace and not one isolated workspace per
agent. It is one logical workspace per repository/project, partitioned further by
workstream, worktree, actor, and session:

| Scope | State |
| --- | --- |
| Repository/project | identity, durable findings, provider results, shared checkpoints |
| Workstream | objective binding, active claims, decisions, risks |
| Worktree | mutations, candidate fingerprints, open transaction state |
| Actor/session | working set, navigation trail, provisional attention |
| Local payload vault | large, sensitive, or native provider output |

Every record that can cross an agent boundary retains actor and session
provenance. Another agent may verify that the recorded inputs remain unchanged;
that does not make the originating agent's reasoning objectively true.

Repository discovery begins locally: an explicit override, local registry entry,
or canonical Git common-directory identity can select or create a workspace.
Different worktrees may share repository-level state while retaining separate
transaction state. Separate clones do not share merely because their remote URLs
match; cross-clone identity must be explicit.

A small optional repository manifest may later carry a stable workspace locator
and capture/retention policy. It must not contain the dynamic event log, payloads,
or live transaction state. Cross-machine transfer, if needed, is an explicit
export/import of a bounded checkpoint that is reconciled against the receiving
clone before any record is reported current.

Start without a daemon. Pi, Claude Code, Neovim, and the CLI call the same
installed kernel over its filesystem-backed state. Add a local socket or daemon
only when measured subscription, concurrency, process-fan-out, or latency needs
justify it; transport must not duplicate kernel semantics.

### Clearhead and Agent Workspace are sibling authorities

Clearhead governs **what work ought to exist**: charters, actions, plans,
priorities, predecessor relationships, and project lifecycle.

Agent Workspace governs **what an agent currently knows about doing that work**:
observations, claims, freshness, attention, findings, candidate changes, and
validation provenance.

The workspace's `Objective` is therefore an **objective binding**, not a second
task record. It supplies the purpose needed to rank attention and organize a
workstream while optionally referencing an external authority such as a
Clearhead action, issue tracker item, or user request. A binding may preserve an
execution-specific intent and a versioned snapshot of the external reference;
it does not own the external item's priority, dependencies, or completion.

Information crosses the boundary deliberately:

1. At start, an external work item is projected into a workspace objective
   binding.
2. During execution, high-volume observations, claims, findings, transactions,
   and evidence remain in Agent Workspace.
3. At completion, a bounded checkpoint and validation summary may be explicitly
   written back to the external work system.

Accepting a transaction must not silently complete a Clearhead action, and a
Clearhead edit must not rewrite historical workspace events. A changed external
objective may instead stale the binding and require reorientation.

Agent Workspace remains useful without Clearhead. An ad hoc user request or a
reference to another tracker can supply the objective. Clearhead is the current
project-work authority, not a mandatory dependency of the agent's epistemic
substrate.

## Product consequence

"Agent-native" describes the optimization target, not private ownership by one
model instance. Capture must ride ordinary agent work, wake orientation must be
bounded, and freshness must address agent-specific epistemic failure modes.
Multiple agents can share the repository/workstream substrate without sharing a
working set or erasing provenance. Humans retain authority over ends and inspect
or correct the resulting execution state through native projections.

The next implementation slice is therefore portability, not another workspace
concept: installed-kernel discovery, external project-scoped state, global Pi
projection, and worktree/session attribution. It must be followed immediately by
a real multi-session task in a foreign repository. Subsequent features should be
pulled by failures observed there rather than by further self-hosting alone.
