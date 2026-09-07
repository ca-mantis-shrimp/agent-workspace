# Decisions

* [External workspace state and the Clearhead boundary](external-workspace-and-clearhead-boundary.md) - Keeps dynamic workspace state external and defines Clearhead and Agent Workspace as sibling authorities.
* [Center the tool surface on MCP](mcp-centered-tool-surface.md) - Makes MCP the canonical workspace tool surface and Pi a thin MCP client plus read-capture hook.
* [Use OKF for curated project knowledge](okf-curated-knowledge-layer.md) - Adopts OKF for durable decisions, research, specifications, and evaluations while preserving native authorities for tasks and operational workspace state.
* [Biome as the canonical TypeScript formatter, enforced at commit](typescript-formatter-gate.md) - Adopts pinned Biome (formatter-only) as canonical form for the Pi extension TypeScript and enforces it in the pre-commit gate, mirroring the rustfmt discipline.
* [MCP servers resolve their repository from the launch directory and fail loud when it is wrong](mcp-repository-resolution.md) - MCP servers use repository-relative paths in .mcp.json (not the unreliable ${CLAUDE_PROJECT_DIR}), and the server names a missing repository instead of dying with an opaque I/O error.
* [Claims gain a retirement disposition: retire without a replacement](claim-retirement-disposition.md) - Adds a ClaimRetired event and ClaimLifecycle::Retired variant so a claim can be retired without a successor, giving claims the freshness-orthogonal disposition axis findings already have and unclogging the stale signal.
