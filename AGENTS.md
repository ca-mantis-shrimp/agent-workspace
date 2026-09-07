# Agent instructions

This repository is an experiment in an agent-native workspace control plane.

Before working:

1. Read `README.md`.
2. Read `.clearhead/charters/workspace-mvp.md`.
3. Run `clearhead read charters`, `clearhead read actions`, and `clearhead query index unscheduled`.
4. Select the highest-priority ready action; do not skip predecessor relationships.
5. Treat `knowledge/design/initial-design.md` as a hypothesis. The executable contract and walking-skeleton evidence may revise it.

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

The toolchain is pinned in `rust-toolchain.toml` (exact version, not floating
`stable`) so every environment produces identical rustfmt output — that is what
keeps the gate meaningful across agents and keeps rustfmt-normalized fingerprints
comparable. Under rustup this resolves automatically; it may trigger a one-time
toolchain download.

Use the Clearhead CLI—not manual `.actions` or sidecar edits—to update lifecycle state. Preserve native tool authority and provenance; do not turn the workspace into a generic wrapper API.

For each implemented slice, add executable acceptance coverage, record key decisions in the relevant document, update the Clearhead action, and commit a coherent checkpoint.

<!-- BEGIN OKF AGENT MEMORY -->
---

## 🧠 Persistent Project Memory (OKF v0.2)

> Powered by [OKF Agent Memory](https://github.com/okf-memory/okf-agent-memory) — Open Knowledge Format (OKF) v0.2 persistent project memory for AI agents.

When working in this codebase, you must follow the memory conventions:

1. **Persistent Knowledge Lives in `knowledge/`**:
   - The `knowledge/` directory is an **Open Knowledge Format (OKF) v0.2** bundle.
   - Store durable facts, architectural decisions, and project findings in `knowledge/`. Never store transient conversational noise.

2. **Read Before Write (Search Before Create)**:
   - Before authoring new knowledge or code, query existing memory: `okf search "<query>"` or inspect `knowledge/index.md`.
   - Update existing concepts instead of creating duplicates.

3. **Strict Context & Search-First Retrieval (No Blanket Scans)**:
   - **DO NOT** use `list_dir`, `grep`, or scan `knowledge/` in bulk.
   - Query knowledge via `okf search "<query>" --limit 3 --json` only when relevant or requested.
   - Inspect concept descriptions first and load full concepts only on demand using `okf show <id>`.

4. **Preserve Trust & Provenance**:
   - Agent writes declare `generated: { by: "<agent>", at: "<timestamp>" }`. Never forge human verification (`verified:`).

5. **Essential Memory Commands**:
   - `okf search "<query>"` — Query memory using in-memory BM25
   - `okf show <id>` — Inspect concept details and relationship graph
   - `okf create <id> --type <type> --title "<title>" --desc "<desc>"` — Document new fact
   - `okf update <id> --desc "<updated-desc>"` — Modify existing concept
   - `okf relate <src> <tgt> --desc "<rel>"` — Link concepts together
   - `okf validate knowledge --strict --drift` — Verify 100% OKF v0.2 conformance

6. **End-of-Task Review Checklist**:
   - Did I make an architectural decision? -> Record under `knowledge/architecture/`
   - Did I add/update concepts? -> Ensure `knowledge/log.md` and parent `index.md` are updated.
   - Did I validate? -> Ensure 0 errors, 0 broken links (`okf validate knowledge --strict`).
<!-- END OKF AGENT MEMORY -->
