---
type: Decision
title: Harness session logs are a separate authority, joined to workspace state by session identity
description: Observes proprietary harnesses by archiving and normalizing their own session files outside the kernel, keeps live read capture in hooks, shares per-harness payload fixtures instead of parsing code, and joins the two sources on (host, harness, session) for offline analysis.
tags: [architecture, observability, telemetry, harness]
generated: { by: claude-code/opus-5.5, at: 2026-09-23T06:10:42Z }
---

# Decision — Harness session logs are a separate authority

**Status:** accepted direction 2026-09-22; nothing implemented yet.
**Participants:** owner + assistant (Claude Code session on `dabdesktop`).
**Work:** the `session-observability` Clearhead charter.

## Context

We want to understand how agents actually behave across the harnesses we use,
for cost, debugging, and eventually self-improvement. Those harnesses are
proprietary, so we cannot instrument their internals. The places we *can*
observe are their edges:

| Edge | Sees | Misses |
|---|---|---|
| Native telemetry (Claude Code OTel: `CLAUDE_CODE_ENABLE_TELEMETRY`) | tokens, cost, latency, tool events | prompts redacted by default; metrics and log events, no causal trace tree |
| Session files (`~/.claude/projects/`, `~/.pi/agent/sessions/`, …) | full ordered messages and tool calls, parent links | only after the fact; formats are not a stable API |
| Hooks | live lifecycle events; can act or block | only the points exposed; nothing about model calls |
| API proxy (`ANTHROPIC_BASE_URL`) | exact requests, full context | setup cost; conflicts with subscription auth |
| Our MCP server | calls to our tools | everything else |

Hooks are per-harness APIs, so building observability on them means N adapters
that drift apart. Session files and native telemetry already exist and need no
new infrastructure. The problem is **collection and normalization, not tooling**.

## Decision

1. **Session logs are their own authority, outside the kernel.** They record
   *behaviour*; the workspace records *intent* (beliefs an agent chose to
   assert, bound to revisions). The kernel does not ingest session logs, and
   session logs do not replace claims. This is consistent with the commons
   stance in [networked agency](../design/networked-agency-continuity-commons.md)
   (no transcript ingestion or activity feeds in shared state): the archive is
   the owner's offline analysis surface, not coordination state. Only distilled
   conclusions come back, as ordinary intentional publications (knowledge
   bindings, decisions).
2. **Collect by archiving raw files, append-only.** Every host pushes its
   session directories over Tailscale SSH to the home server with `rsync -a`
   (never `--delete`) into `raw/<host>/<harness>/`. That handles collection
   *and* retention (Claude Code deletes sessions after `cleanupPeriodDays`,
   30 by default). The archive holds secrets that passed through context:
   mode `700`, never backed up unencrypted off our machines.
3. **Normalize at read time, one adapter per harness.** Raw files are never
   modified; a per-harness *session-log normalizer* maps them into a common core
   plus a harness-specific `payload`, so concepts only one harness has
   (subagents, compaction) are kept, not flattened away. Target the
   OpenTelemetry GenAI semantic conventions (`gen_ai.*`) for the core rather
   than inventing a schema, noting they are still experimental. (Not to be
   confused with the superseded formatter work in
   [configurable normalizers](../design/configurable-normalizers.md).)
4. **Start with DuckDB; add a UI only when you need one.** No always-on services
   at first. Phoenix (one container) is the next step if a trace UI is
   missed; self-hosted Langfuse (Postgres, ClickHouse, Redis, S3) is heavier
   than the need. The normalizer stays; the backend can be swapped.
5. **Live read capture stays in hooks.** An observation is only worth anything
   if it is bound to the file at the moment of the read; archived reads arrive
   after edits and would fail closed on exactly the files under work. So
   `PostToolUse(Read)` and the Pi extension keep calling `observe-read` live.
6. **Share fixtures, not parsing code.** The Claude Code hook (Python), the Pi
   extension (TypeScript), and the archive normalizers read harness formats in
   different languages. `observe-read`'s input *is* the normalized read event;
   each harness gets a directory of captured payloads with expected normalized
   output, and every consumer is tested against it. Harness format knowledge
   stays at the edges; the kernel stays harness-agnostic.
7. **Join key is `(host, harness, session_id)`.** Proposed, not yet built:
   workspace events carry the harness session identity (hooks already receive
   `session_id`). A session id is only unique within one harness on one host.

## Consequences

- Questions neither source answers alone become possible: were stale claims
  acted on; do cold wakes prevent redundant reads; cost per resolved finding or
  committed transaction; claims replaced over and over.
- The read hook swallows every error by design, so capture misses are currently
  invisible. Reconciling archived reads against kernel observations gives a
  capture rate and reasons for misses; that is the first cross-source query.
- Session identity on events may also make the shared-intent misattribution in
  [the coordination pilot](coordination-pilot-and-trust-limits.md) visible.
- Nothing from the archive enters the wake projection; it is already over its
  budget.

## Guardrails

- Self-improvement stays human-gated: agents may *propose* changes from the
  analysis; the owner approves. Metrics that agents optimize against stop
  measuring the thing that matters.
- Design the normalized schema from real records, not ahead of them: explore raw
  files first, then let one concrete question (starting with an overnight-run
  morning summary) drive which core fields exist.
