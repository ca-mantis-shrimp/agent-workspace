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

The executable contract is now recorded in the
[executable contract](docs/executable-contract.md). The active action is
`kernel`. Walking-skeleton scenarios S1–S6 now cover observation staleness,
scoped claims, evidence gating, restart-safe status, and clean-base reversible
file transactions. Implementation choices settled by running code are recorded
in
[`docs/implementation-notes.md`](docs/implementation-notes.md).

## Principles

- Preserve authority rather than hiding tool differences.
- Bind observations and evidence to revisions and inputs.
- Prefer progressive disclosure over repository ingestion.
- Make stale state visible instead of silently reusing it.
- Put mutations inside inspectable, reversible transactions.
- Keep the substrate shared while giving humans and agents native interfaces.
- Prove one end-to-end workflow before generalizing.
