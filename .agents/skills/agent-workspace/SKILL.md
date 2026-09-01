---
name: agent-workspace
description: Use when doing multi-step work in this repository where you form beliefs about code that can silently go stale — especially when resuming cold with little or no chat history. Externalizes your beliefs as observations and claims so the workspace gives you a freshness signal instead of quiet staleness.
---

# agent-workspace: your freshness prosthesis

This tool exists because you have no proprioception for staleness. You read a
file, form a belief, act on it forty steps later — and nothing tells you the
bytes changed underneath you at step twenty-five. The workspace is that missing
signal. It only knows what you explicitly tell it, and it only earns its keep if
you actually consult it.

## Prime directive

**When the workspace reports a claim as `stale`, believe it over your own memory.**
Re-observe the underlying file before you act. The entire point is that it sees
changes your context cannot. A false `current` from you is worse than an honest
`stale` from the tool.

## Orient first (especially cold)

Before anything else, run `status`. It reconstructs the objective, observations,
claims, evidence, and open transactions from durable records — no chat history
needed. Read `freshness_within_scope` on every claim:

- `current` — supporting inputs unchanged since you recorded them. Trustable.
- `stale` — an input changed. Re-observe before you rely on it.
- `unknown` — an input couldn't be verified. Treat as not-yet-trusted.

```
agent-workspace status --repository . --workspace .agent-workspace
```

## The loop

1. `bind-objective --intent "..."` — declare what you're doing.
2. `observe --path <file>` — record that you looked at a file, *before* forming a
   belief about it. Returns an observation.
3. `claim --statement "..." --observation <id> --scope declared` — assert the
   belief, citing the observations that back it. Add `--dependency <path>` for a
   file the claim depends on but you didn't directly observe.
3b. When a claim is genuinely outdated — not merely drift-stale — replace it:
   record the successor claim, then `supersede-claim --id <old> --claim <new>
   --reason "why"`. Supersession is for decisions that no longer hold or
   assessments that were consumed, never for input drift (that is what
   `stale` is for). Superseded claims leave the active projection but stay
   readable history with their replacement link and reason.
4. `status` — check freshness *before acting* on any claim. Distinguish
   `claims` (live beliefs) from `superseded_claims` (retired history): a
   superseded claim is not evidence of anything current, even when its recorded
   freshness says `current`.
5. To make a reversible clean-base experiment: `begin-transaction --claim
   <id>`, then `apply --id <tx> --path <file> --content "..."`, and use
   `revert-transaction` when needed. Post-mutation acceptance is not yet a sound
   workflow: descriptive claims become stale after the owned edit, and the
   separate acceptance-criterion model is still pending.

## Scope honesty

`--scope declared` records exactly the paths and dependencies you supplied; it
still reports completeness as `not-asserted`. If a claim leans on helpers you
cannot enumerate, use `--scope conservative-siblings` to widen the net, or
record dependencies explicitly. In every case, read `scope_assurance` alongside
freshness: an honestly narrow claim beats a falsely confident one.

## Gotchas

- **IDs are zero-indexed.** Read the `id` from the returned JSON — do not guess
  sequential 1-based ids. A wrong id fails hard.
- `--repository` and `--workspace` are required on every call. `--repository` is
  the repo root; `--workspace` is the durable state directory (reuse the same one
  across sessions — that's what makes cold resume work).
- Build first: `cargo build`, then use `target/debug/agent-workspace`.
