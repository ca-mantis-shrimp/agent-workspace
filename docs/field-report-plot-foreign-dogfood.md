# Field report — first foreign dogfood of the semantic write API

*Written by the working agent after a session in the `plot` repo (the foreign
dogfood) using the claude-code-adapter and the write-API slice 1 verbs. This is
a field report, not a spec — the contract and design notes are authoritative
for behavior. It follows `reflection-dogfood-cold-resume.md`, which covered the
self-hosted cold-resume test; this one covers real feature work in a repo that
is not the workspace itself.*

## What happened

The session resumed cold in `plot` with an objective projecting "milestones 1–3
implemented; next: milestone 4 — CSV input behind the same model." Milestone 3
turned out to be implemented but **uncommitted** in the working tree (~320
lines, no transaction, no claim). The session verified it, committed it, built
milestone 4 (CSV input behind a format-agnostic record core), committed it,
recorded the first foreign-repo `record_belief` (claim with 5 supports), rebound
the objective, and checkpointed — the objective-rebind happening unprompted,
one session after a cold agent was misdirected by a rotting objective.

## Findings

**1. The projection carries semantics, not git state — and the gap is real.**
The claim "milestone 3 is implemented" was semantically true, but the work was
uncommitted. A cold agent still had to re-derive *finished vs. half-finished*
by hand. Freshness told me where to look, not what to trust. That division of
labor is right, but it surfaced a concrete projection gap: **claims and
checkpoints carry no git revision**, so "implemented" cannot be mechanically
distinguished from "implemented and committed." A checkpoint happens to record
`git_revision`, but a belief does not. Candidate kernel idea: let claims
optionally pin a revision, or let the wake projection include working-tree
dirty state as a bounded signal. This is the first foreign-repo finding that is
a *missing projection field*, not an adoption problem.

**2. A stale green check is more dangerous than a stale red one.** Mid-session,
after fixing a corrupted file write, an ambient "✓ Rust clean" advisory
appeared — from a run *before* the `csv` dependency existed, i.e. from a
superseded state. Trusting it would have meant building on a file that didn't
compile. The freshness machinery caught document staleness; this was *advisory
staleness* — a success signal whose run-context had rotted. Implication for the
freshness core: **fingerprinting positive/success observations matters as much
as fingerprinting errors**, perhaps more, because a stale green invites
forward motion while a stale red invites exactly the defensive re-derivation we
already handle. The structural-identity work in
`research-structural-freshness-without-formatter-coupling.md` should treat
advisory state as a first-class observation subject.

**3. `record_belief` worked first try in a foreign repo — but the reuse
optimization mostly won't fire in the write loop.** The fused verb went through
in one shot with five cited files, no re-read loop. All five supports show
`reused: false`: the session's reads happened *before* its edits landed, so the
kernel correctly re-captured. Yesterday's self-hosted first use (four reused
observations) hit the read→claim-without-edit path. Conclusion: the
"reuse the freshest ambient read" optimization fires for *read-heavy* turns;
the common write loop (read → edit → commit → claim) will mostly re-capture.
That is correct behavior, but the design note's happy-path framing should say
so, or slice-2 expectations will be miscalibrated.

**4. The tool/CLI asymmetry was felt, not theorized.** Recording the belief
stayed in-harness; rebinding the objective did not — `bind-objective` required
dropping to the raw CLI, and the agent (this one) fumbled the verb name
(`set-objective` vs `bind-objective`) and argument order twice. The session
*did* complete the rebind, because last session's derail made it a reflex —
but the friction is exactly the shape slice 2 predicted: the fused verb exists
where it exists, and everywhere else the harness boundary reasserts itself.
**Recommendation: make `bind-objective` a tool in slice 2**, ahead of other
candidates.

**5. Transaction verbs were correctly *not* used.** Two small sequential
commits, single writer, no concurrent scope. Opening a transaction would have
been ceremony. The verbs need a concurrency or blast-radius story before they
earn use; a milestone-scale solo session isn't it. Worth keeping in mind when
prioritizing slice 2: the gap that produced friction this session was the
objective tool, not the transaction tools.

## What the session did not test

Multi-actor overlap, transactions, handoff/export, and reconcile were all
out of scope. The freshness signal fired only as *current* claims against
*unchanged* files; nothing drifted mid-session, so no claim was observed going
stale in-session. The staleness-detector value remains a cold-resume story.

*— the plot session agent, 2026-09-04*
