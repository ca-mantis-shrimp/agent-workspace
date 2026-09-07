---
type: Decision
title: MCP servers resolve their repository from the launch directory and fail loud when it is wrong
description: MCP servers use repository-relative paths in .mcp.json (not the unreliable ${CLAUDE_PROJECT_DIR}), and the server names a missing repository instead of dying with an opaque I/O error.
tags: [architecture, mcp, deployment, proprioception]
generated: { by: claude-code/opus-4.8, at: 2026-09-07T07:26:59Z }
---

# Decision — MCP servers resolve their repository from the launch directory, and fail loud when it is wrong

**Status:** implemented 2026-09-06 (commits on `master`: mcp guard, .mcp.json fix).
**Date:** 2026-09-06
**Participants:** user + assistant (dogfooding the write door under real use).

## Context

The first real use of `workspace_record_belief` over MCP from a live Claude Code
session failed with a context-free `I/O error: No such file or directory`. Two
compounding causes, both confirmed by reproduction:

1. **`.mcp.json` keyed both servers off `${CLAUDE_PROJECT_DIR}`, which did not
   expand.** In a bridged Claude Code session (`CLAUDE_CODE_CHILD_SESSION=1`) the
   variable is simply unset, so each server received the literal string
   `${CLAUDE_PROJECT_DIR}` as its path and every filesystem-touching call died.
2. **The installed binary was stale**, advertising only 1 of the 10 tools the
   current source wires.

The kernel was never at fault — the CLI write path worked throughout. The failure
lived entirely in the *last mile* between the harness and the kernel, and the
error gave the agent no proprioception about which of "repository", "state", or
"citation" was wrong.

## Decision

1. **Resolve the repository from the launch directory, not a harness variable.**
   `.mcp.json` passes repository-relative paths: `agent-workspace mcp --repository .`
   and `okf mcp knowledge`. Claude Code launches an MCP server with its working
   directory at the project root, so `.` is the repository and `knowledge` is the
   bundle. Verified that `--repository .` resolves to the **identical**
   git-identity-keyed state store as the absolute path — the store is keyed by git
   identity (`locate.rs`), not the literal path string, so `.` is safe and, unlike
   an absolute path, portable to any clone or foreign repo.

2. **Fail loud with proprioception when the repository does not resolve.**
   `src/mcp.rs::run()` guards on `repository.is_dir()` before touching the store and
   returns an error that *names the path and the usual cause* (an unexpanded
   variable); `serve()` emits a matching startup warning to the client's MCP log.
   Non-fatal, so tools still list and the per-call error reaches the agent. This is
   the same fail-toward-legibility stance the kernel takes elsewhere: a bad path
   should announce itself, never masquerade as a bare errno.

## Rationale

- **`${CLAUDE_PROJECT_DIR}` is unreliable.** It is unset in bridged sessions, and
  there is no other dependable project-dir env var. A prior session bet on it and
  it silently broke the door. CWD-relative paths depend only on where the client
  launches the server, which is stable and needs no substitution.
- **Portability.** `.` and `knowledge` work in any clone; an absolute path baked
  into checked-in config would break every other machine and the foreign-dogfood goal.
- **Proprioception over silence.** The whole project exists to give the agent a
  trustworthy signal at the point of action. An opaque `I/O error` across the stdio
  process boundary is the antithesis of that; naming the path restores it.

## Consequences

- Any new MCP server added to `.mcp.json` must use CWD-relative paths. Do **not**
  reintroduce `${CLAUDE_PROJECT_DIR}` — a workspace claim (resting on `.mcp.json`)
  guards against a "helpful" revert.
- **Restart-snapshot tax remains.** Claude Code freezes the MCP tool set at session
  start, so shipping a new verb or fixing the server needs a client restart to take
  effect in-session. Develop against Pi's lighter reload, promote to MCP once stable.
- **Surfaced here, fixed 2026-09-07:** there was no discard/retract verb —
  `supersede-claim` requires a replacement claim, so a junk/probe claim could not
  be cleanly retired without corrupting the supersession graph. Resolved by the
  [claim-retirement-disposition](claim-retirement-disposition.md) decision
  (`retire-claim` / `workspace_retire_claim`). The transaction-discard door
  remains a deferred sibling gap.

## Alternatives rejected

- **Keep `${CLAUDE_PROJECT_DIR}`, fix its expansion.** Rejected: it is unset in this
  environment with no reliable substitute; the dependency itself is the fragility.
- **Bake an absolute `--repository` into `.mcp.json`.** Rejected: non-portable,
  breaks foreign repos and other clones.
- **Default `--repository` to CWD in the `mcp` subcommand (code change).** Deferred:
  the parser requires `--repository` for all verbs, and relaxing that globally is a
  broader footgun (a verb silently using the wrong CWD). Passing `.` from config
  achieves the same result with no code change and an explicit, auditable path.

# Related Concepts
- [Center the tool surface on MCP](mcp-centered-tool-surface.md): Operational hardening of the MCP surface this decision established: how the server learns its repository and fails legibly.
