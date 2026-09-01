# Initial Design Outline

This document is a starting hypothesis, not an implementation contract. The `contract` action should turn it into explicit invariants and executable scenarios.

## 1. Design stance

The workspace is a local control plane. It coordinates native tools through durable state but does not implement their domain logic or pretend their outputs are interchangeable.

A useful decomposition is:

```text
Clients / projections
  Pi tools · CLI/status · Neovim
             │
Application operations
  orient · focus · jump · begin change · record finding · validate · checkpoint
             │
Workspace kernel
  commands → events → projections
             │
Provider adapters
  Git · structural navigation · diagnostics · commands · task context
```

The kernel should not import Pi or Neovim concepts. Both are clients of the same operations and projections.

## 2. Candidate state model

### Workspace

- durable workspace ID;
- repository root and current Git identity;
- configured providers and retention policy.

### Workstream

- objective reference and prose intent;
- lifecycle mode: orient, investigate, plan, edit, validate, review, deliver;
- active transaction;
- working set and navigation trail;
- open findings and current evidence;
- latest checkpoint.

### SemanticLocation

- provider and language;
- repository-relative path;
- symbol path and kind when known;
- observed revision;
- source range as a hint, not identity;
- syntax/content fingerprint for relocation;
- provider-native identity where available.

### Observation

- observation ID and kind;
- semantic location or workspace scope;
- producing provider and operation;
- observed Git revision plus relevant file hashes;
- timestamp;
- bounded normalized summary;
- native payload or content-addressed payload reference;
- freshness state and invalidation reason.

### Finding

- severity, message, rule, and location;
- provider provenance and native payload reference;
- lifecycle: open, resolved, deferred, suppressed, false-positive, stale;
- disposition rationale and actor;
- transaction and evidence associations.

### ChangeTransaction

- intent and acceptance criteria;
- base revision and initial worktree state;
- mutations/diff and affected locations;
- known blast radius;
- findings and evidence;
- lifecycle: open, validating, accepted, committed, reverted, abandoned.

### Evidence

- named check and exact command/provider invocation;
- result, output reference, duration, and environment summary;
- revision plus input fingerprint;
- claims it supports;
- current/stale state and invalidation reason.

## 3. Event model

Prefer commands that validate intent and emit immutable events. Build current status as a projection. Candidate events include:

```text
WorkspaceOpened
ObjectiveBound
LocationFocused
ObservationRecorded
ObservationInvalidated
TransactionBegan
MutationObserved
FindingRecorded
FindingDispositionChanged
ValidationRecorded
EvidenceInvalidated
CheckpointCreated
TransactionCommitted
TransactionReverted
```

Do not expose events directly as the primary client API. Clients request operations; the kernel enforces invariants and records events.

The event format needs versioning from the first persisted record. Native payloads should generally live in a bounded, content-addressed store so the event log remains inspectable and redaction is possible.

## 4. Important invariants

The contract should sharpen and test at least these:

1. No observation may claim freshness without a reproducible relationship to current repository inputs.
2. Validation evidence cannot support a transaction after relevant inputs change.
3. A normalized result never discards the identity of its provider or access to retained native detail.
4. A transaction always names its base state and can expose the exact delta from that base.
5. Restart recovery reconstructs state from durable records rather than conversation history.
6. Failed semantic relocation yields ambiguity or staleness, never silent rebinding.
7. Provider failure is recorded separately from a successful empty result.
8. Human and agent dispositions identify their actor and rationale.
9. Secrets and unbounded output are not copied into durable state by default.
10. A client can inspect why an item is in the working set and why evidence is current.

## 5. Adapter boundaries

Start with narrow capabilities rather than a universal provider interface:

- **Revision provider:** current revision, dirty state, diff, restore/checkpoint primitives.
- **Structure provider:** outline, resolve location, related locations, bounded symbol read.
- **Finding provider:** diagnostics or analyzer findings with native payload retention.
- **Validation provider:** execute or import a check and report exact inputs/results.
- **Objective provider:** resolve external task context and retain its durable reference.

Capability discovery should be explicit. Unsupported operations must fail as unsupported, not return an empty collection.

## 6. First walking skeleton

The first end-to-end test should avoid broad integrations:

1. Initialize the workspace over a tiny Git fixture.
2. Bind a textual objective and base revision.
3. Obtain an outline through one structural adapter.
4. Focus and record an observation for one symbol.
5. Begin a transaction and modify the symbol externally or through a minimal mutation adapter.
6. Detect that the earlier observation is stale.
7. Import one diagnostic as a finding.
8. Run one deterministic validation command as evidence.
9. Resolve the finding and checkpoint.
10. Restart the process and reproduce the same status.
11. Change a relevant input and demonstrate evidence invalidation.
12. Revert the transaction and demonstrate repository restoration.

This scenario should drive storage and API choices.

## 7. Open questions

- Is an embedded database justified, or is an append-only structured log plus generated projections sufficient for the MVP?
- Should file watching emit mutations automatically or should clients establish explicit refresh boundaries first?
- What is the smallest safe input fingerprint for invalidating diagnostics and test evidence?
- Which semantic-location fallback provides useful relocation without creating false identity?
- How should dirty pre-existing changes be separated from transaction-owned changes?
- Which outputs may be retained, summarized, redacted, or referenced only ephemerally?
- Should the initial Pi interface be an extension, MCP server, or direct custom-tool package?

These should remain decisions to investigate, not assumptions embedded in the charter.

## 8. Evaluation

Dogfooding should compare ordinary agent operation with workspace-assisted operation. Useful signals include:

- repeated reads caused by lost orientation;
- tokens spent reconstructing repository state;
- stale observations mistakenly reused;
- validation repeated unnecessarily or omitted;
- time to resume after restart;
- findings lost between tools;
- human effort required to understand agent state;
- bookkeeping overhead introduced by the workspace itself.

The MVP earns continuation only if it reduces failure or cognitive/context cost enough to justify its own state management.
