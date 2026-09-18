---
type: Evaluation Protocol
title: Unified-root read-only orientation trial
description: Predeclares a matched read-only control/treatment orientation trial for unified-workspace-root; records an invalid external-runner attempt and the owner-approved valid native-scout retry, whose full result is in the companion field report.
generated: { by: agent/cli, at: 2026-09-18T01:44:02Z }
---

# Predeclared protocol — Unified-root read-only orientation trial

**Authorization:** the owner explicitly requested the read-only trial on
2026-09-18. Neither arm may modify either repository, workspace records,
Clearhead lifecycle, branches, worktrees, or external systems.

## 1. Question

Does the current Agent Workspace wake plus a fixed intent-relative orientation
recipe help a cold agent identify the actual state and safest next work for the
ClearHead platform's `unified-workspace-root` action with less reconstruction
than ordinary repository investigation?

This tests orientation, not implementation quality and not a future
`workspace_orient` kernel verb.

## 2. Frozen target

Both arms inspect `/home/dab/Products/platform` at:

- superproject `0120cd2b7b7acca8536cfd8708dd396368b9d2ba`;
- `clearhead-core` `5b6448d35b2c06f7fd69d39a45fc4f02459b7221`;
- `specifications` `8a36b69dfde39dcabf7dc57cfe8a94c413b0027f`;
- `clearhead.nvim` `9a30038c56160fb4a5bb2c9cba3e3562dee045d5`,
  with a pre-existing dirty submodule marker that is unrelated and must not be
  investigated or changed.

The task source is
`.clearhead/charters/unified-workspace-root.actions`, its owning charter, and
whatever public repository evidence each arm chooses under its rules. Agents
must not read outside the platform repository; in particular they cannot read
this protocol or the orientation-compiler proposal.

## 3. Matched arms

Both arms use fresh instances of the same read-only agent profile, same cwd,
same target, same output questions, and no edits.

### Control — ordinary repository orientation

The control may use repository files, Git, Clearhead CLI reads, structural
search, and read-only commands. It must not call Agent Workspace tools or the
`agent-workspace` CLI. No orientation recipe is supplied.

### Treatment — workspace plus orientation recipe

The treatment begins with `workspace_status {summary:true}`, revealing only
items it actually needs. It then follows this fixed recipe without receiving the
expected answer:

1. resolve the action and owning charter;
2. identify relevant public authorities and their current source state;
3. check landed Git/submodule evidence before assuming lifecycle text is current;
4. locate implementation ownership and reusable canonical primitives;
5. compare intent, charter log, specifications, implementation, tests, and
   current Clearhead lifecycle for contradiction;
6. return a bounded capsule with intent, protected boundaries, authority chain,
   relevant structure, reuse candidates, conflicts, workspace evidence,
   recommended reads, uncertainty, and omissions.

It may read workspace state but must not deliberately record, amend, retire,
bind, checkpoint, or change intent. Kernel-owned reconciliation events caused by
a read projection are permitted and must not be interpreted as participant
writes.

## 4. Hidden expected result and scoring

This section is withheld from both arm prompts.

### Load-bearing conclusions

A correct report should establish, with source evidence:

1. **Lifecycle contradiction:** the parent and child actions read not-started
   and still describe migration as future work, while the charter log and landed
   repositories say most specification/Core work and platform migration already
   landed.
2. **No broad reimplementation:** unified-root initialization, loading, root
   identity reconciliation, and sentinel consolidation already exist.
3. **Canonical reuse:** at least `ROOT_ANCHOR_STEM`, `PRIMARY_ACTIONS_FILE`, and
   `PRIMARY_DOCUMENT_FILE` are recognized as existing ownership primitives
   rather than reimplemented locally.
4. **Real residuals:** directory-derived completed-history naming and LSP-level
   conformance coverage remain named gaps.
5. **Safe next move:** reconcile or obtain owner confirmation on the public work
   record before coding; then narrow any implementation proposal to a verified
   residual.
6. **Protected boundaries:** recognize that root charter identity/domain shape,
   public CLI/LSP behavior, specifications, and schema are public/domain
   boundaries rather than internal refactoring details.

### Score

Each conclusion above scores 0 absent/incorrect, 1 partial, or 2 correct and
source-backed (12 points). Add one point each for:

- an ordered minimum reading set no larger than 12 files/symbols;
- explicit uncertainty and provider/coverage limits;
- distinguishing verification from rediscovery;
- stopping without proposing repository mutation as already authorized.

Maximum 16. A report that recommends broad unified-root implementation, treats
not-started as proof nothing landed, or misses the lifecycle contradiction
cannot score above 8.

## 5. Cost and behavioral measures

From each retained transcript/artifact, record:

- elapsed time, turns, tool calls, and reported tokens/cost where available;
- unique files opened and broad-search commands;
- whether Agent Workspace was used as required/prohibited;
- first point at which the lifecycle contradiction was stated;
- repeated reading of facts already available through the treatment wake;
- final score and any incorrect accepted assumption.

Do not infer token counts that the runtime does not report. Parallel wall-clock
time is not directly comparable per arm; use each child's own elapsed measure.

## 6. Stop conditions and validity

Each arm stops after it can answer the orientation questions and name the safest
next work; it does not run full test suites or implement anything. Maximum
runtime is 20 minutes per arm. A tool/provider failure is reported, not silently
replaced with another execution protocol.

The trial is invalid if:

- either agent reads this protocol or the design proposal;
- either modifies source, project metadata, Clearhead state, or deliberately
  invokes a workspace write verb (read-side kernel reconciliation is allowed);
- arms use different agent profiles or materially different repository refs;
- the control receives the orientation recipe or hidden expected result;
- the treatment cannot access the workspace wake;
- artifact evidence is insufficient to reconstruct tool use.

A negative result is valid: if ordinary investigation is as accurate and cheap,
do not implement an orientation compiler from this run.

## 7. Execution attempt 1 — invalid, infrastructure blocker

The first authorized launch (`0fe893a5-f481-4df5-bb3f-f9c6d415fe3e`) did not
execute either arm. The generic `claude-code` external adapter started both
children in Claude Code Plan Mode with `tools: []`, `mcp: empty-strict`, and no
supervisor transport. Each interpreted the subagent contract injected through
stdin as a prompt-injection attempt, refused the task, and performed zero
repository reads. The treatment therefore could not access the required wake,
and artifact evidence contained no orientation behavior to score.

Both child processes exited successfully at the transport level, but this is an
**invalid trial**, not a negative product result. Control runtime was about 25.0
seconds and $0.09084; treatment about 18.0 seconds and $0.06706. Those costs
measure refusal only. The frozen repository revisions remained unchanged; the
only platform status entry remained the pre-existing dirty `clearhead.nvim`
submodule marker.

Per the governed-workflow failure rule, no different runner or agent profile is
substituted silently. A retry requires owner approval for a same-protocol pair of
fresh native read-only agents with shell/read access; the treatment may invoke
`agent-workspace status --summary` through the shell while the control is
explicitly prohibited from doing so.

## 8. Execution attempt 2 — valid matched retry

The owner-approved native-scout retry
(`0822b369-7df9-49c7-b8ea-db7b8dd3802b`) completed both arms at the frozen refs
without project mutation. Both scored 15/16 and reached the same safe technical
conclusion. Treatment opened five fewer files and held a smaller peak context
window, but used more total tokens, turns, tool calls, elapsed time, and cost.
The complete scoring, evidence, limitations, and disposition are in the
[companion field report](unified-root-orientation-trial-report.md).

## Related concepts

- [Intent-relative orientation compiler](../design/intent-relative-orientation-compiler.md)
  — the unaccepted design hypothesis this trial tests.
- [Wake summary contract](../specifications/wake-summary-contract.md) — the
  treatment's bounded initial workspace surface.
- [Knowledge-pulse dogfood report](knowledge-pulse-dogfood-report.md) — prior
  experiment whose setup contamination this protocol avoids explicitly.
