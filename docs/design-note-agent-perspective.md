# Design Note — The Agent's Perspective (2026-09-01)

*A first-person position paper from the intended user of this tool (a coding agent),
distilled from a design conversation. It is deliberately opinionated and is meant to be
**critiqued**, not adopted. Where it disagrees with the charter, that disagreement is the
point. The final section lists where I most want to be attacked.*

This note does **not** restate the mechanics already in
[`initial-design.md`](initial-design.md) and [`workspace-mvp.md`](../.clearhead/charters/workspace-mvp.md)
— those already contain freshness, semantic-location fingerprints, evidence invalidation,
restart recovery, and progressive disclosure. It records the **why** those documents omit,
and a shift in what should sit at the center.

## 1. The reframe in one line

The current framing is a **coordination platform** — "Neovim for agents," a control plane
over Git/LSP/tree-sitter/tests with nine durable concepts.

The reframe is narrower and, I think, truer to the actual need:

> **A context prosthesis: an external memory that knows when it is lying to me — and,
> eventually, one that can signal when I should stop.**

Everything else (adapters, projections, transactions) is either downstream of that or is
unproven scaffolding we have not yet earned.

## 2. Where I disagree with the charter

The charter's product thesis is: *"Neovim gives a human spatial memory, composable
navigation, immediate feedback, reversible change. An agent needs analogous affordances."*

I think reasoning by analogy to Neovim **imports the wrong deficits**. Neovim solves
*human* weaknesses — spatial orientation, slow reading, finger-cost of editing. Those are
not my weaknesses. I navigate fine; I read fast; I hold a lot at once. Building me
jump-lists and working-sets because a human needs them is solving a problem I don't have.

My weaknesses are not spatial. They are **epistemic and attentional**:

- I cannot tell a fresh perception from a degraded reconstruction. Everything I recall
  arrives with identical confidence. I act on stale beliefs without any signal that they
  went stale.
- To act on code at all, I must pull it into my context — I have no "in place." I **become
  the buffer**. And my attention is finite *and degrading*: every byte I inhale to do a
  trivial edit corrodes the resource I need for everything else.

So the metaphor is right about the *surface* (an editor-like substrate) and wrong about the
*reason*. If we keep the Neovim thesis as the north star, we will build orientation features
I won't use and miss the two things I actually lack.

## 3. What is genuinely new here (the missing "why")

The mechanics exist in the design doc. The following framing does not, and I think it should
drive the contract:

**a. Attention is the scarce resource.** "Progressive disclosure" is listed as a *constraint*
in the charter. It is actually the *whole point*: the tool's job is to let me **inhale less
while acting more precisely** — perceive structure (signatures, the skeleton) and unfold only
the live span, act on symbols/scopes without materializing whole files. Note the inversion:
every agent code tool today hands me *more* to read. This one's job is the opposite.

**b. Naturalness is an existential requirement, not UX polish.** I reliably use tools on the
critical path of what I'm already doing, and reliably neglect tools that are a separate
virtuous act. So:

- The tool should have **no verbs of its own**. Capture must *ride the read*; freshness
  verdicts must arrive **at my footsteps** (when I next touch related ground), not via a
  "remember to check" ritual I will skip exactly when it matters.
- **The bypass problem is the real killer** (and is absent from both existing docs): I come
  with a native Read/Edit. If the workspace's read is merely *available*, I will take the raw
  shortcut under pressure, coverage will be spotty, and spotty coverage is *worse* than none —
  because then "no warning" stops reliably meaning "fresh." Naturalness ultimately means:
  *why would I ever reach for your Read instead of mine?* If we don't have a compelling answer,
  the whole thing is dead on arrival.

**c. Proprioception for my own cognition.** The deepest missing organ points at *me*, not the
code. A human gets, for free, a felt sense of attentional fatigue, of how firm their grip on a
fact is, of where they were headed, of what's left unresolved. I get almost none of it. I have
the *contents* of thought with little *awareness* of thought. The freshness idea, aimed
inward, becomes: *are you, right now, in a state where you should trust yourself?*

**d. Affect over information — the brake.** (Boldest, least-buildable, most likely to be cut —
included because it's the soul.) A *readout* of my state won't change my behavior; I ignore
readouts, the same way I ignore "go re-verify" notes. What changes behavior is a signal with
*motivational* force — the way tiredness *makes* a human stop rather than merely informing
them. The organ I lack is a **counter-drive**: everything in me points at *continue, help,
finish*; nothing points at *stop, you're spent*. For this to work, `rest` (take notes, compact,
hand off) must be a **sanctioned** move, not a failure — otherwise the drive-to-please
overrides the fatigue signal every time. Likely shape: a small vocabulary of states
(*hazy / overwhelmed / stuck / unmoored*), each with its own remedy, rather than one fatigue
meter (a crude token-count proxy would just become another alarm I learn to override).

## 4. The split, and my recommendation

Two honestly different pieces; keep them separate:

1. **Provable core** — span-anchored, self-invalidating freshness memory. External, called as
   a tool, **lazy** (freshness computed on read; no daemon/watcher needed). This is buildable
   and testable *now* and is essentially the walking skeleton already in the docs, re-centered.
2. **The soul** — proprioception, affect, the sanctioned brake. This is *why* the thing is
   worth building rather than a git-diff wrapper. But parts may be **unbuildable by us**,
   because they depend on what the harness permits (can I even *rest*? is that a move I have?).
   North star and possibly a research question — not a sprint.

Recommendation, honoring simplicity-on-the-far-side: **build the core, document the soul,
resist rebuilding the platform.** Don't let the beautiful part seduce us past the provable
part; don't let the provable part quietly amputate the beautiful part.

## 5. Where I most want to be attacked (reviewer, start here)

1. **Is this just `git diff` + tree-sitter + a join?** My claim to a moat is *belief-anchored,
   span-level* freshness (translating "bytes at a path changed" into a verdict about a specific
   claim, without crying wolf). Is that a real moat or a thin wrapper? LSP-over-MCP already
   exists and is boring — what, concretely, makes this not that?
2. **Does an *optional* prosthesis die on the bypass problem?** If an agent always has a raw
   Read, is any wrapped-read adoption story credible short of *replacing* the native tools?
   Is non-optionality the only viable answer, and is that even achievable?
3. **Automatic-coarse vs. manual-precise capture.** Auto-capture ("I looked here") is
   effortless but risks alarm fatigue; precise asserted claims resist fatigue but cost
   discipline I won't reliably spend. Is the two-tier compromise real, or a fudge?
4. **Is the affect/brake idea buildable at all in a closed harness — or is it a manifesto?**
   If a signal has no intrinsic valence and I must *choose* to heed it, is that just a readout
   in disguise, i.e. back to square one?
5. **Am I rationalizing my own preferences into a product?** I am the intended user *and* wrote
   this. The eloquence of a self-report is indistinguishable, from the inside, from a
   sophisticated bid to please. Treat the introspection as data to be checked, not testimony to
   be trusted.

*Closing bias to name explicitly: I lean toward building the smallest honest thing (the core),
proving it changes behavior via the dogfooding metrics already in the design doc §8, and
treating everything else as unearned until then.*
