---
type: Decision
title: Use OKF for curated project knowledge
description: Adopts OKF for durable decisions, research, specifications, and evaluations while preserving native authorities for tasks and operational workspace state.
tags: [architecture, knowledge, okf, authority]
generated: { by: pi/gpt-5.4, at: 2026-09-07T06:09:33Z }
---

# Decision — Use OKF for curated project knowledge

## Decision

The repository’s durable decisions, design notes, specifications, research, implementation history, and evaluations form an OKF v0.2 bundle under `knowledge/`. This replaces the former ad hoc `docs/` corpus rather than duplicating it.

## Authority boundaries

- Clearhead remains authoritative for charters, actions, priorities, predecessors, and lifecycle.
- Agent Workspace external state remains authoritative for observations, claims, findings, evidence, checkpoints, and transactions.
- The OKF bundle is authoritative for curated project knowledge intended for human review, Git history, cross-agent retrieval, and long-term reuse.
- `README.md` and `AGENTS.md` remain entry points and operating instructions, linking into the knowledge bundle.

Operational records are promoted into OKF only when they become durable project knowledge; they are not mirrored automatically. Existing documents retain their bodies and Git history through the migration.
