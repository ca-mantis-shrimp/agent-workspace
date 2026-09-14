---
type: Specification
title: Contextual coordination contract
description: Defines worktree-relative assessments, durable workstreams, bounded coordination projections, and recursive superproject change tracking for harness-neutral multi-agent coordination.
tags: [coordination, worktrees, superprojects, submodules, contract]
generated: { by: agent/cli, at: 2026-09-14T05:28:03Z }
---

# Contextual Coordination Contract

**Status:** normative contract for the `multi-agent-coordination` charter. It extends the
[workspace MVP contract](executable-contract.md); that contract continues to govern records
and transactions unless this document explicitly makes a verdict contextual.

## 1. Authorities and identities

- **Repository/project identity** is the canonical Git common directory of the repository
  in which Agent Workspace was opened. Linked worktrees share one project identity;
  independent clones do not share merely because their remotes match.
- **Worktree identity** is the canonical worktree Git directory within that common
  directory. A path, branch name, or HEAD is descriptive state and is not the identity.
- **Owned repository** is the project repository itself or an initialized Git submodule
  recursively contained by it. Owned repositories do not become separate Agent Workspace
  projects when reached through the superproject context.
- **Workstream identity** is a durable opaque identifier for one intended outcome. A
  workstream may cite an external work item, but Clearhead or the cited system owns that
  item's priority, predecessor graph, and lifecycle.
- **Session identity** is an opaque resumable attribution identifier attached to one
  workstream and one worktree. Actor and harness labels are informative, not
  authentication. `last_seen` is an estimate from kernel interaction, never proof of
  liveness or death.
- **Declaration identity** is an append-only revisioned record owned by a workstream. It
  names an intended capability/outcome, integration target, and advisory mutation or
  review scope. It is not a lock or lease.
- **Dependency identity** is a directed edge from a consuming workstream to a producing
  workstream and promised capability. It does not imply that the capability exists or has
  landed.
- **Handoff identity** is an immutable transfer record naming a checkpoint, responsibility
  target (a workstream, session, or unassigned), residual risks, and integration state. It
  transfers responsibility, never freshness or truth.
- **Assessment identity** is the pair `(subject identity, worktree identity)`, plus the
  reconciliation sequence. Claims, evidence, and findings remain shared subjects; their
  current/stale/unknown verdicts are worktree-relative assessments.

Git remains authoritative for bytes, revisions, branches, worktrees, nested repository
state, diffs, and reachability. Agent Workspace owns only the durable associations,
attribution, assessments, declarations, dependencies, handoffs, and bounded projections.
Harness adapters capture and present; they own no coordination semantics.

## 2. Contextual freshness

The normative rule is:

```text
freshness(subject, worktree context) -> current | stale | unknown
```

A reconciliation in one worktree MUST NOT overwrite or masquerade as the latest assessment
in another. A status or delta projection MUST name the worktree context used for every
served verdict. A legacy `ClaimReconciled`, `EvidenceReconciled`, or
`FindingReconciled` event without a worktree identity remains replayable as historical
unattributed evidence, but MUST NOT be served as a current contextual assessment without a
new reconciliation.

Two linked worktrees may therefore project one shared claim as current and stale at the
same time. This is not disagreement in the event log; it is the expected result of applying
one subject to different bytes.

## 3. Superproject and subrepository change model

A superproject change snapshot is the bounded union of:

1. superproject-shell index, worktree, and untracked changes;
2. each initialized submodule's index, worktree, and untracked changes, recursively;
3. each submodule HEAD's divergence from the commit pinned by its parent gitlink; and
4. the parent's gitlink change when the parent index or worktree pins a different commit.

Nested paths MUST be remapped into the project namespace. For example, an edit reported by
Git as `src/lib.rs` inside the `clearhead-core` submodule is projected as
`clearhead-core/src/lib.rs`, not only as the opaque `clearhead-core` gitlink. Each entry
retains its owning repository, local status, and revision context so consumers can
distinguish:

- uncommitted nested content;
- committed-but-unpinned nested work;
- a staged or committed gitlink update; and
- an unavailable or uninitialized nested repository.

The opaque parent gitlink entry may coexist with file-granular nested entries because they
answer different questions: integration state versus changed content. They MUST NOT be
deduplicated into a false single-path account.

If `.gitmodules`, an expected submodule checkout, recursive enumeration, or a nested Git
query cannot be read, coverage for that owned repository is `unknown` with the native Git
failure retained. It is never projected as clean. Ordinary nested repositories that are
not represented by a committed gitlink are outside the first implementation's automatic
project membership; they require explicit configuration or a later contract revision.

Repository divergence is evidence of changed bytes or revision relationships, not evidence
of agent activity. Association with a workstream is explicit; otherwise it is reported as
unassociated divergence.

## 4. Declarations, overlap, dependencies, and handoff

A declaration and the overlap snapshot returned to its caller MUST be committed by one
serialized kernel operation. Comparison includes:

- mutation/review path intersection in the project namespace, including remapped submodule
  paths;
- capability/outcome overlap even when paths are disjoint;
- work-item and integration-target overlap; and
- open transaction mutation paths.

Overlap is advisory and ranked by relationship: writer/writer is more severe than
writer/reviewer, while reader/reader is informational. A clean overlap result asserts only
that no conflicting recorded declaration was visible in that serialized operation. It
never asserts that Git is clean or that uninstrumented writers do not exist.

A dependency records planned reliance. `ready` is declared producer state;
`landed` requires Git reachability from the consumer's named integration target. After
landing, the consumer MUST update its own worktree and re-assess shared subjects before
using them. A handoff cannot transfer a freshness verdict.

## 5. Event and projection ownership

Durable events own stable facts and lifecycle transitions: workstream/session creation or
resumption, declaration revisions/releases, dependency edges, handoffs, and contextual
assessments. Git-derived repository snapshots and estimated session recency are projections;
they are recomputed at query boundaries and are not silently promoted to durable truth.

Every bounded collection reports an explicit omission count. Superproject change
projection bounds apply to both owned repositories and changed paths: omitting a nested
repository requires a repository omission count, and omitting its paths requires a path
omission count. Most severe overlaps and unknown-coverage entries rank before clean or
informational entries.

All operations must work over request/response MCP. Heartbeats, push notifications,
remote-pi, and harness hooks may reduce latency but are not correctness prerequisites.

## 6. Concurrency and failure semantics

- The append-only event writer serializes declaration plus overlap, lifecycle transitions,
  and assessments.
- Filesystem and Git state may change during a query. If one coherent snapshot cannot be
  established, the affected assessment or divergence entry is `unknown`; the kernel does
  not combine observations from incompatible revisions into `current`.
- A missing worktree or owned repository is explicit unknown/unavailable state, not an
  implicit release of responsibility.
- Branch names and session recency never prove integration, ownership, or liveness.
- Legacy events never receive invented workstream, session, worktree, or actor identities.
- Cross-submodule mutation atomicity remains Git's boundary: one Agent Workspace
  transaction is scoped to one owning Git repository. Coordination may group related
  workstreams but cannot claim an atomic commit across repositories.

## 7. Prohibited failures

- **C1 — contextual overwrite:** one worktree's reconciliation replaces another's served
  assessment.
- **C2 — gitlink blindness:** nested content changes appear only as an opaque submodule
  directory when the initialized owning repository can provide file-level state.
- **C3 — false clean:** unavailable, uninitialized, truncated, or failed nested enumeration
  is presented as clean or as zero divergence.
- **C4 — invented activity:** raw Git divergence is attributed to an agent or workstream
  without an explicit association.
- **C5 — split declaration:** a declaration is accepted separately from its overlap
  snapshot, allowing concurrent writers to both receive a falsely clean result.
- **C6 — declared landing:** producer/session state or branch naming is treated as proof of
  integration without Git reachability.
- **C7 — transferred truth:** a dependency or handoff causes freshness to be inherited.
- **C8 — unbounded wake:** a projection omits entries without counts or expands with total
  history/repository size on the default path.

## 8. Executable scenarios

**CC1 — linked-worktree independence.** Given one shared claim and two linked worktrees with
different bytes, when both reconcile, then status in one is current and status in the other
is stale simultaneously; restarting either context preserves both assessments.

**CC2 — legacy oscillation characterization.** Against the pre-contextual event model,
reconciling the same claim from divergent linked worktrees appends alternating global
`ClaimReconciled` verdicts. The characterization must reproduce this gap before the
contextual-freshness implementation replaces it.

**CC3 — dirty submodule file.** Given an initialized submodule in a superproject, when
`sub/src/lib.rs` is edited without changing submodule HEAD, then repository divergence
contains `sub/src/lib.rs` with the submodule as owner. It may additionally contain the
parent's dirty gitlink signal, but not instead of the file.

**CC4 — committed but unpinned submodule.** Given a new commit in a submodule while the
superproject still pins its parent commit, then divergence names the submodule HEAD versus
pinned revision and reports the changed nested paths. It does not claim that integration
landed.

**CC5 — unavailable nested coverage.** Given a registered but uninitialized submodule, when
coordination status is requested, then that repository is `unknown/unavailable` with a
nonzero coverage gap; the superproject is not reported clean.

**CC6 — atomic advisory declaration.** Given two concurrent workstreams declaring the same
nested mutation path, at most one receives a snapshot with no recorded writer overlap; both
declarations remain inspectable and advisory.

**CC7 — verified dependency landing.** Given B depends on A's capability, when A marks it
ready but its revision is not reachable from B's integration target, then B sees ready but
not landed. Only after Git reachability succeeds and B re-assesses locally may B consume the
claim as current.

**CC8 — bounded recursive projection.** Given more owned repositories and nested changed
paths than the default caps, status returns deterministic ranked subsets plus separate
repository and path omission counts without traversing unbounded history.

## Related concepts

- [Harness-neutral multi-agent coordination](../design/harness-neutral-multi-agent-coordination.md)
- [First superproject dogfood](../evaluations/platform-superproject-dogfood.md)
- [External workspace state and the Clearhead boundary](../decisions/external-workspace-and-clearhead-boundary.md)
- [Center the tool surface on MCP](../decisions/mcp-centered-tool-surface.md)
- [Harness-neutral multi-agent coordination](../design/harness-neutral-multi-agent-coordination.md): Makes the design note's repository, worktree, assessment, and coordination model normative.
- [First superproject dogfood — the submodule boundary is narrower than feared](../evaluations/platform-superproject-dogfood.md): Turns the platform superproject findings into explicit recursive change-tracking invariants and scenarios.
