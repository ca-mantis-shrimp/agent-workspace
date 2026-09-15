---
type: Decision
title: Use OKF for curated project knowledge
description: Separates durable shared project knowledge from situated workspace state by temporality and authority, while the workspace provides a unified harness-neutral orientation over both.
tags: [architecture, knowledge, okf, authority]
generated: { by: agent/cli, at: 2026-09-15T08:49:18Z }
---

# Decision — Use OKF for curated project knowledge

## Decision

The repository’s durable decisions, design notes, specifications, research, implementation history, and evaluations form an OKF v0.2 bundle under `knowledge/`. This replaces the former ad hoc `docs/` corpus rather than duplicating it.

## Boundary principle

Separate knowledge by **temporality and authority**, not by a human-versus-agent audience. Humans and agents collaborate against the same durable project knowledge:

- Material that should remain intelligible, reviewable in a pull request, portable with the repository, and reusable years later belongs in the OKF bundle.
- Revision-relative beliefs, current attention, uncertainty, dissent, findings, evidence, and active transitions belong in Agent Workspace state.
- Harness-local memory may help one participant, but it is neither project authority nor the continuity substrate for the agent community.

Agent Workspace is the unified, harness-neutral orientation surface over this boundary. It owns the live relationship to curated knowledge—what is currently relevant, what was relied upon, and whether its cited revision changed—without duplicating the knowledge’s canonical text. Successive agent instances should inherit that situated context from the workspace rather than requiring a human to repeat it; adapters must expose the discontinuity honestly rather than pretending that one persistent agent remembered it.

## Authority boundaries

- Clearhead or another linked work system remains authoritative for charters, actions, priorities, predecessors, and lifecycle; it is not a required workspace dependency.
- Agent Workspace external state remains authoritative for observations, claims, findings, evidence, checkpoints, transactions, and their situated relationship to curated knowledge.
- The OKF bundle is authoritative for curated project knowledge intended for human review, Git history, cross-agent retrieval, and long-term reuse.
- `README.md` and `AGENTS.md` remain entry points and operating instructions, linking into the knowledge bundle.

Operational records are promoted into OKF only when they become durable project knowledge; they are not mirrored automatically. Existing documents retain their bodies and Git history through the migration.

## Decision provenance

The repository owner confirmed this boundary in design discussion on 2026-09-15 after correcting an agent that had treated a harness-local memory plugin as the continuity foundation.
