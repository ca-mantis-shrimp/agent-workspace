# Agent instructions

This repository is an experiment in an agent-native workspace control plane.

Before working:

1. Read `README.md`.
2. Read `.clearhead/charters/workspace-mvp.md`.
3. Run `clearhead read charters`, `clearhead read actions`, and `clearhead query index unscheduled`.
4. Select the highest-priority ready action; do not skip predecessor relationships.
5. Treat `docs/initial-design.md` as a hypothesis. The executable contract and walking-skeleton evidence may revise it.

## Formatting gate

This repo ships a tracked pre-commit hook (`.githooks/pre-commit`) that rejects
commits whose Rust code is not `cargo fmt`-clean. It exists because formatter
noise — often rustfmt version skew between sessions — has repeatedly landed
unformatted edits that stale downstream workspace claims for no semantic reason.

Activate it once per clone (`core.hooksPath` is local config, not versioned):

```
git config core.hooksPath .githooks
```

If a commit is rejected, run `cargo fmt`, re-stage, and retry. Blind spot: the
check only sees `.rs` files reachable from a crate root, so a brand-new file not
yet wired into the module tree is not verified until it is.

Use the Clearhead CLI—not manual `.actions` or sidecar edits—to update lifecycle state. Preserve native tool authority and provenance; do not turn the workspace into a generic wrapper API.

For each implemented slice, add executable acceptance coverage, record key decisions in the relevant document, update the Clearhead action, and commit a coherent checkpoint.
