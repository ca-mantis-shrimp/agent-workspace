# Agent instructions

This repository is an experiment in an agent-native workspace control plane.

Before working:

1. Read `README.md`.
2. Read `.clearhead/charters/workspace-mvp.md`.
3. Run `clearhead read charters`, `clearhead read actions`, and `clearhead query index unscheduled`.
4. Select the highest-priority ready action; do not skip predecessor relationships.
5. Treat `docs/initial-design.md` as a hypothesis. The executable contract and walking-skeleton evidence may revise it.

Use the Clearhead CLI—not manual `.actions` or sidecar edits—to update lifecycle state. Preserve native tool authority and provenance; do not turn the workspace into a generic wrapper API.

For each implemented slice, add executable acceptance coverage, record key decisions in the relevant document, update the Clearhead action, and commit a coherent checkpoint.
