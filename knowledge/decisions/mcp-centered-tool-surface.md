---
type: Decision
title: Center the tool surface on MCP
description: Makes MCP the canonical workspace tool surface and Pi a thin MCP client plus read-capture hook.
tags: [architecture, mcp, pi]
generated: { by: pi/gpt-5.4, at: 2026-09-07T06:05:54Z }
---

# Decision — Center the tool surface on MCP

**Status:** implemented 2026-09-06. The MCP server owns all ten workspace tool schemas; the Pi extension is an official SDK client plus capture hook, and the parallel TypeScript tool implementations are removed.
**Date:** 2026-09-04  
**Participants:** user + assistant (architectural check-in)  
**Context:** `workspace_bind_objective` just shipped as the second MCP verb
(`src/mcp.rs`); the Pi extension still implements parallel native tools
(`workspace_record_belief`, `workspace_status`, etc.) in TypeScript. We asked
whether the Pi surface should remain native or converge on the MCP server.

## Decision

**The MCP server is the canonical tool surface.** The Pi extension should
shrink to a thin wrapper that (1) captures reads via Pi middleware and (2) calls
the local MCP server for all workspace verbs. The kernel-owned Rust code becomes
the single source of truth for schemas, validation, and progressive disclosure.

## Rationale

- **One authoritative surface.** Today every new verb is wired twice: once in
  Rust for MCP and once in TypeScript for Pi. Centering on MCP collapses tool
  definitions, validation, and receipts into `src/mcp.rs`.
- **Harness-agnostic by design.** The README calls interfaces "thin transports."
  MCP already serves Claude Code, Cursor, Zed, etc. Pi should be another client
  of the same server, not a second server.
- **Drift reduction.** The semantic-write-api friction notes (brief receipts,
  atomic supersession, checkpoint-as-tool) get harder if fixes must land in both
  Rust and TypeScript. A single MCP surface eliminates that class of divergence.
- **Aligns with charter.** The workspace coordinates native tools but does not
  replace them; adapters should be as thin as possible. MCP lets the adapter be
  transport-only.

## What the Pi extension still owns

Read capture is inherently harness-specific. Pi's middleware observes finalized
model reads and forwards them to the kernel. Today it uses the `observe-read`
CLI over stdin; in the MCP-centered model it should call a kernel-owned
`workspace_observe_read` tool through the same MCP server. The extension also
owns the MCP server's lifecycle: spawn, keep-alive, and graceful shutdown when
Pi exits.

## Migration plan

1. Stabilize the next few MCP verbs (`workspace_supersede_claim`,
   `workspace_checkpoint`, brief-default receipts on existing verbs) so the
   surface is not changing every session.
2. Expose `workspace_observe_read` on the MCP server so the Pi hook can forward
   captures through the same transport used by tools.
3. Refactor `.pi/extensions/agent-workspace/`:
   - Spawn `agent-workspace mcp --repository <repo>` on extension load.
   - Register the discovered MCP tools as Pi tools with native descriptions.
   - Forward each typed tool call to the MCP server and return its result.
   - Keep the read-capture hook, but route captures to
     `workspace_observe_read` instead of the CLI stdin path.
4. Deprecate the hand-written TypeScript native tools once parity is proven.
5. Update README and agent guidance to describe Pi as "MCP client + capture
   hook."

## Implemented shape (2026-09-06)

- The server gained the five missing read projections: `workspace_status`,
  `workspace_delta`, `workspace_working_set`, `workspace_findings`, and
  `workspace_transaction_preview`. Brief/full and checkpoint-selection behavior
  calls the same kernel methods as the CLI; a missing transaction is a strict
  tool error.
- The Pi extension uses `@modelcontextprotocol/sdk` `^1.30.0`, discovers tools at
  load, passes the server's `inputSchema` directly to Pi, and keeps only a static
  map of Pi-specific labels and prompt guidance. One lazy client is shared per
  repository root and all clients close on `session_shutdown`.
- Read capture calls `workspace_observe_read` over that client. The exact
  model-visible byte count remains a separate argument after Pi pagination
  chrome is stripped from the semantic text.
- Absence or incompatibility fails harmlessly: the capture hook remains dormant
  and no broken workspace tools are advertised.
- End-to-end stdio coverage exercises all read projections, both bounded and
  full variants, checkpoint selection, transaction-not-found rejection, and the
  existing write loop. Pi integration coverage drives discovery, forwarding,
  strict errors, capture accounting, and shutdown against a fake MCP server.

## Caveats and preconditions

- **Surface stability.** Do not migrate while tool schemas or receipt shapes are
  still churning. The current session iterated on naming and receipt verbosity;
  converge on those first.
- **Restart latency.** Claude Code snapshots MCP tools at session start, so
  adding a verb requires a Claude restart. Pi `/reload` is lighter; use Pi for
  early validation, then promote to shared MCP once stable.
- **Stdio observability.** All traffic goes through one stdio pipe. Add server
  logging and a test harness that can record MCP request/response traces for
  debugging.
- **Hermetic tests.** Existing Pi tests pin `AGENT_WORKSPACE_BIN`; an MCP client
  extension must still allow test fakes or a controlled server path.

## Deferred alternatives rejected for now

- Keep Pi native tools and duplicate every verb. Rejected: it doubles the
  maintenance surface and invites schema drift.
- Make MCP the capture transport only and keep Pi native tools. Rejected: this
  keeps two tool schemas, which is the core problem.
- Drop the Pi extension entirely and rely on manual MCP calls. Rejected:
  automatic read capture is load-bearing; the hook must stay, it just needs a
  different backend.

## Acceptance for this decision

This decision is considered implemented when:

- The Pi extension loads the MCP server and discovers tools from it.
- `workspace_record_belief` and `workspace_bind_objective` both work through Pi
  without hand-written TypeScript implementations.
- Read capture still lands in the kernel, but via `workspace_observe_read` over
  MCP rather than CLI stdin.
- Pi and Claude Code exercise the same tool schemas and produce the same kernel
  state.
