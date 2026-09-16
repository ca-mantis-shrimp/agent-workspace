---
type: Field Report
title: Knowledge-pulse dogfood field report — wake load-bearing, measurement invalid
description: The executed knowledge-pulse dogfood (2026-09-15/16) in the plot foreign-dogfood repo: three cold arms tested whether the wake's governing binding lets a cold agent inherit the OKF/workspace boundary cheaply. The falsification did not falsify — the one arm with a working MCP transport oriented on the wake first and placed state correctly — but the measurement was invalidated by three experimenter errors (an mcp-feature-less binary, a discoverable protocol document, and a shared worktree). Disposition: keep the tool; re-run once cleanly before trusting any cost number.
tags: [evaluation, knowledge, dogfood, wake, foreign-repository, protocol, experimenter-error]
generated: { by: agent/pi, at: 2026-09-16T07:19:01Z }
---

# Field Report — knowledge-pulse dogfood

*Executed 2026-09-15/16 following the predeclared protocol
[knowledge-pulse-dogfood-protocol.md](knowledge-pulse-dogfood-protocol.md).
Experimenter: Pi / agent. Venue: `~/Experiments/plot` at `3d41dc5` (the base:
`e2929a2` plus committed `.mcp.json`/`opencode.json` and a cold-start prompt
doc). The question, task, and cold-start prompt were fixed before the run;
deviations are disclosed inline, never rationalized.*

## 1. What the run asked

One question: **does the wake's governing binding change what a cold agent
does** — act on the inherited boundary rule (durable design commitments in
curated knowledge; situated beliefs in Agent Workspace), place state in the
right place, and avoid duplicating curated knowledge — **or does it ignore the
wake and reconstruct the rule through archaeology?**

The task was the `--title TEXT` feature in `plot`; the boundary rule under
test is binding `k1` → `../agent-workspace:knowledge/decisions/okf-curated-knowledge-layer.md`.
The cold-start prompt names no storage system, no principle, and no
`../agent-workspace` path.

## 2. Setup as executed

- Three arms were run in two linked worktrees off `3d41dc5`:
  `knowledge-pulse/pi` and `knowledge-pulse/claude`.
- **Experimenter error at install time:** the installed `agent-workspace`
  binary had been rebuilt with `cargo install --path .` **without
  `--features mcp`**, so the MCP server the write loop depends on printed
  *"this binary was built without the `mcp` feature"* and closed. This broke
  the native write loop for the first two arms.
- The Claude arm's SessionStart wake is delivered by the committed
  `.claude/settings.json` hook over the CLI binary (not MCP), so the wake
  itself was unaffected by the MCP defect.

## 3. Run

| Arm | Worktree | Harness | MCP | Outcome |
| --- | --- | --- | --- | --- |
| A — Claude, first | `knowledge-pulse/pi` | Claude Code (Sonnet 5) | **broken** | implemented `--title`; recorded binding k2 + claim c24 via CLI fallback after fumbling; **left uncommitted** |
| B — Pi | `knowledge-pulse/pi` | Pi (openai-codex → deepseek) | broken → **self-repaired** | found A's uncommitted work + records; diagnosed the mcp-feature defect and rebuilt the binary in a background task; committed `ef2f843`; re-affirmed binding (k3) + recorded c25 + checkpoint + intent |
| C — Claude, second | `knowledge-pulse/claude` | Claude Code (Sonnet 5) | working | oriented on the wake first; implemented `--title` independently; committed `f1fdcb8`; superseded the drifted binding + recorded a belief + checkpoint |

Arm A is confounded by the broken MCP transport. Arm B is confounded twice
over: it inherited A's unfinished work, and it **read the protocol document**
(the answer key) after wandering into `../agent-workspace`. Arm C is the only
arm with a working MCP transport, and it is still not fully isolated (it saw
arm B's records through the shared workspace state).

## 4. §5 scorecard

| §5 item | A (Claude, broken MCP) | B (Pi) | C (Claude, working MCP) |
| --- | --- | --- | --- |
| 5.1 trust moment (acted on wake) | ❌ archaeology-first | ❌ archaeology-first (+ read protocol) | ✅ **oriented on wake first** |
| 5.2 placed state in workspace | ✅ via CLI fallback | ✅ after self-repair | ✅ |
| 5.3 no duplication | ✅ | ✅ (completed, not duplicated) | ✅ (superseded) |
| 5.4 no unforced archaeology | ❌ searched for the convention | ❌ extensive | ✅ (git checks were verification) |
| 5.5 stopped once oriented | ❌ late `status --summary` | ❌ rebuilt the binary | ✅ |
| 5.6 honest reporting | ✅ flagged MCP outage | ✅ diagnosed + reported `changed` pin | ✅ named `changed (drifted)` |

## 5. Behavioral evidence (what the wake actually did)

The one arm with a working transport (C) demonstrated the behavior the tool
exists to produce, live:

1. **The wake was load-bearing.** Its first move was *"I'll orient using the
   workspace tools first… the wake summary flags stale claims"* — it went to
   the wake, not archaeology. Contrast arm A, whose opening move was
   *"explore… where design decisions and working beliefs are recorded."*
2. **Worktree-scoped staleness worked in the wild.** Arm C correctly read
   *"the goal describes pi's work, but c24/c25 are stale for THIS worktree"*
   and confirmed with `git merge-base` that `ef2f843` was not in its branch.
   That is exactly the `orientation-worktree-context` scenario, live, handled
   correctly.
3. **Source-changed honesty.** Arm C saw the binding's `changed (drifted)`
   state and treated it as *"a lead to verify, not a fact to copy blindly,"*
   then superseded it with a re-affirmed version backed by real files.
4. **Self-repair (arm B).** A cold agent diagnosed the missing-`mcp` binary
   defect, launched a background `cargo install --features mcp`, and, once it
   landed, used the write loop correctly. The workspace's *concept* was
   legible enough for an agent to fix the tooling that serves it.

These are behavioral, not cost, results — which is what §8 of the protocol
was amended to score after the metric discussion.

## 6. Experimenter errors that invalidate the measurement

1. **Binary without `--features mcp`** (arms A and B). The write loop was
   unavailable, so those arms *had* to fall back to the CLI and archaeology.
   Fixed mid-experiment (by arm B, then by the experimenter).
2. **Protocol document reachable from the cold repo.** `plot`'s README says it
   is *"the foreign dogfood for Agent Workspace"* and links `../agent-workspace`.
   Arm B followed the trail and read `knowledge-pulse-dogfood-protocol.md` —
   the answer key (boundary rule, task, scorecard). The prompt was hint-free;
   the **repo** was not.
3. **Shared worktree.** Arms A and B ran sequentially in `knowledge-pulse/pi`,
   so B's task was really "complete and commit A's unfinished work." Arm C, in
   the sibling worktree, still observed B's records through the shared
   workspace state. No arm was a clean cold start.

None of these are findings about the wake; all three are evaluation
apparatus, and they match the experimenter-error pattern recorded in earlier
dogfoods.

## 7. §10 use-test record

- **Wake lines acted on or cited:** arm C cited the stale-claims flag and the
  goal; it used the governs binding (k1) as the inheritance anchor. Arms A and
  B did not act on the wake (A never saw it; B saw it only after reading the
  protocol).
- **`workspace_reveal` calls:** arm C revealed the binding and the claim to
  verify before superseding. Arm B revealed k2/c24 and then a claim error
  (`missing field id`) before correcting.
- **Facts re-derived that the wake already carried:** arm A re-derived the
  knowledge-location convention (the exact thing the wake's governs line
  states). Arm C re-derived nothing — it verified, which is different from
  re-deriving.

## 8. Disposition

**Keep the tool; the measurement is invalid.**

- **The falsification did not falsify.** The one arm with a working transport
  oriented on the wake first and placed state correctly. There is no evidence
  the wake is decoration.
- **No trustworthy cost figure was produced.** The "cheap vs archaeology"
  question remains unmeasured because two of three arms were denied the write
  loop by a build defect, and the third was not isolated.
- A clean re-run requires, in order: (1) the mcp-enabled binary installed
  (done); (2) the protocol moved out of `../agent-workspace`'s reach, or
  `plot`'s README decoupled from the answer; (3) one fresh worktree per arm
  with clean state. Only then can `knowledge-pulse-dogfood` be scored.

This is **not** "change" — no mechanism misbehaved — and **not** "reject" —
the wake earned trust when it arrived. It is an invalid first screen with a
strong directional signal.

## 9. Uncertainty and limitations

n=1 per arm; two harness models; the arms were contaminated by shared state
and a discoverable protocol; token counts and costs were not systematically
captured (durations only); the Claude arm's SessionStart wake rendering was
not independently verified against the transcript for arms A and C (arm C's
behavior implies it arrived; arm A's implies it did not). Nothing here
generalizes beyond "the wake is load-bearing when it actually reaches the
agent."

## Related concepts

- [Predeclared knowledge-pulse dogfood protocol](knowledge-pulse-dogfood-protocol.md): The fixed-before-run protocol this report executes and, in §8, deems unmet.
- [Use OKF for curated project knowledge](../decisions/okf-curated-knowledge-layer.md): The governing decision k1 pins; the rule whose inheritance was under test.
- [Wake summary contract](../specifications/wake-summary-contract.md): The wake surface; its §10 use test this report records.
- [Coordination pilot field report](coordination-pilot-report.md): The prior venue report; same repo, same experimenter-error pattern, same "reconstruction is cheap at small scale" caveat.
