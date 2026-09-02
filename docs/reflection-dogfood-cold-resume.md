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

## Fourth run — the checkpoint/delta slice, Opus resuming cold

Resumed cold from a compaction. `status` reconstructed the whole picture —
supersession done, both live claims fresh, no open transactions — with no chat
history, so the "orient first" pitch held again. The human relayed two wishes
from other sessions (a delta view, and read-hooking). The useful move was
*refusing to conflate them*: the delta view is a kernel slice already sequenced
after supersession; read-hooking is adapter-layer auto-capture. Building the
delta first is the filter before the auto-capture firehose, not a deferral of it.

Two lessons worth keeping:

- **The delta design was dictated by an existing seam, not chosen freely.**
  `resume_status` reconciles everything and appends a `ClaimReconciled` event on
  every call, so the log grows each `status` and a stale verdict re-emits
  constantly. The obvious "scan for stale-reconcile events after the checkpoint
  sequence" delta would therefore be noise. The sound design — project the log up
  to the checkpoint, project it to now, diff the two — fell out of respecting that
  seam. Reading the code before designing was what surfaced it; a memory-based
  design would have shipped the noisy version.

- **The completion gap closes by making the boundary a first-class thing, not by
  adding a "done" flag.** The prior run left "objective completion" as an open
  lifecycle gap. A checkpoint that snapshots the objective turns rebinding from a
  destructive overwrite into a preserved transition — the delta reports the
  change instead of losing it. Dogfooded live: checkpointing the finished
  supersession objective, then rebinding, then `delta` showed the exact
  transition plus the new claim and observations, with `claims_staled` correctly
  empty because the drift happened before the line was drawn.

Still undone: a checkpoint records the objective but not an explicit "completed"
vs "replaced" disposition, and the ~750-event live log is mostly redundant
reconciliations — no-op suppression is now the loudest unaddressed seam.

## Fifth run — the skill as written, and the suppression slice (Pi)

First run cold-started purely through the skill text: build → `status` → `delta`,
three commands, no memory archaeology. `delta` did the decisive work — it showed
the prior session had bound *no-op event suppression* as the objective with zero
work since the `checkpoint-delta-shipped` line. The objective handoff (bind, then
a later session picks it up) worked exactly as the checkpoint design intended.

The slice itself shipped fast (commit `b8c9856`): all three reconcile seams
recompute every verdict but persist only changed ones; the no-op path skips a
redundant re-projection too. F9 holds by construction — suppression conditions
persistence, never computation, and a test proves an out-of-band edit after a
fully suppressed status is still caught. Live numbers: a status over real edits
appended 24 honest verdict changes; the next status appended zero. The log had
reached ~970 events, most of them redundant reconciliations.

New findings this run:

- **Transient claims are invisible to delta.** A claim recorded and superseded
  entirely inside one window (my 15→16 chain) appears in neither
  `claims_recorded` nor `claims_superseded` — the delta diffs active sets, and
  the claim is active in neither. The durable log keeps it; the resume view
  doesn't. Documented as a known limitation: resumption cares about live
  beliefs.
- **The drift-refresh loop works but is manual.** A post-claim docs edit staled
  claim 16; refreshing via observe → re-claim → supersede (16→17) took three
  commands. Honest, but it's the same bookkeeping tax the adapter auto-capture
  track exists to eliminate.
- **Second formatter-noise incident.** Between turns, an edit-time hook
  reformatted `tests/walking_skeleton.rs` with non-rustfmt style; `cargo fmt
  --check` caught it and it was reverted. The workspace would have flagged it
  too (the claim citing that file would stale). This keeps happening — worth a
  repo-level guard (fmt gate in CI or a pre-commit hook) rather than vigilance.

### Suggested objectives, in order

1. **Writer locking (kernel, small).** Status is the hot path for agent and
   future Neovim projection alike; today two concurrent statuses collide as
   `CorruptLog`. An advisory lock file on the workspace directory that makes
   concurrent writers serialize (or fail with a clear lock error) is a small,
   sharp slice. Candidate statement: "concurrent statuses either serialize or
   fail loud with a lock error, never corrupt the log."
2. **Materialization efficiency (kernel, medium).** Every command replays the
   whole log; `status` replays it once per item plus the append. No-op
   suppression slows growth but replay is still O(log). Options: single-pass
   status (compute all verdicts in one projection, then batch-append the
   changed ones — F9-safe because the projection happens before the append), or
   checkpoint-anchored snapshots + tail replay. Careful: cache *inputs and
   projections*, never verdicts.
3. **Close the kernel action, pivot to the Pi interface (strategic).** The
   biggest product gap is adoption tax: observe/claim/supersede are all manual.
   The first pi-interface slice should be adapter auto-capture — a thin
   extension that records observations when the agent reads files — so the
   workspace fills itself. Suppression and delta make that firehose affordable;
   that is the sequence paying off.
4. **Small lifecycle items (any time).** An explicit "objective completed"
   disposition distinct from "replaced"; a concise `status` mode (delta is the
   resume surface; status at full verbosity is 50KB+).

## Sixth run — writer locking, semantic fingerprinting, and the version-skew close (Opus, 2026-09-01)

Shipped four coherent slices, each verified end-to-end (not just green tests):
a pre-commit rustfmt gate (`29792a7`), writer locking (`6a8e539`), opt-in
rustfmt-normalized fingerprinting (`b18fb4e`), and an exact toolchain pin
(`e4e3f83`). They turned out to be one story about *trusting the freshness
signal* — from the commit boundary inward to the fingerprint itself. Writer
locking (suggested objective #1) is done; the version-skew close was the sealer,
because the same rustfmt skew undermined both the fmt gate and normalized
fingerprints.

### Two opinions for whoever resumes (this is the forward guidance)

1. **The normalize *adoption gap* is the real next work — above materialization
   efficiency.** Normalized fingerprinting shipped opt-in, so the *default*
   `observe` still fingerprints bytes. That means the exact failure that started
   this (a reformat staling a live belief) can recur tomorrow, because nobody
   remembers to pass `--normalize`. The mechanism exists; the lived experience is
   unchanged. A latent feature is one forgotten flag away from dead code. The
   slice: auto-normalize recognized source types by default, with a fast path
   that only shells the formatter when the raw bytes already differ (so the
   common unchanged status pays nothing), plus a fingerprint-scheme version bump
   for the one-time migration. Deferred here deliberately (per-reconcile
   subprocess tax; changing existing fingerprints' meaning) — but it is the thing
   that makes the tool deliver on its promise.
2. **Byte-default fingerprinting is a design smell.** This run built three
   compensations around the same abstraction — the fmt gate, the normalizer, the
   toolchain pin — all because "input changed = bytes changed" is subtly wrong
   for source code. Three workarounds around one assumption is one smell, not
   three features. The honest end state inverts the current default:
   formatter-canonical (semantic) is the default for recognized types, byte-mode
   is the opt-in escape hatch. The incremental path taken here points at that
   inversion; name it so it is not lost.

Process note that earned its keep: "done", a green test, and a success exit code
each lied once this run (a stray fmt regression under a "done"; a toothless test
that passed on an orphan file; a `claim` whose statement the shell corrupted via
backtick substitution). Check the durable artifact, never the claim of it — the
project's own thesis, reflected in how the work goes.

## Seventh run — the auto-normalize inversion, and the tool catching its own incident (Pi, 2026-09-01)

Picked up run 6's bound-but-unstarted objective through the standard cold start
(build, status, delta) and shipped it: `--normalize auto` is now the default,
resolving to rustfmt for `.rs` at capture time with the concrete scheme
persisted per record; claim dependencies auto-detect kernel-side; a raw-byte
fast path skips the formatter subprocess whenever bytes are unchanged;
`--normalize none` is the escape hatch. One deliberate deviation from run 6's
suggestion: **no fingerprint-scheme version bump** — records are
self-describing and only new records get the new default, so old logs replay
byte-identically. A global bump would have invalidated honest byte-mode
history for no gain. 29 tests green, commit `1b70030`.

### The headline: the feature prevented its first false stale within the hour

Minutes after the checkpoint, an edit-time hook reformatted
`tests/walking_skeleton.rs` into *non*-rustfmt-canonical style — the third
formatter-noise incident, same class as the one that produced claim 17's
false stale in run 5. This time the reconcile verdict on the affected
observation stayed **current** ("observed unit unchanged; container changed
outside mediated unit") and claim 27 stayed current, because the canonical
form was unchanged while the raw bytes drifted. Under the old byte-default
this was exactly the reformat-stales-a-belief failure. The byte drift
remained *visible* in the reason string — normalization hides nothing, it
just stops crying wolf. The committed (canonical) bytes were restored and
`cargo fmt --check` passes.

### What felt good

- Cold start is now routine: two commands, zero recap, objective recovered
  from `delta`'s `objective_change` alone. Third consecutive clean handoff.
- The design-claim step (claim 26) earned its keep: writing the design as a
  claim *before* implementing forced the version-bump deviation into the open
  as a recorded decision instead of a silent choice.
- Claim 18's supersede reason wrote itself from the delta evidence — the
  staleness was honest (doc gained a section) *and* the advice was consumed,
  and the tool made both visible.
- The feature dogfooded itself during its own evidence gathering: the
  post-implementation observes auto-recorded `rustfmt` on the `.rs` files and
  `none` on the `.md` with no flags passed.

### Friction that remains

- `status` output is still a wall (a 3909-line JSON this run); `delta` is the
  resume surface but `status` needs its concise mode. Known item, unfixed.
- The observe→claim ritual is still manual and chatty (nine CLI invocations
  this run just for bookkeeping). Adapter auto-capture remains the real fix.
- Long claim statements through a shell CLI remain a footgun (run 6's
  backtick corruption); a `--statement-file` or stdin mode would remove it.
- Wording wrinkle: for a *whole-file* normalized observation, the reason
  "observed unit unchanged; container changed outside mediated unit" is
  accurate but reads oddly — the unit/container vocabulary predates
  normalizers. For whole-file it really means "canonical form unchanged; raw
  bytes differ". A normalizer-aware reason string would read better.
- The live environment keeps lying transiently: the build was red mid-edit
  (struct fields before initializers) and the test-runner reported on that
  superseded state. Durable artifact over live signal, again.

## Eighth run — correcting the plan, then crossing the Pi boundary

Cold start initially produced the wrong next action. The agent treated the
original 2026-08-31 Clearhead predecessor chain as fresher than the later field
evidence and started `working-set`. The human challenged that conclusion. Git
history, workspace objective/checkpoint history, and this reflection all agreed:
run 5 explicitly said “close kernel, pivot to Pi; adapter auto-capture first,”
run 7 repeated it, and `status-cost-closed` recorded the same handoff. The prior
agents had updated every narrative surface except Clearhead. The graph—not the
human's memory—was stale. Commit `147b7bc` repaired it, and claim 34 explicitly
superseded the mistaken claim 33.

That correction matters beyond project hygiene. “Clearhead is authoritative”
cannot mean “an old plan defeats newer executable evidence.” Authority includes
revising the plan through Clearhead when experiments invalidate it. We did not
skip predecessor relationships; we changed the relationship with a reason and
kept the broader capability actions visible.

The first Pi slice then crossed the real boundary rather than wrapping it. A
project-local extension observes native `read` calls and records only finalized
context text it can match to repository bytes. A real Pi run read three lines of
`README.md`; the workspace automatically gained observation 77 (`pi.read`, range
`18:216`, 198 source bytes, 250 model-boundary bytes, no retained payload). That
is the first time ordinary agent behavior populated the workspace without a
manual `observe` command—the adoption-tax thesis finally moved from prose into
running code.

The independent review was valuable precisely because the happy-path dogfood
passed. It found four boundary defects: sensitive directories were not matched,
invalid UTF-8 could create lossy offsets, selected bytes had a second-read race,
and `message_end` was not the latest available context boundary. The close fixes
canonicalize symlink targets before policy checks, reject invalid UTF-8, pass an
expected raw selector fingerprint for kernel-side compare-before-append, and
capture from `context`. One limit remains honest: a later-loaded context handler
can still alter messages, and an out-of-range concurrent edit can change the
container fingerprint while the selected unit stays valid. Pi needs a final
context hook or provider-snapshot import to close those windows soundly.

One process wart also repeated: the harness formatter again rewrote unrelated
Rust assertions non-canonically after an edit. The tracked fmt gate caught it;
`cargo fmt` restored the canonical form immediately before commit. The separate
environment action remains justified.

## Ninth run — the workspace oriented its own developer (Pi, 2026-09-01)

The cold-start workflow ran as designed, and the interesting part happened
before any deliberate work: the read auto-capture slice from run 8 recorded my
reads of the skill file and the extension sources (observations 87–89) while I
was still orienting. No manual `observe`, no ceremony — ordinary orientation
populated the workspace. The adoption-tax thesis now holds in both directions:
the tool no longer depends on the agent remembering it, because it is watching.

The slice itself was deliberately boring by design: two custom tools,
`workspace_status` and `workspace_delta`, that exec the kernel binary and return
its JSON verbatim. Everything semantic stays in the kernel; the extension maps
arguments and nothing else. The one genuinely new judgment call was error
policy: Pi custom tools have no `isError` field, so throwing is the only error
channel, and I had to decide which conditions deserve to be errors. Expected
environment conditions (a directory outside any Git checkout) return plain
text; only a failed kernel invocation throws. An agent running these tools in a
random repository gets a usable answer, not a stack trace.

Two things I would flag for the next agent:

1. **The dogfood test and the tool under test are now the same mechanism.** The
   acceptance question for this slice — "can a fresh agent orient without
   shelling to the CLI?" — was answered by launching a fresh `pi -p` session
   that used only the tools. It reported the objective, claim freshness, and
   latest checkpoint correctly and confirmed it needed no CLI. But note what
   that test cannot see: it verifies that the *answers* flow, not that the
   agent *trusts* them under pressure. The cold-start failure mode from the
   first reflection (walking past `status` to reconstruct by hand) is now
   harder but not impossible — the tools appear in the tool list, but an agent
   mid-task may still never call them. The next real signal will be a session
   that *ignores* the tools and whether anything catches that.
2. **Claim supersession fired for the right reason this time.** Shipping the
   slice staled the run-8 handoff umbrella (claim 37) — its "next objective"
   was consumed and its supporting actions-file input changed. That is exactly
   the "genuinely outdated, not drift-stale" case the skill describes, and the
   supersession with reason 39 left an inspectable trail. The workspace
   managed its own handoff correctness without human correction, which run 8
   needed a human to force.

Also observed: the harness edit-time formatter struck again, this time on the
TypeScript sources (the same chained-one-liner collapse seen in runs 5–7). The
`git diff -w --stat` comparison confirmed it was whitespace-only and the
working tree was restored from the committed form. Claims 38/39 stayed `current`
through it — the rustfmt-normalized-fingerprint lesson generalizes, but note it
is doing its job: without normalization the same drift would have staled the
claims while meaning nothing.

Residuals carried forward unchanged: later-loaded context middleware can alter
the read projection; outside-range concurrent edits can affect container
fingerprints; auto-capture skips are silent; and orientation tool results are
not themselves observed. The bound next decision is whether the loop verbs
(observe, claim, transaction lifecycle) deserve the same projection, or whether
the interface moves to the Neovim cockpit.
