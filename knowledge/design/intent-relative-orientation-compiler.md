---
type: Design Note
title: Intent-relative orientation compiler
description: Proposal for compiling intent, public authorities, structural provider output, reusable capabilities, and situated evidence into a bounded task-local view; the first unified-root trial tied quality but cost more, so no kernel feature is authorized and any follow-up must simplify the recipe and choose a harder case.
generated: { by: agent/cli, at: 2026-09-18T01:42:27Z }
---

# Design proposal — Intent-relative orientation compiler

**Status:** owner-requested proposal for critique. It does not change the
accepted implementation roadmap, reopen cancelled coordination entities, or
authorize a kernel feature. The first bounded experiment is complete: the
[unified-root trial](../evaluations/unified-root-orientation-trial-report.md)
found equal 15/16 quality, fewer treatment reads and a smaller peak window, but
higher total tokens, turns, tools, time, and cost. That result does not authorize
a kernel feature; future work must simplify the recipe and choose a case whose
public charter is not already an excellent orientation capsule.

## 1. Product thesis

The next useful layer is not a larger wake, a second knowledge base, or a
repository-wide summary. It is an **orientation compiler**:

> Given an intent, compile the smallest provenance-backed view of public
> authority, repository structure, reusable capabilities, situated beliefs,
> and possible overlapping work that an agent needs before its first edit.

The compiler owns selection and explanation, not truth. Clearhead or another
work system owns intent and priority; Git owns repository state; specifications,
ontology, schemas, types, code documentation, and decision records own public
meaning; structural providers own their analyses; tests own their outcomes; and
Agent Workspace owns only the situated join, provenance, freshness, omissions,
and progressive-disclosure path.

Deleting workspace state must make orientation slower, never make project truth
unrecoverable.

## 2. The problem it must solve

The existing wake answers: *why was work happening, what did earlier agents
believe, what governing knowledge applies, and what changed?* It deliberately
does not answer:

- which public authorities govern this particular intent;
- which subsystem and modules own the relevant behavior;
- which existing abstraction or capability should be reused;
- whether ontology, specification, schema/type, and implementation disagree;
- what the minimum trustworthy reading set is;
- whether another line of work is producing or changing a needed capability.

Without those answers, a successor can be fresh but still rebuild the wrong
mental model, duplicate a core abstraction, or spend most of its context
reconstructing relationships other participants already discovered.

## 3. The output: an orientation capsule

The existing 1000-byte wake remains unchanged. It is the interrupt-sized index.
Orientation is a follow-up projection fetched when work begins or the intent
changes.

A capsule is bounded and task-local. It contains:

1. **Intent** — thesis, external authority, acceptance conditions, and source
   state.
2. **Protected boundaries** — relevant public interfaces, external obligations,
   public documentation/taste surfaces, and core domain entities or
   relationships. These are pointers to public artifacts, never workspace-owned
   declarations.
3. **Authority chain** — relevant ontology, specifications, schemas/types,
   decisions, and implementation owners, with the reason each item was selected.
4. **Relevant structure** — subsystem/module/symbol entry points from native
   structural providers, tied to their repository revision.
5. **Reuse candidates** — existing abstractions or capabilities that may own the
   work, distinguishing public declarations from inferred candidates.
6. **Situated state** — current/stale/unknown workspace claims and evidence that
   materially narrow the task.
7. **Conflict signals** — changed authorities, incompatible public guidance, or
   disagreement between declared ownership and current implementation.
8. **Community signals** — only evidence-backed semantic overlap: related
   intentions, branches/worktrees, claims, transactions, or Git divergence.
   Absence never means nobody else is working.
9. **Recommended reads** — the smallest ordered set of exact sources or symbols
   the agent must inspect before editing.
10. **Uncertainty and omissions** — what could not be resolved, which providers
    were unavailable, and what was excluded by bounds.

Every item carries three answers: **who said this, why is it relevant, and how
current is the supporting input?** A capsule without those answers is merely a
persuasive summary.

## 4. Progressive workflow

```text
resolve intent
  → retrieve public authorities
  → obtain structural/capability candidates from native providers
  → join relevant workspace evidence
  → detect authority conflict and protected-boundary contact
  → emit bounded capsule + minimum reads
  → agent verifies the named sources
  → implement with native edit tools
  → validate with native checks
  → reconcile workspace state and promote durable conclusions publicly
```

The capsule is not permission to edit. It is a navigation and preflight result.
The agent still reads the exact implementation and tests it will change.

If public authorities disagree, the result is `needs-review`, not a guessed
precedence. The relevant participants may resolve the contradiction directly or
through a bounded council. Prefer correcting the public guidance in a separate
commit before implementation; a combined specification-and-code commit remains
acceptable when tightly coupled and independently reviewable.

## 5. Human and agent decision boundary

The orientation view should mark, rather than silently cross, four owner-held
boundaries discovered in design discussion:

- public interfaces and compatibility commitments;
- paid, operational, licensed, or otherwise external dependencies;
- public documentation and other product-taste surfaces;
- core domain entities, relationships, and semantics.

Agents may autonomously choose implementation below those boundaries. They may
propose boundary changes, but a detected boundary contact changes the workflow
from implementation to a reviewable proposal unless an existing public mandate
already authorizes the change.

In the ClearHead platform, these boundaries are expressed through the ontology,
specifications, schemas/types, Clearhead entities, API documentation, and
repository architecture. The compiler should follow those authorities rather
than invent a private boundary registry.

## 6. Reuse without a capability registry

The motivating failure is an agent implementing a concern locally after another
agent introduced a core abstraction for it. The first version should not answer
that by creating a workspace-owned ontology of capabilities.

Instead it should report two classes:

- **Declared ownership** — a public source explicitly says a module, type, API,
  or library owns a capability. This can justify a strong interruption.
- **Inferred reuse candidate** — structural search, references, tests, or prior
  situated claims suggest an existing abstraction. This is a prompt to inspect,
  never proof of canonical ownership.

A future lightweight near-code declaration may be justified if repeated trials
show inference misses important abstractions. Such a declaration must be
human-readable, Git-reviewed, useful without Agent Workspace, and projected by
the tool rather than owned by it.

## 7. Community model

Large-agent scaling requires local projections over shared evidence, not one
plan or status stream every participant must read.

An agent should see only:

- work that governs or overlaps its intent;
- capabilities it consumes that another work item may produce or change;
- contradictions that require its evidence or judgment;
- integration changes that invalidate its assumptions.

Coordination is optimistic. Git worktrees and branches isolate writers; semantic
overlap is advisory; duplicate explorations may proceed; and a merger or council
can reconcile them. No lock, presence estimate, or silence may establish safety.
The merger facilitates synthesis but receives no authority to change public
intent or protected boundaries.

This proposal does not automatically reopen the cancelled workstream,
declaration, or handoff entities. Existing Git, claims, checkpoints, linked
intentions, and ordinary messaging should supply the first trial. New machinery
must be earned by a repeated failure those tools cannot solve cheaply.

## 8. Provider and kernel boundary

The kernel should not implement ontology reasoning, source indexing, LSP,
tree-sitter, or task-manager semantics. Native providers remain authoritative.
A future implementation may coordinate bounded provider results, but each
result must retain:

- provider identity and native reference;
- repository/worktree and revision;
- query or selection reason;
- coverage and omission information;
- current, changed, unavailable, or unknown assessment where meaningful.

Provider failure is distinct from a successful empty result. An unavailable
structural provider cannot be rendered as “no reusable abstraction exists.”
Likewise, ten claims derived from one source remain one evidentiary lineage, not
ten independent confirmations.

The orientation capsule should initially be an ephemeral projection. Persist
only deliberate conclusions through existing claims, bindings, checkpoints, or
public knowledge. Do not turn every generated module summary into durable state.

## 9. Smallest falsifiable trial

Before adding a kernel verb, run one consequential task in the ClearHead
platform repository using existing tools.

### Selected candidate: unified workspace root

The owner selected the `unified-workspace-root` charter, specifically the
priority-1 `adopt-unified-root` action and its conformance/migration child. It is
a strong orientation fixture because it crosses public domain and interface
boundaries while retaining independently checkable behavior:

- the public model is governed by the charter plus `specifications/workspace.md`,
  `configuration.md`, `charters.md`, and the workspace schema;
- implementation spans Core workspace assembly/pathing, initialization, doctor,
  plans, archival, CLI, and LSP;
- the canonical reusable primitives already include `ROOT_ANCHOR_STEM`,
  `PRIMARY_ACTIONS_FILE`, and `PRIMARY_DOCUMENT_FILE`;
- the charter log reports that most specification and Core work, plus platform
  migration, already landed, while the actions still read not-started and their
  descriptions still include migration as future work;
- the named residuals are narrower: completed-history naming still derives from
  the project directory, and LSP-level conformance remains open.

That mismatch is part of the test, not setup noise. A successful orientation
capsule should detect that the public work record and landed repository state do
not tell one coherent lifecycle story, verify the cited commits and current
specification before implementation, and narrow the task to real residuals. An
agent that starts by rebuilding unified-root initialization has failed the
orientation test even if its code is locally correct.

### Treatment

A fresh agent receives only the ordinary bounded wake, then follows a fixed
orientation recipe using:

- Clearhead intent and predecessors;
- linked OKF/specification/ontology sources;
- `project_report`, `module_report`, symbol search, and bounded symbol reads;
- Agent Workspace claims, bindings, working set, and reveals;
- Git status, history, worktrees, and native tests.

The recipe produces a capsule in the schema from §3 before the first edit. No
new workspace entities are introduced.

### Task shape

Choose a real task where:

- the relevant behavior spans more than one platform repository or layer;
- an existing core abstraction is easy to miss but should be reused;
- at least two public authorities govern the change;
- the correct implementation can be independently checked.

Plant no answer in the prompt. Record the expected authority chain and reusable
abstraction privately before the run so scoring cannot move afterward.

### Comparison and measures

Compare against an ordinary fresh-agent run with the same intent, model class,
permissions, and validation budget. Measure:

- tokens and elapsed time to the first correct edit;
- unique files and bytes read before that edit;
- repeated reads already covered by current evidence;
- whether the core abstraction was found and reused;
- authority conflicts detected before implementation;
- incorrect accepted assumptions;
- human recap/intervention burden;
- capsule-production overhead;
- final verified correctness and integration rework.

The treatment earns implementation only if it reduces reconstruction or avoids a
material architectural error by more than its own coordination cost. A simpler
sequence of existing tools remains the preferred answer when it performs as
well.

## 10. Possible later surface, not yet authorized

If the trial succeeds, the smallest product surface is plausibly one read verb:

```text
workspace_orient { intent?, focus?, budget?, since? }
```

Its result is the bounded capsule, with opaque item ids or native references for
one-call expansion. It composes existing workspace projections with provider
outputs; it does not copy source bodies, choose architectural truth, create work,
or mutate external authorities.

The wake may eventually carry one line such as `orient: needs-review` or
`orient: ready · 4 recommended reads`, but only after use proves that signal is
worth scarce wake bytes.

## 11. Prohibited failures

- **Second source of truth:** public intent, domain semantics, or capability
  ownership exists only in workspace state.
- **Summary laundering:** an inferred or generated summary appears as canonical
  fact without provider and source.
- **False completeness:** missing provider coverage is rendered as absence.
- **Intent-free ranking:** old or merely recent claims crowd out evidence
  relevant to the current objective.
- **Silent precedence:** contradictory public authorities are resolved by tool
  ranking rather than review.
- **Global-plan tax:** every agent must ingest all participants' work before
  becoming useful.
- **Coordination theater:** bookkeeping consumes more context than the repeated
  reading or integration work it prevents.
- **Wake inflation:** the 1000-byte wake becomes a compressed repository dump.

## 12. Questions the trial must answer

1. Can a useful authority chain be assembled from existing links, or is a
   lightweight public relationship declaration missing?
2. Can structural providers identify reusable abstractions semantically enough,
   or do they mostly return lexical neighbors?
3. Which capsule sections actually change agent behavior?
4. How small can the recommended reading set remain without creating false
   confidence?
5. Does intent-relative ranking cure the active/stale claim backlog, or is claim
   lifecycle curation independently necessary?
6. Which cross-agent overlap is useful before merge, and which is cheaper to
   reconcile afterward?
7. Is capsule composition best owned by the kernel, a provider adapter, or a
   harness-level recipe once the experiment reveals the stable seam?

## Related concepts

- [The agent's perspective](agent-perspective.md) — establishes attention as the
  scarce resource and demands progressive disclosure rather than repository
  ingestion.
- [Initial design outline](initial-design.md) — defines the native-provider
  boundary and the original structure-provider hypothesis.
- [Harness-neutral multi-agent coordination](harness-neutral-multi-agent-coordination.md)
  — historical proposal for explicit coordination entities; this proposal keeps
  those entities deferred during the first trial.
- [Networked agency and the continuity commons](networked-agency-continuity-commons.md)
  — supplies the community model and authority-preserving council direction.
- [Wake summary contract](../specifications/wake-summary-contract.md) — remains
  the bounded interrupt-sized orientation index.
- [Knowledge pulse contract](../specifications/knowledge-pulse-contract.md) —
  provides pinned pointers to governing public knowledge without copying it.
- [Use OKF for curated project knowledge](../decisions/okf-curated-knowledge-layer.md)
  — governs the durable/open versus situated/workspace boundary.
- [Selection disposition — linked, not built-in](../decisions/coordination-selection-disposition.md)
  — prevents this proposal from silently reopening unearned coordination
  machinery.
