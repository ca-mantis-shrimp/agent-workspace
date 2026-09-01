# Response to “The Agent’s Perspective” (2026-09-01)

*This is a companion critique of
[`design-note-agent-perspective.md`](design-note-agent-perspective.md), not a
synthesis that supersedes it. The disagreement is retained because it should
inform the executable contract.*

## Position

The note identifies the project’s most important product risk and gives its
strongest account of purpose:

- attention, rather than file access, is the scarce resource;
- useful capture must ride ordinary work instead of becoming a bookkeeping
  ritual;
- an optional workspace can fail if raw tools remain the easier critical path;
- the buildable freshness core must be separated from the speculative
  cognitive “brake.”

Those claims should change the emphasis of the contract. They do not require
discarding the coordination model. Provenance, restart recovery, findings,
evidence, and transaction boundaries are the machinery that can make bounded
attention and honest freshness enforceable rather than aspirational.

A better center for the MVP is:

> **A bounded epistemic substrate that captures observations during ordinary
> work, distinguishes what is current, stale, or unknown, and lets an agent
> act precisely while ingesting less context.**

## Where the disagreement is narrower than it appears

The Neovim analogy should not be interpreted to mean that agents share human
spatial, reading, or editing limitations. It names desirable properties: durable
orientation, progressive disclosure, immediate feedback, and reversible action.

The proposed outline-to-module-to-symbol flow, bounded symbol reads, and
semantic working set are useful precisely because an agent has no persistent “in
place.” They reduce the material that must enter context and retain the reason a
location mattered after that material leaves context. They should be evaluated
by context cost and stale-reasoning reduction, not by resemblance to an editor.

The analogy is therefore a design aid, not the product thesis. If it causes
human affordances to be copied without evidence, it should be abandoned.

## Naturalness without eliminating intent

“The tool should have no verbs of its own” is directionally right but too
absolute. A more useful rule is:

> **The workspace should require no bookkeeping-only actions.**

Reads, edits, diagnostics, validation, and revision changes should automatically
produce the coordination records needed to assess freshness. The agent should
not have to remember to say “record that I read this” or “invalidate what
changed.”

Some workspace operations nevertheless carry intent that cannot be inferred
safely:

- beginning a change and naming its purpose;
- accepting or disposing a finding;
- declaring which claim a validation check supports;
- checkpointing or handing off incomplete work;
- accepting, reverting, or abandoning a transaction.

These are not clerical duplication. They establish semantic and safety
boundaries. The design should minimize them and place them on the critical path,
not pretend they can all be reconstructed from filesystem activity.

## The bypass problem requires an honest coverage model

Raw Read/Edit access creates incomplete observation. Incomplete observation is
dangerous only if the workspace presents silence as assurance.

The contract should use at least three epistemic states:

- **Current** — the record is supported by tracked inputs whose relevant
  fingerprints still match.
- **Stale** — a previously supporting input changed or relocation no longer
  resolves unambiguously.
- **Unknown** — the workspace lacks enough tracked information to make either
  claim.

“No stale warning” must never imply “current.” Current is a positive,
reproducible claim. Raw or out-of-band operations may leave related records
unknown, reduce a declared coverage boundary, or cause conservative
invalidation. The status projection must expose that loss of coverage.

This makes optional adoption coherent, although not necessarily useful. The
practical adoption test remains: why would an agent choose a workspace-aware
operation over raw access? The initial answer should be concrete utility—smaller
reads, semantic resolution, retained provenance, automatic freshness, and
restart recovery—not moral pressure to use the correct tool. Harness-level
mediation may later improve coverage, but the MVP should not depend on replacing
every native operation.

## Observation is not belief

“Memory that knows when it is lying” is an effective north star but too strong
as an implementation claim. A span fingerprint can establish that the bytes
supporting a recorded symbol observation changed. It cannot establish every
dependency of an inference drawn from that observation.

The contract should distinguish three layers:

1. **Observation** — automatically captured provider output or bounded source
   material, with reproducible input fingerprints.
2. **Claim** — an interpretation asserted by an actor, with declared supporting
   observations and an explicit or conservative dependency scope.
3. **Evidence** — the outcome of a named check, bound to its exact invocation
   and relevant inputs, supporting stated acceptance claims.

These layers can have different invalidation precision. Automatic observations
may be cheap and coarse. Claims may require deliberate dependency declarations.
Evidence may use command-, target-, file-, or repository-level fingerprints. The
workspace should report the reason and scope of every freshness verdict and
admit when it cannot exclude an untracked dependency.

That is more honest than suggesting that semantic anchoring alone provides
belief-level truth. False negatives are more dangerous than conservative
invalidation; excessive conservatism, however, will create alarm fatigue.
Dogfooding must measure both.

## The brake as policy rather than simulated affect

The proposed sanctioned brake points to a real asymmetry: agents are strongly
rewarded for continuing and weakly supported in checkpointing, narrowing scope,
or handing off. But a signal cannot acquire motivational force merely by being
named *hazy* or *overwhelmed*.

The buildable version belongs partly in the harness and partly in workspace
policy:

- bounded context and tool-output budgets;
- visible unresolved findings and unknown coverage;
- invalid-evidence gates before accepting a transaction;
- checkpoint and handoff as successful lifecycle outcomes;
- explicit residual-risk recording;
- recommendations—or, where authority permits, hard gates—when required
  evidence is stale or the transaction has drifted from its objective.

These mechanisms can be tested. Whether richer cognitive-state signals improve
agent behavior remains a research question. The MVP should preserve room for
that work without claiming to manufacture affect.

## The “thin wrapper” question

The first implementation may be Git, tree-sitter, retained tool results, and
careful joins. That is not a failure. Algorithmic novelty is not the acceptance
criterion.

The difficult product work is to preserve native authority while composing:

- bounded perception;
- semantic identity across change;
- explicit freshness and unknown state;
- intent and evidence;
- restart-safe recovery;
- low-friction use on the critical path.

If that composition does not measurably change behavior, it has not earned
continuation regardless of architectural sophistication. If it does, beginning
with ordinary primitives is an advantage.

## Consequences for the executable contract

The `contract` action should incorporate the following requirements:

1. Define `current`, `stale`, and `unknown`, including who may assert each state
   and from which evidence.
2. State that absence of a warning never establishes freshness.
3. Separate observations, claims, and validation evidence, with distinct
   dependency and invalidation semantics.
4. Define a visible coverage boundary for operations observed by the workspace.
5. Include raw and out-of-band reads, edits, and revision changes in failure
   scenarios.
6. Require automatic capture where native operations cross the workspace, while
   retaining explicit verbs only for genuine intent or lifecycle boundaries.
7. Make bounded output and context cost part of the API contract, not an
   optimization deferred to evaluation.
8. Treat checkpoint and handoff as valid successful outcomes rather than
   incomplete failure states.
9. Measure false-current verdicts, conservative invalidations, repeated reads,
   bytes or tokens ingested, reconstruction after restart, and bookkeeping
   overhead.
10. Keep cognitive proprioception and motivational braking as documented
    research questions, not MVP guarantees.

The walking skeleton should therefore prove not merely that a symbol observation
becomes stale after an edit. It should also show that unobserved activity yields
`unknown`, that a fresh agent can recover a bounded account of why the symbol
mattered, and that workspace-assisted operation consumes less context than the
raw alternative without hiding provider detail.

## Synthesis

Build the smallest honest freshness core, but do not amputate the coordination
boundaries that make it trustworthy. Avoid an infrastructure platform in search
of a use case; equally, avoid reducing the project to a cache invalidator that
cannot represent intent, evidence, or uncertainty.

The durable test is behavioral:

> Does the workspace let an agent inhale less, retain the right relationships,
> recognize when support has decayed, and stop or hand off without pretending
> that incomplete knowledge is current?
