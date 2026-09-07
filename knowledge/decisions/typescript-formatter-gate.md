---
type: Decision
title: Biome as the canonical TypeScript formatter, enforced at commit
description: Adopts pinned Biome (formatter-only) as canonical form for the Pi extension TypeScript and enforces it in the pre-commit gate, mirroring the rustfmt discipline.
tags: [architecture, formatter, tooling, pi]
generated: { by: claude-code/opus-4.8, at: 2026-09-07T06:42:57Z }
---

# Decision — Biome as the canonical TypeScript formatter, enforced at commit

**Status:** implemented 2026-09-06.
**Date:** 2026-09-06
**Participants:** user + assistant (user delegated the formatter choice).

## Context

The `.pi/extensions/agent-workspace` TypeScript had **no** committed formatter
config. An edit-time formatter (a lens on save) had silently reflowed
`index.ts`/`index.test.ts` in the working tree — the exact `.ts` residual
predicted when the Rust formatter drift was root-caused to a rustfmt edition
mismatch. Undefined canonical form means every session can re-drift and stale
downstream workspace claims for no semantic reason. The Rust side already
solved the same problem with a pinned toolchain plus a `cargo fmt --check`
pre-commit gate; the TypeScript side had neither.

## Decision

Adopt **Biome** as the single canonical TypeScript formatter for the extension,
and **enforce it in the pre-commit gate** alongside the existing rustfmt check.

- Canonical form is declared in `.pi/extensions/agent-workspace/biome.json`:
  tabs, 80-column, **formatter-only** (`linter.enabled: false`).
- The Biome version is pinned as an **exact** `devDependency`
  (`@biomejs/biome` `2.5.12`) via the package lockfile, so every environment
  produces byte-identical output — the same guarantee `rust-toolchain.toml`
  gives rustfmt.
- Config is scoped to `**/*.ts` (excluding `node_modules`) so Biome never
  fights npm over the 2-space `package.json`/lockfile.
- `.githooks/pre-commit` gains a directory-form Biome check that `cd`s into the
  extension (Biome discovers config from the working directory). It skips with
  a note when the pinned Biome is absent, matching the Rust gate's stance toward
  a missing toolchain.

## Rationale

- **Biome over Prettier.** The committed code already used tabs; Biome's default
  is tabs at 80 cols, so it is canonical with minimal churn, whereas stock
  Prettier (2-space) would rewrite every line for no reason. Biome is one
  self-contained binary (formatter + linter) — lighter to pin and faster,
  matching the repo's "leverage a battle-tested tool, do one thing right" ethos.
- **Formatter-only for now.** Enabling the linter would introduce a second,
  louder gate with its own backlog. Scope the decision to the drift problem;
  the linter is a separate, deliberate future choice.
- **Enforce, don't just define.** The rustfmt story's actual lesson was that the
  pre-commit gate — not the config alone — is what stopped drift from landing.
  A defined-but-unenforced canonical would let the identical incident recur.

## Consequences

- New verb/edit work in the extension must be Biome-clean to commit; run
  `npm run format` (added script) to fix.
- Unlike the Rust gate — whose blind spot is a `.rs` file not yet wired into the
  module tree — the TS gate globs the directory, so a brand-new `.ts` file is
  covered the moment it exists.
- Open follow-up: the edit-time lens's own formatter was not locatable in this
  repo, so the gate defines a canonical the repo can snap back to but cannot yet
  guarantee the lens emits it. If the lens keeps re-drifting on save, point it
  at this `biome.json` (or replace it with the pinned Biome).

## Alternatives rejected

- **Prettier.** Larger churn (tabs -> spaces) and heavier dependency for no gain.
- **Discard the drift, add no config.** Leaves canonical undefined; the drift
  simply returns next session — the band-aid the Rust root-cause explicitly
  rejected.
- **Match the lens's formatter exactly.** Could not locate the lens config in
  this repo; deferred until it demonstrably re-drifts.

# Related Concepts
- [Structural freshness without formatter coupling](../research/structural-freshness-without-formatter-coupling.md): Distinct concern: this formatter enforces source-commit hygiene, not the freshness fingerprinting that research rejected coupling to a formatter.
- [Center the tool surface on MCP](mcp-centered-tool-surface.md): Governs the Pi extension whose TypeScript this decision keeps canonical.
