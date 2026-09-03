# Design note — semantic write API (plan, not yet built)

*Written during the foreign-dogfood run on `~/Experiments/plot`, after fixing the
`--workspace` in-repo footgun. Assesses the thinnest change that closes the
write-loop gap both adapters share. Anchored to the CLI write verbs in
`src/main.rs` (`observe`, `claim`, `supersede-claim`, `bind-objective`,
`checkpoint`) and the `Workspace` methods behind them (`focus_observation`,
`record_claim`, `supersede_claim` in `src/lib.rs`).*

## The gap, restated

The **read** side is first-class in the adapters — Pi exposes `workspace_status`,
`_delta`, `_working_set`, … as tools, and the Claude adapter surfaces orientation
on wake. The **write** loop (`bind-objective`, `observe`, `claim`, `checkpoint`,
`supersede-claim`) has *no tool in either adapter*. An agent that wants to assert
a belief must shell the raw CLI, guided only by prose in `SKILL.md`. Two failure
modes follow, and we have already hit both:

1. **Drift.** Prose about flags rots against the binary → footguns (the in-repo
   `--workspace` split-brain, now guarded in `locate.rs`).
2. **Starvation.** Observations are captured *ambiently* (the Read hook), but
   claims are *deliberate, two-step, and invisible*. So observations pile up and
   claims almost never get made. Dogfood evidence: plot sat at ~22 observations
   to 1 claim. A freshness signal with almost nothing to be fresh *about* cannot
   earn trust — which is the entire experiment.

## The load-bearing realization: fuse observe+claim into one intentional verb

The fix is **not** to expose all ~8 write verbs as tools — that is the "generic
wrapper API" the charter forbids. It is to notice that the kernel's mechanical
vocabulary (`observe` a path → get an id → `claim` citing that id) is not the
agent's vocabulary. The agent's actual cognitive act is a single thing:

> *"I now believe **X**, and it rests on files **Y, Z**."*

Today that is two CLI calls with an id threaded between them. The auto-capture
hook does step one for free and leaves step two — the belief — as manual friction.
**That split is the starvation mechanism**, not a coincidence beside it. Collapse
it into one verb and the belief, not the file-read, becomes the unit the agent
records.

Crucially, the fused verb should **consume the ambient observation, not duplicate
it**: if a fresh observation of `Y` already exists (from the hook), `record-belief`
reuses it; otherwise it creates one. That is what wires the ambient *sense* to the
deliberate *belief* instead of running two disconnected ledgers.

## Proposed surface — four intentional verbs, not eight mechanical ones

- `record-belief --statement "..." --rests-on <path> [--rests-on <path>] [--scope ...] [--supersedes <id>]`
  — fuses `focus_observation` + `record_claim` (+ `supersede_claim` when
  `--supersedes` is given) into one **atomic** kernel operation. Reuses a fresh
  existing observation per `rests-on` path. Returns the claim with its freshness.
  **This is the core verb; everything else is secondary.**
- `set-objective --intent "..."` — declares/switches the goal, auto-checkpointing
  the prior one so the transition is recorded, not overwritten (fuses
  `checkpoint` + `bind_objective`).
- `checkpoint --label ... [--note ...]` — unchanged; the resume boundary.
- Reversible-experiment machinery (`begin-transaction`/`apply`/`revert`) stays out
  of this surface for now: it is an advanced workflow, not the core belief loop.

These are **kernel operations, not adapter porcelain.** The fusion/atomicity/reuse
logic lives behind the binary (matching "the binary owns every capture decision");
the CLI and any future MCP adapter are thin mappings onto the same verbs. That is
what keeps this adapter-agnostic instead of re-introducing drift one layer up.

## Thinnest falsifiable first cut

Build **one** verb — `record-belief` — via the CLI only. Defer `set-objective`
fusion, `--supersedes`, and MCP until it proves out.

- **Task:** a foreign-repo slice in which the agent must form three beliefs about
  code it reads.
- **Teeth (mechanical):** each `record-belief` must (a) surface in `status` as a
  claim, and (b) reuse-or-create an observation such that an out-of-band edit to a
  `rests-on` path turns the claim `stale` — reusing the S1 staleness machinery.
- **Measure:** claim count and raw-CLI invocation count vs. the `observe`+`claim`
  baseline. **Pass** = at least as many claims land, with fewer calls and no
  split-brain-class error.
- **Fail-guard:** if `record-belief` records a claim whose `rests-on` observation
  is *not* reused when a fresh one exists, it fails — that would re-fork the two
  ledgers the verb exists to join.

## Minimal kernel work this implies

1. A `record_belief` method on `Workspace` composing `focus_observation` +
   `record_claim` in one event-sourced transaction, with observation reuse.
2. A `record-belief` CLI verb over it.
3. (Deferred) `set-objective` fusion; `--supersedes`; then an MCP adapter.

## Where this leaves MCP

`record-belief` is necessary **regardless** of transport — the MCP adapter would
expose this same verb, so building it CLI-first is not throwaway. It fixes *drift*
and *friction-starvation* everywhere. What it does **not** fix is
*invisibility-starvation in the Claude adapter specifically*: hooks cannot offer a
callable tool, so until MCP, a Claude agent still only learns the verb from the
skill. That is the honest split. **Trigger to build the MCP transport:** a second
machine needing the shared store, **or** measured claim-starvation that persists
in Claude sessions *after* `record-belief` ships. Not before — build the mechanism
when the evidence demands it.

## Why this is the right first cut

It reuses the existing model instead of growing it (compose, don't add axes). It
honors the charter by exposing the agent's *intent* (a belief) rather than the
kernel's *mechanics* (observe-then-claim), keeping the surface semantic and small.
And it attacks starvation at its actual cause — the observe/claim split — with the
smallest thing that can be **false**: one verb and a claim-count.
