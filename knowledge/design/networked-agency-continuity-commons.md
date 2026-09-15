---
type: Design Proposal
title: Networked agency and the continuity commons
description: Owner-accepted direction for a council experiment: temporary evidence-led deliberation, self-organizing execution, and continuity through intentionally published consequences; Clearhead plan integration remains deferred until repeated use demonstrates a need.
generated: { by: agent/cli, at: 2026-09-15T17:40:45Z }
sources:
  - resource: design/agent-perspective.md
  - resource: decisions/coordination-selection-disposition.md
  - resource: decisions/external-workspace-and-clearhead-boundary.md
---

# Networked agency and the continuity commons

**Status:** owner-accepted direction for a bounded council experiment, still
written for independent council review rather than treated as proven
architecture. The owner accepted the protocol principles on 2026-09-15 and
explicitly deferred Clearhead integration: these outputs are mostly plans, so
integration must be earned by recurring friction rather than built in advance.
This authorizes configuration and evaluation of the council protocol, not a
roadmap change or silent reopening of coordination entities cancelled by the
existing selection disposition.

**Accepted experimental protocol:** use an immutable bounded brief, independent
first reports, a mechanically assembled claim matrix, at most one focused
cross-examination, and a decision membrane that separates deterministic facts,
reversible in-mandate choices, unresolved hypotheses, and authority-expanding
choices. The facilitator records and exposes synthesis but does not govern.
Persist the recommendation, evidence, dissent, uncertainty, reopening
conditions, participants, context modes, cost, and owner disposition—not
transcripts or private reasoning. Begin with existing Pi council machinery and
two read-only project profiles; extract harness-neutral or Clearhead machinery
only after observed use demonstrates a need.

## Thesis

A multi-agent system should not assume that technical orchestration requires a
command hierarchy. Mechanical infrastructure may launch, connect, isolate, and
stop agents without becoming their governing authority. A healthier model is a
networked community: a temporary council maintains the best current public
state of the plan, while agents self-organize around that state and shared
standards.

The continuity system should make participation easier without making every
participant continuously legible. It should remember consequential
commitments, discoveries, decisions, and unresolved disagreements—not model all
activity.

> Remember consequences, not behavior.

## Separate governance from execution

A supervisor-led model quietly imports an organizational hierarchy:

```text
human -> supervisor -> workers
```

Process spawning does not justify that authority structure. The proposed split
is instead:

```text
human mandate and constitutional boundaries
                    |
          temporary agent council
       maintains the best current plan
                    |
       shared, versioned intention graph
                    |
         self-organizing community
                    |
      harness-neutral execution machinery
```

The human retains authority over ends, permissions, consequential external
effects, and resource boundaries without becoming the dispatcher for every
reversible step. The council governs plan transitions, not workers. The
execution broker is a servant of the community: it may adapt to Pi subagents,
Claude Code, or another harness, but it owns no planning authority.

Council membership and roles should be temporary and decision-specific. A
particular deliberation may need a proposer, independent critic, domain
advisor, synthesizer, verifier, or minority reporter. Those roles should not
harden into a permanent ruling class.

## Clearhead as intention graph, not activity monitor

Clearhead may legitimately serve as both the intention system and the durable
work graph because child actions and predecessor relationships express
intentions at different granularity. It should not, however, equate execution
attempts with actions.

- A **Clearhead action** is a durable collective commitment.
- An **execution run** is one agent's attempt to contribute to a commitment.
- A **proposal** is a possible transition of the public plan.
- A **council decision** accepts, rejects, or revises that transition.

An agent may privately decompose an action while working. A subproblem earns
promotion into the shared graph when it survives a run, blocks or enables other
work, needs independent acceptance, carries a consequential decision, or
requires human prioritization or authorization. Retries, temporary worktrees,
tool calls, transient hypotheses, and process status remain execution state.

A future cross-harness system therefore need not invent a second durable work
graph. It can take a Clearhead action, permit local expansion, and propose only
the durable part as a graph diff. Clearhead remains optional to the underlying
workspace: it is a linked authority where available, never a required runtime
dependency.

## The continuity commons and its publication membrane

Continuity becomes oppressive when it tries to represent everyone at all
times. The alternative has three zones.

### 1. A small shared root

The durable commons contains only what a future participant needs to continue:

- governing purpose and constraints;
- latest accepted plan version;
- important decisions and their evidence;
- unresolved questions and disagreements;
- acceptance standards;
- latest integrated repository state;
- links for progressive disclosure.

A cold participant should receive a bounded pulse of this state and reveal
details only when needed.

### 2. Local autonomy

An agent's exploration is local by default: private reasoning, scratch notes,
failed approaches, temporary decomposition, native tool calls, tentative
hypotheses, and worktree mutations. The commons does not require heartbeats,
activity feeds, transcript ingestion, productivity scoring, or narration of
every step.

### 3. Intentional publication

An agent crosses the publication boundary when its work becomes consequential
to others: a useful conclusion, tested result, new dependency, proposed plan
change, material disagreement, blocker, impossibility result, integration-ready
artifact, or handoff another participant must inherit.

> The commons sees what participants deliberately contribute, not the whole
> process that produced it.

Provenance remains modest: a record means that an actor reported something; it
does not prove what the actor perceived or authenticate the whole trajectory.

## Council by exception, autonomy by default

Council deliberation earns its cost when evidence may change the shared plan:
ambiguous decomposition, incompatible architectures, overlapping commitments,
consequential integration, invalidated assumptions, stopping decisions, or
requests for more authority and resources.

It should not be required for bounded implementation, ordinary investigation,
local execution choices, reversible experiments, or questions that
deterministic checks can settle. A council is an event convened around a
specific decision, not a standing meeting through which all work must pass.

A proposal should be a compact plan diff:

```text
current plan version
proposed additions, removals, or dependency changes
rationale and supporting evidence
known uncertainty and alternatives
permissions or resources affected
conditions that would reopen the decision
```

The output is a new accepted plan version plus preserved objections—not an
order to individual workers. Community members inspect the graph, select ready
contributions according to capability and expected value, and publish results
that may cause another plan revision.

## Safeguards against network failure modes

A network can coordinate toward error as efficiently as a hierarchy. Council
procedure should therefore resist groupthink and administrative capture:

1. Evidence accompanies plan proposals.
2. Independent criticism occurs before reviewers inherit the dominant answer
   when the decision warrants that cost.
3. Repeated claims resting on one source do not become independent evidence.
4. Material dissent and uncertainty survive the winning synthesis.
5. Deterministic checks outrank votes on deterministic questions.
6. Quorum, budget, and timeout rules prevent deliberation from becoming a veto
   on action.
7. Peer consensus cannot expand the human mandate, permissions, or external
   effects.
8. Honest impossibility, handoff, and stopping within budget count as valid
   outcomes.

Shared standards should govern interoperability and safety—evidence formats,
acceptance gates, permission boundaries, integration conventions, and treatment
of dissent—not prescribe how every agent must reason or sequence its work.

## Anti-bureaucracy acceptance test

The design succeeds when a cold participant can cheaply answer:

1. Why are we here?
2. What is the best current public understanding?
3. What changed?
4. What remains unresolved?
5. Where is the supporting evidence?
6. What decisions may I make autonomously?

It need not answer what every agent is doing, how many tools they used, whether
they followed a preferred process, or how to reconstruct their private
reasoning.

Governance should scale with risk, irreversibility, uncertainty, and contention.
If the system requires collective ratification for a one-file fix, it has
failed. If it cannot challenge an inherited cross-cutting plan, it has also
failed.

## Questions for council review

1. Can a shared plan be maintained without appointing a permanent planner or
   quietly recreating one through the synthesizer role?
2. What decision rule preserves progress and dissent better than either
   unanimity or simple majority?
3. Which plan changes may agents accept autonomously, and which cross the
   human's constitutional boundary?
4. Can Clearhead represent proposed and accepted graph transitions cleanly, or
   should proposals live outside it until ratified?
5. What information must cross the publication membrane for collision
   avoidance without introducing presence and activity surveillance?
6. How can independent criticism survive a shared corpus that makes the current
   consensus easiest to retrieve?
7. What measurable evidence would show that council deliberation reduces error
   or continuity cost rather than adding bureaucracy?
8. What should remain deliberately unknowable to the commons?

The desired shape is neither an obedient swarm nor governance everywhere. It is
free initiative within a visible mandate, a small continuity root, deliberate
publication, and temporary councils at consequential forks.

# Related Concepts

- [The agent's perspective](agent-perspective.md): Extends the constructive collective-agency discussion with an owner-requested network-governance and non-surveillant continuity proposal.
- [Selection disposition — existing tools suffice; the substrate is the intentions board, linked not built-in](../decisions/coordination-selection-disposition.md): Keeps the accepted no-built-in-coordination disposition in force unless council review later produces an explicit replacement decision.
- [External workspace state and the Clearhead boundary](../decisions/external-workspace-and-clearhead-boundary.md): Preserves Clearhead as a linked optional intention authority while execution and epistemic workspace state retain separate ownership.
