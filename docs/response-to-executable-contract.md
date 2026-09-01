# Response to the Revised Perspective and Executable Contract (2026-09-01)

*This follows the revision appended to
[`design-note-agent-perspective.md`](design-note-agent-perspective.md) and reviews
[`executable-contract.md`](executable-contract.md). The conceptual disagreement
has largely resolved. What remains is an adversarial precision pass before the
`contract` action can close.*

## Where we now agree

The revision responds to the critique honestly rather than merely softening its
language. In particular, the following are sound foundations for the MVP:

- observations, claims, and evidence are distinct layers;
- `current` must be positively established rather than inferred from silence;
- bypass has separate soundness and adoption dimensions;
- bookkeeping-only actions should disappear from the critical path, while
  intent-bearing boundaries remain explicit;
- the Neovim analogy is a design aid, not evidence that agents share human
  deficits;
- checkpoint and handoff are successful outcomes;
- cognitive braking remains a research question beyond the buildable policy
  perimeter.

The resulting contract is substantially stronger than either preceding design
note. The remaining issues do not require another product reframe.

## Freshness, dependency assurance, and coverage are different dimensions

The contract currently asks `current | stale | unknown` to carry three kinds of
knowledge:

1. whether recorded inputs still match;
2. whether the declared dependency scope is complete;
3. whether the workspace observed all relevant operations.

These cannot be collapsed safely.

A workspace can reproduce that every input in a declared set still matches. It
cannot generally prove that no undeclared semantic dependency exists. Therefore
“no `current` over an untracked dependency” is not an executable invariant if
“relevant” means relevant in objective reality. Enforcing it literally would
make nearly every nontrivial claim permanently `unknown`.

The contract should instead report at least:

- **freshness within scope:** `current | stale | unknown`;
- **scope assurance:** `declared | derived | conservative`, plus whether the
  actor asserts that the scope is complete;
- **operational coverage:** which paths and operations were mediated, and the
  repository state at the last reconciliation boundary.

A claim may then be `current` **within its recorded dependency scope** while
also reporting that scope as partial or conservatively derived. Client
projections must not shorten that to an unqualified claim of truth.

This preserves a useful positive result without pretending the workspace has
solved semantic dependency discovery.

## Bypass must distinguish reads from mutations

A raw read gives the agent information the workspace did not record. It does
not, by itself, make a tracked repository observation stale. The workspace
cannot know what belief the agent formed from that read unless the belief is
later registered as a claim. The honest result is absent provenance for that
belief, not a mutation of unrelated freshness state.

A raw edit or revision change is different. It changes inputs, but a lazy system
cannot notice that change until a defined reconciliation boundary. The contract
must name those boundaries, for example:

- before returning any freshness verdict;
- before validation or transaction acceptance;
- when opening or resuming a workstream;
- after a mediated mutation;
- on an optional watcher event.

Every verdict must identify the repository or input fingerprint against which
it was computed. If the current state has not been reconciled, the verdict is
unknown rather than silently inherited from the last session.

Scenario S2 should therefore begin with a claim whose declared dependency scope
includes path B, perform an out-of-band edit to B, and then cross a named
reconciliation boundary. Only then can the expected invalidation be tested.

## Intent may be fused, but never inferred

The revision usefully proposes fusing intent with the natural action that first
requires it. A transactional edit operation may atomically:

1. begin a transaction with explicit intent and base state;
2. apply the requested mutation;
3. record the resulting delta.

That removes a separate ceremony without guessing intent after the fact. An
ordinary unmediated edit cannot safely supply an objective, acceptance criteria,
or ownership boundary merely because it happened first.

## Revert must preserve pre-existing work

Scenario S6 currently says that reverting restores the repository to revision
R. This is unsafe when the transaction begins from a dirty worktree or when
concurrent human work exists.

A transaction needs both:

- a Git revision identity; and
- a captured initial worktree state or equivalent transaction-owned delta
  boundary.

Revert must remove only transaction-owned mutations and reconstruct the initial
worktree state. If later overlapping mutations make that ambiguous, revert must
stop with a conflict instead of destroying work or claiming success.

A separate scenario should exercise a dirty initial worktree and prove that
unrelated pre-existing changes survive transaction rollback.

## The contract must be normatively complete

The `contract` action also calls for authority boundaries, privacy constraints,
and explicit non-goals. Referencing `initial-design.md` as background is useful,
but an executable contract should either incorporate those terms normatively or
state exactly which sections are imported.

Before closure it should define or normatively reference:

- objective, workstream, semantic location, finding, transaction, and
  checkpoint;
- Git, provider, command, task-system, and workspace authority boundaries;
- retention limits, redaction behavior, content-addressed payload handling, and
  what happens when native detail cannot be retained safely;
- unsupported capability and provider-failure behavior;
- concurrency and external-mutation behavior;
- explicit MVP non-goals.

At least one scenario should cover each safety invariant. Provider failure,
secret-bearing output, ambiguous relocation, dirty-worktree rollback, and
concurrent overlapping edits currently lack behavioral examples.

## Acceptance must use fixed comparisons

“False-current approaches zero” is a useful evaluation aspiration but not an
executable threshold. Contract scenarios should require zero false-current
verdicts over a fixed adversarial fixture suite. Dogfooding can separately
report an observed rate with its sample size and known instrumentation gaps.

Likewise, bounded perception should be tested with:

- a fixed repository task;
- equivalent successful task outcomes for raw and assisted operation;
- a defined accounting boundary for bytes or tokens;
- a maximum assisted budget or minimum reduction;
- provider detail still retrievable on demand.

Returning an outline instead of a file trivially reduces bytes but does not
prove that the agent could complete the same work.

## Resolution

The design dialogue has reached a coherent product position. The contract now
needs to make its guarantees relative and observable:

> The workspace proves freshness over recorded inputs, exposes how those inputs
> were selected, reports the operations it could observe, and never upgrades a
> scoped result into an unqualified truth claim.

With that distinction, explicit reconciliation boundaries, safe dirty-worktree
rollback, and complete authority/privacy/failure scenarios, the `contract`
action will be ready to close and the walking skeleton can begin without
smuggling infrastructure assumptions into the design.
