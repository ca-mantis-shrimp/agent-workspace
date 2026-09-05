# Agent Workspace

An experiment toward an agent-native equivalent of the capabilities Neovim provides a human: durable orientation, semantic navigation, immediate feedback, bounded attention, and reversible change.

This is not intended to be another editor or a wrapper that renames existing tools. It is a **stateful coordination layer** over native authorities such as Git, LSP, tree-sitter, test runners, Clearhead, Pi, and Neovim.

## Why

Coding agents can read files, edit text, and run commands, but they commonly hold the relationships between those operations only in a transient context window. They lose:

- why a location was visited;
- which revision an observation describes;
- whether evidence became stale after an edit;
- how findings relate to an intended change;
- what must be restored after restart;
- which tool is authoritative for a claim.

The workspace makes those relationships explicit and inspectable.

## Shape

```text
                        Clearhead / objectives
                                 │
                    ┌────────────▼────────────┐
                    │      Agent Workspace     │
                    │  events + projections    │
                    │  provenance + freshness  │
                    │  transactions + evidence │
                    └──┬───────────┬────────┬──┘
                       │           │        │
                  MCP server    Pi ext.   Neovim
                 (any client)             projection
                       └───────────┼────────┘
                 ┌─────────────────▼───────────────┐
                 │  Git · LSP · syntax · tests      │
                 │  analyzers · command runners     │
                 └──────────────────────────────────┘
```

The native tools remain authoritative. The workspace owns coordination state
and preserves each provider's provenance and native result. Clients reach the
same kernel-owned state through whichever surface fits them: an MCP server (the
harness-agnostic path — Claude Code, Cursor, Zed, …), the Pi extension, or a
thin Neovim projection.

## Proposed MVP layers

1. **Kernel** — append-only events, materialized state, Git revision binding, checkpoints, and restart recovery.
2. **Repository model** — semantic locations, observations, working sets, jump history, and staleness detection.
3. **Work model** — findings, dispositions, validation evidence, and reversible change transactions.
4. **Adapters** — narrow integrations for Git plus one structural provider and one validation provider.
5. **Projections** — a harness-agnostic MCP surface (Claude Code, Cursor, …) and the Pi tool surface, then a thin Neovim projection of the same state.

See [the initial design outline](docs/initial-design.md) and [the active MVP charter](.clearhead/charters/workspace-mvp.md).

## Project state

Clearhead is authoritative for planned work:

```sh
clearhead read charters
clearhead read actions
clearhead query index unscheduled
```

The executable contract is recorded in the
[executable contract](docs/executable-contract.md). The walking skeleton and its
agent-facing MVP are complete: revision-aware observations and claims, bounded
working sets, persistent findings, evidence-gated reversible transactions,
checkpoint/delta recovery, and the MCP, Pi, and Claude Code surfaces all share
kernel-owned semantics and have been exercised on live repositories.

Evaluation has since moved outside this repository. The kernel installs
independently and resolves project-scoped state from an external local store
keyed by Git identity; foreign-repo dogfooding (on a separate `plot` project)
confirmed that a cold agent trusts a narrow current claim without defensively
reconstructing it, and that the fused `record_belief` write verb lands first-try.
The write loop is now reachable as a native tool — over MCP for any client, and
as a Pi custom tool — instead of raw CLI. The storage, ownership, and Clearhead
authority boundaries are in the
[external-workspace decision](docs/decision-external-workspace-and-clearhead-boundary.md);
measurements are in the
[self-hosted field report](docs/reflection-dogfood-cold-resume.md) and the
[foreign-repo write-API field report](docs/field-report-plot-foreign-dogfood.md);
implementation choices settled by running code are in
[`docs/implementation-notes.md`](docs/implementation-notes.md).

A residual surfaced while writing these docs: with the active-claim set grown,
the Claude Code `SessionStart` wake status now exceeds the harness's inline
preview budget (`orient_session_drive.py` flags it). That is the predicted
kernel-owned *bounded wake projection* slice — cap active-claim cardinality in
the wake surface, not just headline length — and it is the next hardening step
before the Neovim projection.

## Interfaces

An interface is a *thin transport*: it moves reads and beliefs to the kernel and
projects kernel state back, but owns no semantics. Every client speaks the same
vocabulary:

- **observation** — a file, or a byte range within one, captured at a Git
  revision. Ambient reads become observations automatically.
- **belief → claim** — you *record a belief* (the write act, citing the files it
  rests on); the kernel stores it as a *claim* (the tracked entity) bound to
  those observations.
- **freshness** — the kernel's verdict on whether a claim's cited inputs still
  hold. `current` means "the parts I checked are unchanged"; `stale` outranks
  your remembered belief and means re-verify before acting.
- **checkpoint → delta** — a named line drawn in the log, and the change since
  it. Together they are the cold-resume surface.
- **objective / working set / finding / transaction** — the bound goal, the
  ranked locations under attention, an outstanding issue, and a reversible
  evidence-gated change.

Both surfaces below bound their projections identically — compact JSON, capped
cardinality with explicit omission counts, `full` on demand — because those
limits are kernel semantics, not adapter-local summaries.

### Claude Code

Wired in `.claude/settings.json` and `.mcp.json` as three organs, none of which
replaces a native tool:

- **Sense** — a `PostToolUse(Read)` hook forwards each read window to the
  kernel's harness-agnostic `observe-read`.
- **Proprioception** — a `SessionStart` hook pushes the bounded `status` and
  `delta` into the model's opening context, so a cold session wakes oriented.
- **Write** — an MCP server (`agent-workspace mcp`) exposes
  `workspace_record_belief`, the fused observe+claim verb.

Per-repo setup — install the kernel with the (opt-in) MCP subcommand onto your
`PATH`, then let `.mcp.json` wire the server:

```sh
cargo install --path . --features mcp
```

The hooks and the MCP server both snapshot at session start, so **restart Claude
Code after wiring** for the tool to appear.

### Pi

The project-local extension at `.pi/extensions/agent-workspace/` auto-captures
successful text reads without replacing Pi's native `read`, and exposes the
`workspace_status`, `workspace_delta`, `workspace_working_set`,
`workspace_findings`, and `workspace_record_belief` tools. Build the kernel, then
start Pi from the repository (or `/reload` a trusted session):

```sh
cargo build
pi
```

A bounded `read` streams its chrome-stripped model-visible text to `observe-read`;
the kernel — not the extension — maps lines to a UTF-8 byte selector and validates
drift, sensitivity, and containment, while the adapter separately preserves the
full model-visible byte count. Failed, truncated, drifted, out-of-repository,
workspace-internal, and sensitive-path reads fail closed; native payload
retention remains off.

## Principles

- Preserve authority rather than hiding tool differences.
- Bind observations and evidence to revisions and inputs.
- Prefer progressive disclosure over repository ingestion.
- Make stale state visible instead of silently reusing it.
- Put mutations inside inspectable, reversible transactions.
- Keep the substrate shared while giving humans and agents native interfaces.
- Prove one end-to-end workflow before generalizing.
