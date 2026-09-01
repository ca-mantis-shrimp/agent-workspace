---
id: 01a05b72-fb11-7201-aaa0-b02243a7a699
alias: workspace-mvp
parent: agent-workspace
state: Active
---
# Agent Workspace MVP

Build a small but real agent-native workspace: a stateful control plane that preserves intent, semantic location, provenance, staleness, change state, and validation evidence while native tools continue to do their specialized work.

The motivating problem is not that agents lack file and shell access. It is that the agent must currently reconstruct its working set and the relationships between repository state, requirements, reads, edits, diagnostics, and validation inside a bounded context window. That reconstruction is fragile, expensive, and usually lost between sessions.

## Product thesis

Neovim gives a human spatial memory, composable navigation, immediate feedback, and reversible change. An agent needs analogous affordances expressed through semantic memory, evidence-aware navigation, bounded context, persistent findings, and reversible transactions.

This workspace is a shared substrate with native projections for each participant:

- Pi receives an agent-oriented tool protocol.
- Neovim remains the human cockpit and projects workspace state through native concepts such as quickfix, signs, commands, and statusline data.
- Git, LSP, tree-sitter, ast-grep, compilers, test runners, Clearhead, and other systems retain authority in their own domains.

The workspace coordinates those systems; it does not replace them or flatten them into a lowest-common-denominator API.

## MVP outcome

A fresh agent can open a small Git repository and:

1. bind its session to an objective and repository revision;
2. navigate from project outline to module to symbol while retaining a semantic jump trail;
3. see which observations are current or stale;
4. begin a reversible change transaction with an explicit intent;
5. collect diagnostics and test outcomes in a persistent quickfix-like queue;
6. validate and checkpoint the transaction;
7. restart and recover the objective, working set, findings, transaction, and evidence without reconstructing them from chat history.

A human can inspect that same state, initially through a CLI or machine-readable status view and later through Neovim.

## Core concepts

The MVP should define only a small durable vocabulary:

- **Objective** — why the current work exists, usually linked to an external authority such as Clearhead.
- **Session/workstream** — one bounded line of investigation or implementation.
- **Semantic location** — repository path plus symbol identity, revision, and a relocation fingerprint.
- **Observation** — a fact obtained from a tool, carrying source, time, revision, native payload reference, and freshness.
- **Working set** — ranked locations and observations currently relevant to the objective.
- **Finding** — an actionable issue with lifecycle and disposition, preserving the provider's native result.
- **Change transaction** — intent, base revision, mutations, affected locations, findings, validation, and rollback boundary.
- **Evidence** — the result of a named check, bound to exact inputs and invalidated when those inputs change.
- **Checkpoint** — a restart-safe projection of the current workstream.

## Authority boundaries

- Git is authoritative for repository content, revisions, diffs, and rollback.
- Language and structural tools are authoritative for their native analyses.
- Command exit status and retained output are authoritative for executed validation.
- Clearhead or another task system is authoritative for project work state when linked.
- The workspace is authoritative only for coordination state: associations, provenance, freshness, dispositions, transactions, and checkpoints.

Every normalized record must retain its provider identity and enough native data or a content-addressed reference to reconstruct what the provider actually said.

## Constraints

- Local-first and useful in one repository before attempting distributed collaboration.
- Restart-safe; chat history must not be the only store of state.
- Revision-aware; stale evidence must be visible and must never silently count as current validation.
- Bounded; progressive disclosure is preferred over eagerly ingesting whole repositories.
- Reversible; mutations happen inside explicit transactions with inspectable diffs.
- Adapter-oriented without prematurely defining a universal tool schema.
- Safe by default: avoid storing secrets and support retention/redaction boundaries for native output.
- Human-inspectable: state and event history must have a straightforward textual or CLI projection.

## Non-goals for this charter

- Building a general-purpose replacement for editors, Git, LSP, CI, or task managers.
- Multi-user synchronization, remote execution, or hosted infrastructure.
- Autonomous task selection across an organization.
- Perfect symbol identity across arbitrary refactors.
- Normalizing every possible diagnostic, test framework, or agent runtime.
- A polished Neovim UI before the agent workflow proves useful.

## Walking-skeleton scenario

Use one deliberately small repository and one contained defect. The daemon records the objective and base revision; an adapter provides an outline; the agent focuses and reads one symbol; a file change invalidates the affected observation; the agent opens a transaction, applies a mutation, imports one diagnostic and one test result, resolves the finding, checkpoints, restarts, and receives the same coherent status.

The first implementation is successful when this scenario is covered end-to-end with automated tests and inspectable event history. Architecture should evolve from this scenario rather than from speculative support for every tool.

## Risks to investigate early

- Symbol relocation can produce false identity after edits.
- Event logs can retain sensitive command output indefinitely.
- Automatic evidence invalidation can be either dangerously weak or unusably broad.
- Adapter normalization can erase information needed for debugging.
- Concurrent human and agent writes can invalidate transactions mid-operation.
- A workspace that requires excessive bookkeeping will cost more context than it saves.

## Completion criteria

This charter can close when the walking skeleton works through the Pi interface, survives restart, detects stale observations and evidence, supports transaction rollback, retains native provenance, and has been dogfooded on representative maintenance tasks. The Neovim projection may remain experimental, but its shared-state contract must be demonstrated rather than mocked through unrelated editor-local state.
