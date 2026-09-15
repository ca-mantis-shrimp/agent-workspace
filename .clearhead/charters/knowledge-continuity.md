---
id: 01a0a46f-3a7d-7e12-8f35-fb86cbf8554a
alias: knowledge-continuity
parent: agent-workspace
state: Active
---
# Harness-neutral knowledge continuity

Build the smallest harness-neutral continuation surface through which a cold agent can inherit the durable knowledge governing its present work without requiring a human recap or broad repository archaeology.

## Motivation

Agent Workspace already preserves operational state across sessions, while the repository’s OKF bundle preserves durable, human-reviewable project knowledge. A cold Claude Code probe in the foreign `plot` repository correctly rediscovered this boundary and avoided duplicating an already-recorded decision, but only after following repository links, reading instructions, running six knowledge searches, and spending 18 turns, 15 tool calls, 82.5 seconds, and $0.672. Retention worked; salience remained expensive.

The human should not repeatedly lead successive agents to the same accepted insight. A community has learned something only when a new participant can act from it, with honest provenance and discontinuity, rather than independently reconstructing it or pretending to remember.

## Product thesis

Agent Workspace should pulse a bounded set of **knowledge bindings** through `status` and `delta`:

- `status` answers which durable commitments currently govern the work;
- `delta` answers which bindings or authoritative sources changed since the checkpoint;
- native tools progressively reveal the referenced document when needed.

The workspace owns the situated relationship to knowledge—applicability, selection reason, source identity/version, availability, and change state—not the canonical document text. Human and agent participants use the same durable knowledge corpus; the boundary is temporality and authority, not audience.

## Required properties

1. **Harness-neutral kernel semantics.** CLI, MCP, Pi, Claude Code, Neovim, and future clients project the same bounded records; adapters add no knowledge-selection policy.
2. **One useful wake surface.** Brief `status` includes a small ranked knowledge pulse with explicit omission count; `delta` includes additions, removals, source-version changes, and availability transitions.
3. **Explainable applicability.** Every surfaced item says why it appears. Initial selection is based on explicit bindings and existing workspace relationships, never unlabelled semantic inference over all documentation.
4. **Progressive disclosure.** The pulse carries a headline and native/opaque reference, not a copied document body. Agents use the source’s native interface or repository read path for detail.
5. **Authority preservation.** In a repository with curated knowledge, that source remains canonical. The workspace records which source revision it relied upon. In a bare repository, a short workspace-native constraint may stand alone.
6. **Optional enrichment.** OKF, Clearhead, and other providers may enrich a binding but are never runtime requirements. `unavailable` and `unknown` are honest states, not permission to invent content.
7. **Distinct truth semantics.** Artifact claims remain freshness-tracked beliefs. Durable decisions and constraints retain establish/supersede/retire history and do not become false merely because code changes; their referenced source can still change or become unavailable.
8. **Bounded and safe.** Cardinality, headline, reason, and reference fields are bounded with explicit omissions. No automatic native-payload retention, secret ingestion, or whole-corpus wake scan.
9. **No human recap requirement.** A cold successor can identify and apply a governing principle without the human naming its storage system, re-explaining prior reasoning, or supplying an index.
10. **Measure before expansion.** Ship and test the smallest explicit binding and pulse before adding semantic recommendation, generic promotion automation, or broader coordination entities.

## Authority boundaries

- Git owns repository bytes, revisions, diffs, and whether a referenced document changed.
- OKF or ordinary repository documentation owns curated, portable project knowledge when present.
- Clearhead or another linked work system owns work existence, priority, predecessors, and lifecycle when present.
- Agent Workspace owns operational epistemic state and the live binding between current work and durable knowledge.
- Harness-local memory may cache a pointer for one participant but is never project authority or the community continuity substrate.

## Walking-skeleton scenario

The `plot` repository identifies itself as foreign dogfood for Agent Workspace. A durable binding associates its cold-successor work with the Agent Workspace decision governing curated knowledge and situated workspace state.

A fresh agent in a different harness receives brief `status` and `delta`. Without a conversational recap or broad knowledge search, it can:

1. see the governing decision’s headline, authority, reference, source version, and selection reason;
2. open the full source through a native tool only if needed;
3. place a new provisional belief in workspace state rather than harness-local memory;
4. avoid duplicating an already-curated decision;
5. observe a changed source version in `delta` after the referenced decision is revised;
6. continue honestly from the inline headline when the optional knowledge provider is unavailable.

The same scenario must work over request/response MCP without notifications, heartbeats, a Clearhead installation, or harness-specific memory.

## Non-goals

- Replacing `docs/`, OKF, Git, Clearhead, or native search/read tools.
- Copying a knowledge corpus into the workspace event log.
- Automatically inferring binding decisions from arbitrary prose.
- Building a generic semantic-search or recommendation engine in the kernel.
- Requiring every repository to adopt OKF or Clearhead.
- Persistent chat, mailbox semantics, fabricated continuous agent identity, or autonomous organizational task selection.
- Reopening the cancelled workstream, session, declaration, or structured-handoff entities without a new evidence-backed decision.

## Completion criteria

This charter can close when the contract is executable, the kernel and all current projections expose the same bounded knowledge pulse, source changes and unavailability fail honestly, and a predeclared cold-agent `plot` experiment demonstrates correct placement and reuse with no human recap and materially less discovery work than the 2026-09-15 baseline. The closing report must retain failures, context cost, false relevance, omission behavior, and any cases where existing documentation plus workspace claims remain sufficient without further machinery.
