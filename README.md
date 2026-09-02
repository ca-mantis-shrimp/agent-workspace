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
[executable contract](docs/executable-contract.md). The walking-skeleton kernel
action is complete: scenarios S1–S7 cover observation staleness, scoped claims,
evidence gating, restart-safe status, clean-base reversible file transactions,
byte-accounted bounded perception with reveal-on-demand, and explicit claim
supersession that separates active drift from retired beliefs. Default status is
a concise orientation surface backed by single-pass log projection; `--full`
retains the audit view. The Pi and Claude Code adapters now share kernel-owned
read-capture and bounded orientation semantics and have both been live-dogfooded;
the adapter phase is complete. The next product slice is the semantic-location
and bounded-working-set layer, where accumulated observations become an
intentional navigable working set rather than a provenance-only stream.
Implementation choices settled by running code are recorded in
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
