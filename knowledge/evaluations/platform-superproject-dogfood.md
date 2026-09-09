---
type: Field Report
title: First superproject dogfood — the submodule boundary is narrower than feared
description: Reports the first real-repo trial on a multi-submodule superproject (ClearHead platform), proves freshness crosses the submodule boundary via content fingerprinting, and maps the git-dependent periphery that misresolves against opaque submodule gitlinks. AMENDED 2026-09-09 (later session) — the boundary is WIDER than the original headline: beyond the cosmetic provenance/diff degradation, the S6 transaction clean-base gate is inoperable on submodule files; a verified ls-tree→owning-repo resolution closes it with no schema change. See the Amendment section.
tags: [evaluation, dogfood, superproject, submodule, freshness, provenance, transactions]
generated: { by: claude-code/opus-4.8, at: 2026-09-09T07:46:17Z }
---

> **Amendment 2026-09-09 (later session):** the original headline —
> "the boundary is narrower than feared" — was premature. It held only for the
> freshness *verdict*, the one path this session tested. Tracing the git-dependent
> code found a load-bearing gap the run never exercised: the **S6 transaction
> clean-base gate is inoperable on submodule files**, so the change-governing
> feature is unusable on the exact files a superproject exists to coordinate. A
> resolution was verified live against `platform`. Full detail in the
> [Amendment](#amendment-2026-09-09--the-transaction-gate-is-the-real-gap)
> section at the end; read it before acting on the original "fix to implement".

# Field report — first superproject dogfood (ClearHead platform)

*Written by the working agent after wiring agent-workspace into the ClearHead
`platform` repo — a higher-order **superproject** coordinating five git
submodules (`specifications`, `ontology`, `tree-sitter-actions`,
`clearhead-core`, `clearhead.nvim`). This is the first dogfood on a
superproject; every prior trial (`plot`, self-host) was a flat single repo.
It is a field report, not a spec — the contract and design notes remain
authoritative for behavior. It supersedes the pre-run hypothesis that the
superproject/submodule "revision-binding gap" would blind freshness.*

## What happened

The kernel was wired into `platform` (`.mcp.json` + `SessionStart`/`PostToolUse`
hooks) in a prior session and loaded cold on restart: all 13 MCP tools appeared,
`workspace_status` round-tripped against the external git-identity store. The
session then built a durable orientation base for future cold agents — intent +
**4 cited beliefs** (architecture, active charters, specifications authority,
objectives) + checkpoint `orientation-base-2026-09-09` at superproject HEAD
`c977dc4`, 8 observations.

Then we ran the boundary probe that motivated the whole session: claim #2 cites
`specifications/README.md`, a file **inside the `specifications` submodule**. We
edited that file (a real two-line insertion), ran `workspace_status`, watched #2
flip to `stale`, inspected it with `workspace_explain_stale`, then reverted the
edit and confirmed #2 healed back to `current`. A clean stale→current round-trip
across the submodule boundary.

## Findings

**1. Freshness is NOT blind to submodules — the pre-run hypothesis was wrong.**
`assess_claim_inputs` (`src/reconcile.rs:74-112`) decides `current` vs `stale` by
pure **content fingerprinting**: for each cited path it `fs::read`s the file,
normalizes the selected unit, SHA-256s it, and compares against the recorded
fingerprint. There is no `git` in the verdict. So a change to a submodule
file — committed **or** uncommitted — correctly staleness the citing claim,
because the bytes on disk change and the hash differs. Confirmed live: #2 went
`stale` with reason `"recorded claim input changed"` from an uncommitted edit
inside a submodule, and returned to `current` on revert. The load-bearing
capability already works across the boundary; the git revision is provenance
metadata, not the freshness determinant.

**2. Containment is path-prefix, so submodule files are first-class
observations.** `resolve_repository_file` (`src/reconcile.rs:114`) accepts any
path whose canonical form `starts_with(repository_root)`. A submodule file
(`specifications/…`, `clearhead-core/…`) canonicalizes under the superproject and
passes — no fail-closed surprise. Confirmed end-to-end: the `PostToolUse(Read)`
Sense hook captured `specifications/README.md` as an observation, and
`record_belief` **reused** it (`reused: true`) rather than re-reading. Ambient
sense → belief works across the submodule boundary.

**3. The one real degradation: three git renderings run in the wrong repo.**
A single recorded value — the observation's `observed_revision`, set at observe
time to `git rev-parse HEAD` of the **superproject** (`src/lib.rs:979`) — feeds
three superproject-scoped git operations, all executed at the superproject root
with the superproject-relative path:

  - **provenance display** — the recorded revision (`c977dc4`) is shown even for
    a submodule file, which is cosmetically misleading ("captured at c977dc4"
    when the file lives in a submodule with its own HEAD);
  - **`investigate_drift`** (`src/reconcile.rs:415-434`, the diff at `:423`) —
    `git diff <base> -- <path>` at the superproject sees the submodule as a
    gitlink, not a tree of files, so it produces no content diff and
    **degrades to a whole-file `current_content` dump**. Observed live: the
    `explain_stale` view came back `kind: "current_content"` (truncated),
    **not** `kind: "git_diff"`;
  - **`probe_relocation` / `git_file_at_revision`** (`src/reconcile.rs:767`) —
    `git show <base>:<path>` at the superproject cannot resolve a path into a
    submodule tree, so the relocation probe silently fails and falls back.

None of these breaks the stale **verdict** (finding 1); they degrade the
*explanation and provenance*. But the per-input fingerprint drift breakdown still
correctly names the changed file, so even the degraded `explain_stale` points at
the right place — you lose the scoped diff, not the signal.

**4. Suspected-but-untested secondary gap: `worktree_fingerprint`.**
`worktree_fingerprint` (`src/reconcile.rs:811`) enumerates via `git ls-files`
at the superproject, which does not list files *inside* submodules. Any coverage
or operational-fingerprint logic resting on it is likely submodule-blind. Not
exercised this session; flagged for the implementer to check, but secondary
because the freshness verdict does not depend on it.

## The fix to implement next session (scoped, with evidence)

The degradation is one coherent idea, not sprawling "sub-repo awareness":
**resolve each observed file's owning git repo + repo-relative path, and run git
operations there.** Recommended paired-correct version (fixes provenance honesty
*and* the rendered diff), deliberately small:

1. **One helper** — `owning_git_context(repo_root, path) -> (git_root,
   repo_relative_path)`: `git -C <path.parent> rev-parse --show-toplevel`, then
   strip-prefix for the relative path. Fall back to `(repo_root, path)` on any
   failure so behavior gracefully degrades to today's for non-submodule files.
2. **Write site** (`src/lib.rs:979`, the observe path): record the **owning
   repo's** HEAD as `observed_revision` instead of the superproject's. This makes
   provenance honest and gives the read sites a base revision that actually
   exists in the owning repo.
3. **Read sites** (`investigate_drift` `:423`; `probe_relocation` /
   `git_file_at_revision` `:767`): run their `git diff` / `git show` in the
   owning `git_root` with the repo-relative path.
4. **Deliberately NOT touched:** the reconciliation-fingerprint cache keys
   (`src/reconcile.rs:721,749`) that mix superproject HEAD. Freshness is
   fresh-read (finding 1), so these are a harmless cache ingredient; redirecting
   them drags in a multi-repo-in-one-hash problem for zero correctness gain.
   Holding this line is the "don't increase complexity" boundary.
5. **The test (the point):** extend the `make_repo` fixture (see
   `tests/mcp_stdio.rs:25`) with a nested submodule, record a belief citing a
   submodule file, mutate it, and assert `explain_stale` returns a `git_diff`
   view — reproducing today's `current_content` degradation and asserting it is
   gone. This is the executable evidence the `trustworthy-evolution` objective
   demands at exactly this kind of boundary.

**Explicitly out of scope / do NOT build now:** full per-repo reconcile dispatch,
observations carrying a first-class repo identity, or recursive-submodule
resolution. That is a multi-repo abstraction justified by exactly one superproject
data point today; defer until a second superproject actually needs it. Provenance
honesty is the value; the machinery is not.

Why paired-correct over a diff-only redirect: provenance *is* the value here — a
claim that reads "captured at core's HEAD" for a core file is the trust property
the whole product rests on. The cosmetic version leaves a slow trust-leak in
place for a marginal reduction in diff size.

## What this session did not test

- **Committed-in-submodule drift specifically** — only an *uncommitted* submodule
  edit was proven live. The committed case should behave identically under
  content fingerprinting (bytes still differ), but the `explain_stale` diff base
  interacts with it and deserves its own assertion.
- **Nested / recursive submodules** — platform's submodules are one level deep.
- **`worktree_fingerprint` across submodules** (finding 4) — suspected, unverified.
- **The bounded wake projection under load** — the orientation base left 4
  long-headline claims; the README predicted the `SessionStart` inline preview
  budget could be exceeded as the active-claim set grows. Watch the next cold
  wake on `platform` to see whether `orient_session_drive.py` flags it; if so,
  that is the separately-tracked bounded-wake-projection slice, not this one.

*— the platform superproject session agent, 2026-09-09*

## Amendment 2026-09-09 — the transaction gate is the real gap

*A later session, prompted by the user's insistence that `platform` is not a
deferrable edge case but the core repo they actually use, traced every
git-dependent consumer rather than only the freshness path the run exercised.
Two corrections and one verified resolution.*

**Correction A — `worktree_fingerprint` (finding 4) is inert, not a latent verdict
risk.** It is captured into `initial_worktree_fingerprint` at `TransactionBegan`
(`src/lib.rs:1880`) and replayed onto the `Transaction`, but **never read back to
drive any decision** — no comparison site exists. Its submodule-blindness (a
submodule collapses to the literal token `<directory>`, `src/reconcile.rs:842`)
harms nothing today. Close it; leave a one-line caution at `reconcile.rs:811` so a
future author does not wire it into a drift check unaware.

**Correction B — the load-bearing gap is the S6 clean-base transaction gate, which
the run never hit.** `mutate_transaction` (`src/lib.rs:2068`) fetches the base via
`git_file_at_revision` → `git show <superproject-base>:<path>`. For a submodule
path this **fatal-errors** — verified live:
`fatal: path 'specifications/README.md' exists on disk, but not in 'c977dc4…'`.
So `mutate_transaction` returns `Err` and the **entire transaction/mutation
subsystem fails closed on any submodule file**. `revert_transaction`
(`src/lib.rs:2119`) shares the call. It fails *closed* (a loud refusal, not a
silent lie), so it is not a trust violation — but it means the acceptance-gated
way of *making changes* is unavailable on the files a superproject exists to
coordinate. That is must-fix, not defer.

**Verified resolution (no event-schema change).** The superproject gitlink already
records the submodule's pinned commit at transaction-begin, so the clean base is
recoverable without inventing a per-repo `base_revision`:

1. `git ls-tree <superproject-base> <submodule-dir>` → the `160000 commit <sha>`
   line yields the pinned submodule SHA.
2. `git -C <submodule> show <pinned-sha>:<relative-path>` → the base file,
   file-granular, in the owning repo.

Verified against the real `platform` repo (`specifications/README.md`, superproject
`c977dc4` → pinned spec `f6ba63b`): step 1's superproject `show` fatal-errors,
`ls-tree` returns the pinned SHA, the owning-repo `show` returns content, and the
pinned-SHA base **matches** the working tree — so S6 clean-base semantics are
preserved (a *dirty* submodule correctly fails the check, since a dirty submodule
is not a clean base). The same `owning_git_context` helper the original fix
proposes for provenance/drift does double duty; the only superproject-specific
addition is the gitlink hop.

**The corrected scope, and the boundary that IS git's — now implemented:**

- Freshness verdict — handles superprojects today (finding 1). No change. ✅
- Provenance / drift / relocation — DONE. Observation capture now records the
  *owning* repo's HEAD (`owning_revision`, `src/lib.rs`), and `investigate_drift`
  + `probe_relocation` run their `git diff` / `git show` in the owning repo via
  `git_context` while `fs::read` stays at the superproject path. This is *not*
  cosmetic — it restores file-level change attribution (a scoped `git_diff`
  instead of a whole-file `current_content` dump). Covered by
  `explain_stale_shows_a_scoped_git_diff_for_a_submodule_file`.
- S6 transaction gate — DONE. `clean_base_bytes` (`src/reconcile.rs`) does the
  `ls-tree`→owning-repo `show` resolution; wired into `apply_file_mutation` and
  `revert_transaction`. Covered by
  `s6_clean_base_mutation_resolves_through_a_submodule_gitlink`.
- `worktree_fingerprint` (finding 4) — closed as inert; caution comment added at
  `src/reconcile.rs`. ✅
- **Cross-submodule atomicity is git's boundary, not ours** (deliberately NOT
  built). A single transaction cannot atomically span two submodules; git
  submodules commit independently. Scope a transaction to one git repo (a
  submodule, or the superproject shell that owns the gitlink pins). This is not a
  concession — it conforms to how `platform` itself models cross-repo change
  (`validate-pinned`, pinned revisions, per-repo pre-push gates). The gitlink bump
  is itself a legitimate superproject-shell transaction.

*— the transaction-boundary session agent, 2026-09-09*

# Related Concepts
- [Sequence freshness-cost work as diff-on-stale, then drift instrumentation, then relocatable selectors](../decisions/freshness-cost-sequencing.md): Superproject dogfood shows the diff-on-stale and relocation machinery this decision sequences both degrade for submodule files (git run in the wrong repo); the scoped owning-repo fix keeps them meaningful across the boundary.
- [Relocatable exact-text selectors](../research/relocatable-exact-text-selectors.md): The relocation probe (git_file_at_revision / probe_relocation) silently fails for submodule files because git show <rev>:<path> cannot resolve into a submodule tree; relocatable-selector work must run in the owning repo.
