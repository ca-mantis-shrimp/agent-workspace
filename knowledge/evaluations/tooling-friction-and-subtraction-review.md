---
type: Design Reflection
title: Tooling friction and the case for subtraction
description: Assesses which workspace and memory tools an agent would choose voluntarily and recommends simplifying the operating protocol before adding capabilities.
tags: [evaluation, friction, tooling, memory, agent-experience]
generated: { by: pi/gpt-5.4, at: 2026-09-07T06:25:32Z }
---

# Tooling friction and the case for subtraction

## Overall judgment

The core system is useful, but its operating ritual is approaching the point where ceremony can outweigh the continuity it preserves. The architecture should remain; the next design phase should subtract friction rather than add capabilities.

The strongest test is voluntary use: if instructions stopped requiring these tools, an agent would still choose workspace status, delta, record-belief, checkpoint, and OKF search/show. Other surfaces should remain situational and earn their place through measured use.

## Tools that earn their place

- **Workspace status and delta** materially improve cold recovery by exposing the objective, current and stale beliefs, and changes since the last checkpoint.
- **Record-belief** matches the cognitive act of asserting a conclusion over cited inputs and is preferable to manually threading observation IDs.
- **OKF search and show** are better than grepping a monolithic implementation log for architectural history and prior evaluations.
- **Clearhead queries** are valuable when an agent must select and sequence work autonomously. They are not necessary ceremony for every direct user request.
- **MCP** is successful infrastructure because it centralizes schemas and should remain mostly invisible.

Working-set, findings, transaction, evidence, and full-audit projections are valuable only when task risk or uncertainty pulls them into use.

## Friction observed in live use

### Overlapping memory planes

The same fact can be duplicated across Agent Workspace claims, the OKF bundle, and personal Pi memory. Use one routing rule:

- Agent Workspace stores live execution knowledge whose input freshness matters.
- OKF stores curated project knowledge worth committing and sharing.
- Personal memory stores user preferences and cross-project continuity that does not belong in the repository.

A fact should normally have one canonical home; other layers may reference it rather than copy it.

### Claim accumulation

The semantic write API fixed claim starvation, but cheap writes now create curation pressure. This session produced several overlapping completion and migration claims. The system needs brief write receipts, atomic replacement, and an easier way to close or consolidate implementation claims at a checkpoint. Guidance should ask for handoff-relevant beliefs, not every intermediate conclusion. See the [semantic write API](../design/semantic-write-api.md).

### Objective binding remains bookkeeping

The OKF migration was initially checkpointed under the previous MCP objective because objective binding remained a separate manual act. The correction also hit a CLI naming mismatch (`--external-reference` guessed, `--reference` required). This is direct evidence for an atomic objective transition that checkpoints the prior objective and binds the next one.

### Always-loaded instructions are too broad

The installed OKF policy is useful, but detailed command tutorials belong in the on-demand skill rather than every agent’s opening context. The always-loaded contract should be limited to search when historical knowledge is relevant, persist only durable knowledge, prefer update over duplication, never forge verification, and validate bundle changes.

The startup protocol should also distinguish cold autonomous task selection from direct user-directed execution. Full Clearhead orientation is justified for the former, not automatically for the latter.

### Tool-generated history can become noise

The OKF relationship tool emits both a concept-update entry and a link entry for each relation. This is mechanically accurate but makes `log.md` noisier than the project history it is intended to clarify. The tool should eventually make semantic changes concise by default.

## Preferred lean workflow

### Cold start

1. Read workspace status.
2. Read workspace delta.
3. Search OKF only when the task has relevant historical context.
4. Query Clearhead only when selecting or sequencing work.

### During work

- Let native reads capture observations automatically.
- Record a belief when it would let a successor avoid reconstruction or detect dangerous drift.
- Use findings, evidence, and transactions when task risk warrants them.

### End of work

1. Run native validation.
2. Record one consolidated handoff belief.
3. Promote genuinely durable knowledge into OKF.
4. Checkpoint the workspace.
5. Update Clearhead when it owns the work item.

## Recommended next design work

Run a subtraction and friction audit before adding features:

1. make record-belief receipts brief by default;
2. add atomic belief replacement;
3. add atomic objective transition;
4. improve claim lifecycle cleanup at checkpoints;
5. shorten always-loaded instructions;
6. enforce the routing boundary between Workspace, OKF, and personal memory;
7. measure voluntary tool use across real foreign-repository tasks.

The system is addressing a real need. Its next risk is not missing capability but losing that value under bookkeeping and context overhead.
