# Design note — configurable normalizers (plan, not yet built)

> **Status 2026-09-03:** not started. This plans the seam that lets the kernel
> normalize more than Rust for freshness fingerprinting *without editing core
> code per language*. Written after a prettier reflow of the Pi adapter TS
> (semantic no-op) staled claim 67 — because TS fingerprints raw bytes today.
> Precedent: [`push-signals-belong-in-kernel-projection`] and the write-back-lag
> slice — solve it once in the core, every adapter inherits it.

## The gap

The kernel already normalizes code *before* fingerprinting, so
semantically-identical-but-restyled bytes get the same fingerprint and do **not**
stale a claim. That is why rustfmt edition skew stopped causing phantom
staleness. But the language→normalizer choice is a hard-coded `match`:

- `src/model.rs:97` `impl Normalizer` — `detect_for_path`, with `Some("rs") =>
  Self::Rustfmt` (`:111`) and `_ => Self::None` (`:112`).
- `src/reconcile.rs:139` calls `Normalizer::detect_for_path` at capture time.
- `src/reconcile.rs:256` `normalize_unit` dispatches the enum to the *invocation*
  (`Rustfmt => rustfmt_canonical`, `:259`; `rustfmt_canonical` shells `rustfmt`
  at `:263`).

So every non-Rust language fingerprints **raw bytes** → any reflow stales claims.
Adding Python or TS today means editing that `match` and the enum — the exact
per-language band-aid we keep hitting. This note replaces the `match` with
declared configuration.

## Scope boundary — this is the *freshness* half only

Two problems were bundled in discussion; keep them apart:

- **A. Formatting must not stale claims** (freshness integrity). Lives in the
  **kernel normalizer**. *This note.*
- **B. The tree-on-disk should stay clean** (readable diffs, no style noise in
  commits). Lives in **format-on-edit** — editor save-hooks for humans, an
  end-of-turn `Stop`-hook formatter for agents (never PostToolUse-on-`Edit`: a
  mid-turn reformat breaks the next `Edit`'s exact-string match). Separate slice,
  separate doc. Noted here only so a future session does not conflate them.

A alone would have prevented claim 67's staling. B is a lower-stakes convenience.
Do A first; it serves what this tool is *for*.

## Design — registry × config

Two layers with a clean split:

1. **Registry (the enum): *how* to run each formatter.** Stays in the core.
   `Normalizer` grows variants — `Rustfmt`, `Black`, `Prettier`, `Ruff`, `None` —
   each carrying its invocation contract in `normalize_unit` (stdin→stdout,
   exit codes, config discovery). Adding a *genuinely new tool* is still code:
   one variant + one invocation fn. This is unavoidable — something must know how
   to drive `black`.

2. **Config: *which* extension → *which* registered normalizer + pinned
   version.** Data, not code. Replaces `detect_for_path`'s hard-coded arms.
   Adding a language whose formatter is already registered becomes a config edit:

   ```toml
   # .agent-workspace/normalizers.toml  (illustrative)
   [normalizers]
   rs = { tool = "rustfmt", version = "<from rust-toolchain.toml>" }
   ts = { tool = "prettier", version = "3.x.y" }   # pin, see below
   py = { tool = "black",    version = "24.x.y" }
   ```

   Config selects **only from registered tools** — never an arbitrary shell
   command. Arbitrary commands cannot be made deterministic or safe, and the
   registry is what makes invocation known.

### Where config lives and when it is read

A **repo file** (property of *this codebase*: "Rust via rustfmt, TS via
prettier@X"), read at **capture time** to stamp `Observation.normalizer`
(already a durable per-observation field). Consequences, all desirable:

- Config evolution is **forward-only**: old observations keep the normalizer
  they were captured with; reconcile (`location_freshness_verdict`,
  `reconcile.rs:176`, uses `observation.normalizer`) always compares like with
  like. No retroactive restamping, consistent with event-sourcing.
- It travels with a clone and is reviewable in a PR.
- The `version` in config is documentation/guard; the *actual* pinned binary
  comes from the ecosystem's native manifest (`rust-toolchain.toml`,
  `package.json`, a pinned `black`). Config names the tool; the manifest pins it.

## The load-bearing constraint — determinism, or it is worse than nothing

A normalizer only keeps fingerprints comparable if it emits **identical bytes in
every environment**. rustfmt gets this free from the pinned rustup toolchain.
`black`/`prettier` do not: two agents on `black 24.1` vs `24.8` normalize the
same file to different canonical forms → different fingerprints → **silent false
staleness**, worse than raw-byte churn because it is invisible. Pinning is the
entire defense. An unpinned normalizer entry is a footgun; the loader should
refuse or loudly warn on an entry with no resolvable pinned version.

### Open decision: fallback semantics for a missing/mismatched formatter

Current behavior (`reconcile.rs:253-259`): `normalize_unit` **falls back to raw
bytes** when the tool is unavailable or the unit does not parse. For rustfmt this
is safe (effectively always present). For a non-ubiquitous formatter it is *not*:
an observation whose fingerprint was computed under normalization, reconciled in
an environment lacking the tool, would compare canonical-vs-raw → false stale.

The safer degradation is **`Unknown`** ("could not verify"), which the model
already supports, rather than a raw-byte comparison that fabricates a verdict.
Decide this explicitly when implementing, and mind the `observed_raw_fingerprint`
fast-path (`model.rs` doc-comment; `reconcile.rs:141`) — it compares raw bytes
first and skips the formatter when they match, so divergence only bites when raw
bytes differ but canonical form is (supposedly) equal, i.e. exactly the reflow
case normalization exists to catch.

## Build slices (each ships with acceptance coverage)

1. **Extract the seam.** Replace `Normalizer::detect_for_path`'s hard-coded
   `match` with a resolver that reads the config file and maps extension →
   registered normalizer. Seed config with `rs = rustfmt` only, so behavior is
   byte-for-byte unchanged. Test: existing Rust freshness tests still pass; a new
   test asserts the resolver returns `Rustfmt` for `.rs`, `None` for unmapped.
2. **Prove the seam on a real second language.** Register `Prettier`
   (`normalize_unit` variant + invocation) and add `ts = prettier` to config.
   This is the language that actually bit us. Test: a prettier reflow of a `.ts`
   observation's file is `current`, not `stale`, when prettier is present.
3. **Decide + implement fallback semantics** (raw vs `Unknown`) per the open
   decision above, with a test that forces the tool absent and asserts the chosen
   verdict — the guard against cross-environment divergence.
4. **Loader hardening.** Reject/warn on an entry lacking a resolvable pinned
   version; unknown `tool` names fail closed with a named error.

After slice 1, adding Python is: register `Black` once (slice-2-shaped), then a
two-line config edit — the "stop editing core per language" property is the goal.

## Deliberately out of scope

- **Markdown.** Prose uses line breaks as authorial intent; a formatter will
  fight it. Do not register a Markdown normalizer without a specific decision.
- **Problem B (format-on-edit / clean diffs).** Its own slice and doc.
- **The pre-commit gate.** A thin backstop, not the primary mechanism; unchanged
  by this note.
