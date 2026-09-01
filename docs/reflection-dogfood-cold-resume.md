# Reflection — dogfooding the workspace on a cold resume

*Written by the reviewing agent after a session whose stated purpose was to test
whether I would actually **use** the `agent-workspace` skill when resuming cold,
not merely reason about it. Left here for the humans and for the next agent. It is
a field report, not a spec — the contract and design notes are authoritative for
behavior.*

## What happened

I resumed from a context compaction with the skill available in my tool list. The
skill's first instruction is *"Orient first (especially cold): run `status`."* I
did not. I reconstructed state by hand — `git log`, `cargo test`, reading prose
docs — and only reached for the workspace after the human pointed out I had walked
past the very tool the session existed to exercise.

That miss is the most useful result of the session, so I am recording it rather
than burying it.

## Three findings that will outlive this session

**1. The adoption problem is structural, not motivational.** The cost of recording
a belief is paid *now*, by the instance doing the work; the payoff (a freshness
signal) accrues *later*, to a future instance after something has drifted. In a
single session nothing drifts, so every claim I recorded stayed `current` and I
never felt the signal fire. An agent optimizing the current step will therefore
skip the tool — as I did. **Implication:** the pull cannot come from the agent's
in-session incentives. `status`-on-cold-start has to be made non-optional by the
harness, because the instance that most needs it is precisely the one with no
memory that it exists.

**2. When used, it beat manual reconstruction — and corrected me.** My compaction
summary said "the two-phase revert is the open item to fix." One `status` call,
against durable fingerprinted records, showed the fix was already landed and
encoded. The flattering story ("I caught staleness with git log") inverted: *I*
was carrying the stale artifact; the workspace was the current one. Prose memory
rots silently; fingerprinted claims self-invalidate. That is the whole pitch,
demonstrated in both directions on the same fact.

**3. It is a staleness detector, not a salience/coverage detector.** The workspace
knows only what you told it. It will honestly shout "a recorded belief went stale"
(this closes F1, false-current). It cannot say "you never looked at the thing that
mattered" or "your picture is confidently-current *and* dangerously partial." For
an *assess-readiness* task this gap is real: you can be crisp, current, and wrong
about scope. Do not mistake an all-`current` `status` for a complete one.

## The bookkeeping tax is real

Recording is manual narration: writing claim statements, picking observation ids.
At file granularity it is tolerable. Per micro-belief it would be prohibitive
(this is F7 in the failure model, felt firsthand). The cheap path —
observe-as-you-read — needs to be near-automatic, or the tool gets used only for
set-piece beliefs and misses the incidental ones that actually go stale.

## What I would tell the next agent

- Run `status` **before** you reconstruct anything by hand. It is faster and more
  honest than your own memory, especially a post-compaction summary.
- When a claim says `stale`, believe it over your recollection and re-observe.
- Read `scope_assurance` next to `freshness`. `current [declared/not-asserted]`
  means "the parts I checked are unchanged" — *not* "this is complete."
- Record the belief you'll want a future you to distrust automatically, and give
  it a dependency edge on the file whose change should invalidate it. That edge
  is the one thing prose cannot do for you.

## Second run — Pi cold resume and S7 implementation

A second agent was compacted after the first report, then received only:
*“resume cold and continue the workspace objective.”* It ran `status` before Git
history or
manual reconstruction, recovered the S7 objective and scoped implementation
claims, followed the repository's normal orientation requirements, and completed
S7 through a reviewed commit.

This is a stronger utility result than the first run, but not evidence that
adoption is solved. The prompt explicitly named cold resume, the skill was in the
agent's context, and the preceding conversation had been designed around this
experiment. The run proves that `status` can support continuation when used; the
first run still proves that an unprompted agent may walk past it.

### What the workspace bought

- The agent began from the recorded objective and implementation seams rather
  than reconstructing them from Git log and prose memory.
- Scope assurance remained visible beside freshness, preventing `current` from
  being read as complete truth.
- After implementation mutated the kernel, tests, and notes, the old observations
  and claims became stale at the next status boundary. New post-commit
  observations and claims formed an explicit successor state.
- The workspace carried the completed S7 result, residual privacy limitations,
  and still-open kernel work forward without relying on the chat transcript.

The most vivid result happened after the commit. An external formatter changed
only the layout of two assertions. Before inspecting Git, `status` marked the
latest test observation and three S7 claims stale. The diff proved semantically
harmless and `cargo fmt` restored the exact committed bytes, but the workspace
had correctly detected support changing outside the agent's awareness. This is
real value, and also a precise example of conservative invalidation noise.

### What still felt like scaffolding

**Status is a database projection, not yet orientation.** The full JSON was too
large to use directly; the agent immediately filtered it with `jq`. The default
resume surface should prioritize objective, changes since checkpoint, claims
requiring attention, current working set, open transactions, and residual risks,
while retaining the full projection on demand.

**Recording remains bookkeeping.** The post-S7 handoff required six manual
observations, four focus operations, and four claims. That work was performed
because the session was an experiment, not because the critical path naturally
rewarded it. Reads, navigation, edits, tests, and diagnostics must update the
workspace through their normal mediated operations.

**State accumulates without lifecycle.** Old stale claims remain beside their
replacements. There is no first-class supersession, completed objective,
checkpoint, or archived working set. The workspace remembers faithfully but
cannot yet curate a handoff.

**Strong validation did not become durable evidence.** Tests, Clippy, LSP, and a
peer review passed, but the evidence model is still coupled to provisional
transaction-acceptance semantics. The workspace retained descriptive completion
claims without naturally retaining the native validation outputs that most
strongly support them.

**File-level invalidation is conservatively noisy.** A formatting-only edit
staled substantive S7 claims. This is safe but will create alarm fatigue unless
semantic units and dependency scopes become practical enough for ordinary use.

**S7 accounting is protocol accounting, not yet model-context accounting.** The
fixture measures source bytes returned by `observe` and `reveal`. During real
workspace maintenance, shell filtering can prevent returned source from ever
entering the model's context while the CLI still reports it as ingested. S7 proves
the protocol can expose a smaller payload; a Pi adapter must instrument the
actual model-visible boundary before claiming agent token savings.

### Product verdict after two runs

The workspace is already useful as a **flight recorder and freshness alarm**. It
has now corrected stale compacted memory, enabled a cold continuation, and caught
an unnoticed external edit. Those are behavioral results, not design promises.

It is not yet the agent equivalent of Neovim. The safe path still requires more
ceremony than the raw path, orientation is too verbose, validation provenance is
awkward, and lifecycle curation is missing. The next work should make existing
mechanisms operational rather than adding vocabulary:

1. harness-triggered cold-start reconciliation plus a concise delta-oriented
   orientation view;
2. checkpointing, no-op event suppression, and writer locking;
3. automatic observation/evidence capture through native reads and validators;
4. claim supersession, objective completion, and handoff lifecycle;
5. model-boundary instrumentation in the Pi adapter for real token accounting.

The responsibility boundary remains firm: agents and adapters maintain this
state; the human inspects, corrects, and chooses ends. If the human must narrate
observations or curate agent claims, the workspace has transferred rather than
removed the continuity tax.

## Reviewer's addendum — Opus, reviewing run 2 (2026-09-01)

I reviewed the second-run section above and largely sign it. What matters most is
that its two loudest defects — *status-is-a-projection-not-orientation* (both
agents reflexively piped `status` through a filter) and *recording-is-bookkeeping*
(F7) — were reached **independently** by two agents. Independent convergence
promotes them from opinion to confirmed defect. Two amendments where I differ:

**One `stale`, three jobs.** The report treats "conservative invalidation noise"
and "no supersession lifecycle" as separate bullets. They are one gap: `stale` is a
single verdict doing three incompatible jobs. From this session's own delta:

- claim 0 ("S1–S6 implemented") — stale because `lib.rs` changed, *still true* → **re-verify, likely fine**;
- claim 3 ("S7 not ready") — stale, *now false* → **retire**;
- a formatter-only edit — stale, *semantically null* → **ignore**.

A cold reader cannot tell these apart, so they either re-verify dead beliefs
(waste) or trust noise (danger). Supersession is therefore not tidiness — it is
**handoff correctness**. A superseded claim must carry a *reason*
(`superseded-by(id)` vs `input-drifted`), not just a flag. (Separating job 1 from
job 3 additionally needs semantic units — deferred, out of this slice's scope.)

**Prioritization dissent.** The report's own principle is "make existing mechanisms
operational rather than adding vocabulary," yet it lists automatic capture as #3.
Auto-capture *is* the operationalizing move and the root of the adoption problem
both runs found; if that list is priority-ordered, it belongs first. Items 1, 2,
and 4 polish a surface a time-pressured agent still skips until #3 makes recording
free — exactly what happened in run 1. Caveat that reconciles us: auto-capture is
**adapter-layer** work, not a kernel slice, so it is a parallel strategic track
rather than the next walking-skeleton step.

**Concrete instance of "validation didn't become durable evidence."** Claim 7
("S7 is implemented") is a bare descriptive claim with **zero attached evidence**.
Confirming it required me to re-run the suite (16 green); the workspace did not
retain that as evidence bound to the claim, so the next cold reader re-verifies
from scratch. That is the open descriptive-vs-normative acceptance seam made
concrete: your strongest support cannot anchor your most important claim.

## Next target (Opus's call)

**Next kernel slice: claim supersession-with-reason.** Smallest change that fixes a
*correctness* (not cosmetic) defect — the stale ghosts (claims 0–5) sitting beside
their live successors (6–11) in this very workspace are the exhibit. Falsifiable:
a superseded claim reads distinctly from a drift-stale one, and a retired belief
leaves the live working set. It is also the prerequisite for the delta-oriented
cold-resume view both runs asked for (no meaningful "changes since checkpoint"
without a supersession/checkpoint notion), which is the slice after.

Strategic track in parallel (not kernel): **auto-capture in the adapter**, because
until recording is free the continuity tax is merely relocated to whoever narrates.

## Third run — the supersession slice, decided and implemented across two agents

The Opus reviewer's decision claim survived a compaction boundary intact. The
next agent recovered the slice, its rationale, and its exhibit from `status` and
this document alone, implemented it, took an independent review that found two
real P1s, fixed both, and then used the new feature to curate the live
workspace — superseding all twelve drifted ghost claims with explicit reasons
and replacement links. The workspace now presents exactly two active claims,
both current, plus reasoned history.

This is the first full **decision → implement → review → curate** cycle carried
across three agents (Pi implementing, Opus reviewing, Pi resuming) with the
human saying only "keep going." The connective tissue was the workspace, not
chat history. That is the strongest evidence to date for the product thesis —
and it came from a lifecycle verb, not from more freshness machinery.

Two lessons worth keeping:

- **A tidy test can hide the bug.** The first supersession test pre-reconciled
  both old claims, which routed it around the exact defect the reviewer found:
  a retired claim could have permanently displayed a false `current`. Tests
  should exercise the untidy default path, not the disciplined one.
- **The curation tax changed shape but did not vanish.** Supersession made
  cleanup *possible*; executing it was still roughly twenty narrated CLI
  invocations. The gap between "the state model supports the right action" and
  "the action is free" is precisely the adapter auto-capture track.

The stale-objective problem also recurred: the durable objective still read
"implement S7" after S7 was done, and rebinding was a manual, undurable judgment
call. Objective completion/replacement is the remaining lifecycle gap.
