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
