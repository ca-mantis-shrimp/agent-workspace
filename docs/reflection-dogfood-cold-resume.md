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
- Record the belief you'll want a future you to distrust automatically, and give it
  a dependency edge on the file whose change should invalidate it. That edge is the
  one thing prose cannot do for you.
