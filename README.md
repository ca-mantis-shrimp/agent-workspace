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
                    │    Agent Workspace      │
                    │ events + projections    │
                    │ provenance + freshness  │
                    │ transactions + evidence │
                    └───────┬─────────┬───────┘
                            │         │
                     Pi projection   Neovim projection
                            │         │
                 ┌──────────▼─────────▼──────────┐
                 │ Git · LSP · syntax · tests   │
                 │ analyzers · command runners  │
                 └──────────────────────────────┘
```

The native tools remain authoritative. The workspace owns coordination state and preserves each provider's provenance and native result.

## Proposed MVP layers

1. **Kernel** — append-only events, materialized state, Git revision binding, checkpoints, and restart recovery.
2. **Repository model** — semantic locations, observations, working sets, jump history, and staleness detection.
3. **Work model** — findings, dispositions, validation evidence, and reversible change transactions.
4. **Adapters** — narrow integrations for Git plus one structural provider and one validation provider.
5. **Projections** — an agent-native Pi tool surface, then a thin Neovim projection of the same state.

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
checkpoint/delta recovery, and the Pi and Claude Code projections all share
kernel-owned semantics and have been exercised on the live repository.

The final dogfood pass found two boundaries that should be fixed before a
statusline-frequency Neovim projection: compact status takes about six seconds
on the accumulated workspace because it eagerly reconciles historical state,
and stale-first working-set ranking can omit the newest current focus. It also
found and fixed a tracked-directory-symlink crash in transaction startup. The
next architecture slice is therefore bounded/incremental reconciliation and
working-set curation; the thin Neovim projection follows on that shared-state
contract. Measurements and the decision rationale are in the
[dogfood field report](docs/reflection-dogfood-cold-resume.md). Implementation
choices settled by running code are recorded in
[`docs/implementation-notes.md`](docs/implementation-notes.md).

## Pi extension

The project-local extension at `.pi/extensions/agent-workspace/` auto-captures
successful text reads into the workspace without replacing Pi's native `read`
tool. Build the kernel first, then start Pi from the repository (or use
`/reload` in an already-running trusted session):

```sh
cargo build
pi
```

A bounded `read` streams its chrome-stripped model-visible text to the kernel's
harness-agnostic `observe-read`; the kernel—not the extension—maps lines to a
UTF-8 byte selector and validates drift, sensitivity, and containment. The
adapter separately preserves the complete model-visible byte count, including
Pi pagination chrome. Failed, truncated, drifted, out-of-repository,
workspace-internal, and sensitive-path reads fail closed; native payload
retention remains off.

The extension also exposes kernel-bounded `workspace_status` and
`workspace_delta` projections. Their defaults use compact JSON, cap claim and
change cardinality with explicit omission counts, and retain `full` expansion
for audit. The Claude Code `SessionStart` hook consumes the same bounded kernel
surfaces, so model-entry limits are shared semantics rather than adapter-local
summaries.

## Principles

- Preserve authority rather than hiding tool differences.
- Bind observations and evidence to revisions and inputs.
- Prefer progressive disclosure over repository ingestion.
- Make stale state visible instead of silently reusing it.
- Put mutations inside inspectable, reversible transactions.
- Keep the substrate shared while giving humans and agents native interfaces.
- Prove one end-to-end workflow before generalizing.
