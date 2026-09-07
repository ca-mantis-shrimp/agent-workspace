# Decisions

* [External workspace state and the Clearhead boundary](external-workspace-and-clearhead-boundary.md) - Keeps dynamic workspace state external and defines Clearhead and Agent Workspace as sibling authorities.
* [Center the tool surface on MCP](mcp-centered-tool-surface.md) - Makes MCP the canonical workspace tool surface and Pi a thin MCP client plus read-capture hook.
* [Use OKF for curated project knowledge](okf-curated-knowledge-layer.md) - Adopts OKF for durable decisions, research, specifications, and evaluations while preserving native authorities for tasks and operational workspace state.
* [Biome as the canonical TypeScript formatter, enforced at commit](typescript-formatter-gate.md) - Adopts pinned Biome (formatter-only) as canonical form for the Pi extension TypeScript and enforces it in the pre-commit gate, mirroring the rustfmt discipline.
