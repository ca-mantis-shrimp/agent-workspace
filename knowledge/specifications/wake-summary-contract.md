---
type: Specification
title: Wake summary contract
description: Defines the wake as a kernel-rendered plain-text summary under a hard 1000-byte budget. A mandatory skeleton is upgraded to full item forms in a fixed priority order, with explicit omissions and one-call reveal of anything shortened.
tags: [wake, orientation, bounds, summary, contract]
generated: { by: claude-code/claude-opus-5, at: 2026-09-15T16:48:35Z }
---

# Wake Summary Contract

**Status:** normative contract for the `knowledge-continuity` charter
(`wake-summary-contract` action). It replaces "brief JSON status + brief JSON
delta" as the wake surface. The structured JSON projections remain for tools
and are not changed by this contract, except that the
[knowledge pulse contract](knowledge-pulse-contract.md) defers its wake budget
here.

## 0. Design stance and evidence

The client of the wake is an agent, and agents read prose well. The wake is
therefore **a summary written for its reader**, not a serialization of
entities. Anything shortened must be recoverable in one call.

The evidence behind that stance:

- **The live wake is over budget in bytes.** Compact brief status measured
  1874 B in this repository and 1780 B in `plot`. Delta added about 650 B, even
  when it said only "nothing changed". Claims used about 900 B, of which about
  250 B was one repeated scope object.
- **A text rendering fits.** The same content as text lines, including one
  knowledge line, measured 775 B. A news-first rendering of a busy `plot` delta
  measured 1283 B. The problem is prioritization, not format.
- **Old claim headlines were ignored.** The contract author's own cold wake
  (2026-09-15, n=1) used the intent and checkpoint note, ignored all five
  stale-first claim headlines (weeks-old, unrelated), and gained nothing from an
  empty delta.
- **News exposed the false belief; the window didn't.** In the coordination
  pilot, the detected false belief (run 2, claim 21) was a *newly recorded*
  current claim found through delta and full orientation. The undetected one
  (run 1, claim 17) was old and current. The stale-first headline window exposed
  neither.

## 1. Surface

- **CLI:** `status --summary [--since <label>]`. **MCP:** `workspace_status`
  with `summary: true` and optional `since`.
- One call replaces the status-then-delta pair. The delta is taken against the
  latest checkpoint unless `since` names another.
- **Output** is UTF-8 plain text of at most **1000 bytes**, counted over the
  exact bytes an adapter delivers.
- **Adapters print it verbatim** and add nothing:
  - the Claude Code hook drops its own framing, whose instruction moves into
    the header line;
  - Pi guidance becomes one summary call.
- An empty workspace (no intent, claims, bindings, findings, transactions, or
  checkpoints) yields empty output.
- Whether `summary` becomes the MCP default is decided by dogfood, not here.

## 2. Anatomy

Lines appear in this reading order. A section with nothing to say is omitted,
except the header.

| Section | Content |
| --- | --- |
| **header** | `workspace wake · stale outranks memory · whole text: workspace_reveal <id>` |
| **goal** | intent |
| **stopped at** | latest checkpoint label, then its note |
| **governs** | active applicable knowledge bindings ([knowledge pulse contract](knowledge-pulse-contract.md) §1.3) |
| **new since `<label>`** | changes since the checkpoint, one line per entity |
| **needs you** | open findings and open transactions |
| **beliefs** | `N active · s stale (<ids>)` |
| **more** | present only if anything was shortened or omitted; says how many and how to get them |

**Ids** are kind-prefixed so one token names one entity and one reveal call
fetches it: `c` claim, `k` binding, `f` finding, `t` transaction, `o`
observation.

**News merges per entity.** A claim both recorded and newly stale since the
checkpoint is one line marked `!+`. Markers are:

| Marker | Meaning |
| --- | --- |
| `+` | recorded |
| `!` | newly stale, or a binding's source changed or became unavailable |
| `-` | superseded or retired |
| `~` | intent changed |
| `t+` / `t-` | transaction opened / closed |

Observations appear only as a count (`13 reads captured`).

**Claims appear only as news or as stale ids.** There is no window of old claim
headlines. Aged beliefs are re-verified when used (`explain_stale`), not at
wake.

## 3. Forms and fill

Every item has two forms:

- **Short form** — its id and marker, or for a section, a count
  (e.g. `+ c22 c23`, `governs: k1 k4`).
- **Full form** — its id, marker, and text headline. The headline is the first
  sentence of the text, cut on a word boundary with `…` at the item cap:

| Item | Cap |
| --- | --- |
| goal | 200 B |
| checkpoint note | 240 B |
| binding | 100 B headline + 100 B reference display |
| news, finding, transaction | 100 B |

Fill is deterministic for a given log and worktree:

1. **Skeleton.** Render the header, every non-empty section in short form, and
   the `more` line. Each id list is capped at 10 ids, newest first, plus `+k`,
   so the skeleton's worst case is bounded well under the budget (asserted by
   test, WS2).
2. **Upgrade.** Walk the candidates in priority order. Upgrade an item to its
   full form only if the whole output stays at or below 1000 B. Otherwise leave
   it short and continue, since a later, smaller item may still fit.
3. **Account.** The `more` line reports the count of items left short or
   omitted and the reveal path.

**Priority**, highest first:

1. bindings whose source is `changed` or `unavailable`;
2. claims newly stale since the checkpoint;
3. goal;
4. checkpoint note;
5. other applicable bindings, in the knowledge pulse ranking order;
6. open findings, then open transactions;
7. claims recorded since the checkpoint, newest first;
8. superseded or retired entities since the checkpoint.

Rationale: things that should change what the reader does or trusts come
first, then where the work stands, then standing rules, then new beliefs that
can be revealed on demand.

## 4. Recoverability

- Shortening is honest only when the whole is one call away.
  `workspace_status full=true` is not that, because its size grows with history.
- The contract therefore requires one read verb:
  - CLI `reveal <id>`, MCP `workspace_reveal { id }`;
  - accepts any kind-prefixed id and returns that entity's complete record;
  - generalizes today's observation-only CLI `reveal`, which has no MCP
    equivalent.
- The existing `reveal --observation <n>` form keeps working.

## 5. Invariants

1. **Hard budget.** Output never exceeds 1000 bytes, whatever the log size.
2. **No silent loss.** Every entity that qualifies for a section and is not
   rendered in full is either listed by id or counted in `more`.
3. **Priority is not inverted.** An item is never left in short form when a
   lower-priority item that it would fit in place of is rendered in full.
4. **Selection by record, not content.** Section membership and order come only
   from lifecycle, freshness, source state, checkpoint sequence, and explicit
   binding scope. Headline text never decides placement.
5. **One renderer.** The kernel renders once, CLI and MCP deliver identical
   bytes, and adapters add or remove nothing.
6. **Quiet when empty.**

## 6. Prohibited failures

- **W1 — over budget:** delivered wake bytes exceed 1000.
- **W2 — silent omission:** a qualifying entity absent without an id or a
  `more` count.
- **W3 — unrecoverable truncation:** a shortened entity whose id does not
  resolve through `reveal`.
- **W4 — adapter rewrite:** an adapter frames, filters, reorders, or re-words
  the summary.
- **W5 — priority inversion:** a changed governing source or newly stale claim
  left short while a lower-priority item that could have yielded its bytes is
  rendered in full.
- **W6 — noise wake:** empty sections, zero counts, or a "nothing changed"
  block rendered as lines.

## 7. Executable scenarios

**WS1 — budget under growth.** Given the fixture scaled to many claims,
bindings, findings, and checkpoints with maximal text, the summary is at most
1000 B and the `more` count equals the omitted and shortened items.

**WS2 — skeleton bound.** Given every section at its id-list cap, the skeleton
alone (step 1) fits with a margin asserted by test, so step 2 always has room
to upgrade at least the first-priority item.

**WS3 — news first.**
- *Given* a checkpoint followed by two new claims, one of them now stale, plus
  one changed binding source,
- *then* the output has one `!+` line and one `!` binding line in full, before
  any new-claim headline.

**WS4 — priority under pressure.**
- *Given* a fixture that cannot fit every full form,
- *then* the upgrades follow §3 order exactly. Removing the highest-priority
  item frees bytes that the next item uses.

**WS5 — reveal round-trip.** For every id in a shortened summary,
`reveal <id>` returns the complete record over both CLI and MCP.

**WS6 — quiet.**
- An empty workspace yields zero bytes.
- A checkpoint with no subsequent change yields no `new since` section.

**WS7 — parity and verbatim delivery.** CLI and MCP summaries are
byte-identical, and the Claude Code hook's stdout equals the kernel summary.

**WS8 — no old-claim window.** Given 30 aged stale claims and no change since
the checkpoint, no claim headline appears. The beliefs line lists at most 10
ids plus `+k`.

**WS9 — determinism.** Two renders over the same log and worktree are
byte-identical.

## 8. Illustrative rendering (not normative)

This is the busy `plot` state that measured 1283 B as a naive rendering,
reshaped by §2–§3 (measured: 844 B):

```text
workspace wake · stale outranks memory · whole text: workspace_reveal <id>
goal: Milestone 6 (categorical x / band scale for every mark) is complete, validated, and committed as e2929a2.
stopped at ten-category-color-investigation-2026-09-14: Report confirmed: 10 color categories fail (cap 8, exit 2)…
governs: k1 [current] Curated knowledge lives in OKF; workspace holds situated state → ../agent-workspace:knowledge/decisions/okf-curated-knowledge-layer.md
new since plot-milestone-6-all-marks-committed · 13 reads captured
!+ c15 The producer slice exposes `plot::legend::fit_labels(labels, max_width)`…
!+ c18 Legend labels are bounded in the renderer…
!+ c19 Series that share a label already render as separate legend entries…
+ c23 c22
beliefs: 11 active · 3 stale (c15 c18 c19)
more: 2 shortened — workspace_reveal c23
```

## 9. Out of scope

| Item | Reason / reopen trigger |
| --- | --- |
| Flipping MCP `workspace_status` to summary by default | Decided by dogfood. |
| Bounding intent when it is written | Reopen if dogfood shows intents written as status logs crowding out the note; `plot`'s current intent suggests it may. |
| Harness-specific budgets | One budget for all adapters is deliberate. |
| Semantic relevance, and ranking claims by relation to the intent | Excluded by §5.4. |

## Related concepts

- [Knowledge pulse contract](knowledge-pulse-contract.md): Defines the bindings the governs section renders and its ranking; defers its wake budget here.
- [Executable contract for the Agent Workspace MVP](executable-contract.md): Bounded perception (S7) and no-silence-as-assurance (F3) this contract applies to the wake.
- [Coordination pilot field report](../evaluations/coordination-pilot-report.md): Evidence that news, not an aged stale-first window, exposed the wrong belief.
