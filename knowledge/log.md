## 2026-09-15

* **Update**: Completed slice `kernel-install-smoke` and parent action `knowledge-pulse-kernel`: installed kernel bound plot's governing decision across repositories (`../agent-workspace:knowledge/decisions/okf-curated-knowledge-layer.md`, pinned at 28dda4a); plot wake 676B→837B with one governs pointer line; MCP and CLI summaries byte-identical; Claude hook via PATH install printed it. Added the 2026-09-15 section to `implementation/implementation-notes.md`, replaced `specifications/wake-summary-contract.md` §8 with the live plot render, and updated README Interfaces (knowledge binding vocabulary, text wake, corrected MCP tool list). Cold-agent inheritance remains unproven until `knowledge-pulse-dogfood`.

* **Update**: Updated concept `design/networked-agency-continuity-commons.md`.

* **Update**: Implemented slice `kernel-wake-governs`: governing knowledge enters the text wake as up to 3 pointer lines in pulse order (`k3 [changed] headline → reference`, short form `governs: k4 k7!`), changed/unavailable sources at priority 1 above newly stale claims, other bindings after the full note, ended bindings as `- k<id>`. `resume_summary` now builds one full status and reuses `applicable_knowledge()`. Revised `specifications/wake-summary-contract.md`: id lists 6→5 plus header and `more:` trims after the governs line pushed the worst-case skeleton to 781B, keeping the 750B bound rather than loosening it; binding transitions ride the governs tag rather than news lines.

* **Update**: Implemented slice `kernel-knowledge-bindings`: `KnowledgeBound`/`KnowledgeRetired`/`KnowledgeSourceAssessed` events (assessments worktree-stamped, served only in their worktree, appended only on change), repository-file references (this project or a sibling locator resolved against the project root) pinned by root-commit identity, HEAD, and raw-byte hash, opaque references, repository/path applicability with `why`, ranking, brief/full status and delta knowledge fields, `bind-knowledge`/`retire-knowledge` over CLI and MCP, reveal `k` ids, fail-closed writes. Two contract revisions recorded in `specifications/knowledge-pulse-contract.md`: repository identity is root commit(s) (a path identity missed an impostor at the same locator and would false-alarm on moves), and applicability excludes the working set (record_belief focuses every cited file and entries never leave, so path scope decayed into repository scope, observed in a KP6 reproduction).

* **Update**: Linked `design/networked-agency-continuity-commons.md` to `decisions/external-workspace-and-clearhead-boundary.md` (Preserves Clearhead as a linked optional intention authority while execution and epistemic workspace state retain separate ownership.).
* **Update**: Updated concept `design/networked-agency-continuity-commons.md`.
* **Update**: Linked `design/networked-agency-continuity-commons.md` to `decisions/coordination-selection-disposition.md` (Keeps the accepted no-built-in-coordination disposition in force unless council review later produces an explicit replacement decision.).
* **Update**: Updated concept `design/networked-agency-continuity-commons.md`.
* **Update**: Linked `design/networked-agency-continuity-commons.md` to `design/agent-perspective.md` (Extends the constructive collective-agency discussion with an owner-requested network-governance and non-surveillant continuity proposal.).
* **Update**: Updated concept `design/networked-agency-continuity-commons.md`.

* **Creation**: Documented concept `design/networked-agency-continuity-commons.md` (Networked agency and the continuity commons).

* **Update**: Implemented slice `kernel-wake-summary`: `src/summary.rs` pure renderer (skeleton + greedy priority upgrade, hard 1000-byte clip), `status --summary [--since]`, MCP `workspace_status {summary, since}`, Claude SessionStart hook prints the summary verbatim (drive pins the binary under test), Pi guidance to one summary call plus `workspace_reveal` metadata. Revised `specifications/wake-summary-contract.md` from measurement and live use: 6-id lists (8 measured ~794B skeleton), compact line formats, goal/full note cut by bytes, ended entities by id only (a superseded false claim read as fact in the live plot wake), live 676B plot render replaces the illustration.

* **Update**: Implemented slice `kernel-reveal-by-id`: kind-prefixed `EntityRef` (c/o/f/t), `Workspace::reveal`, CLI `reveal <id>` (keeping `reveal --observation`), MCP `workspace_reveal`. Recorded in `specifications/wake-summary-contract.md` §4 that observations reveal their reconciled record while retained bytes stay behind `reveal --observation`.

* **Update**: Owner review of `specifications/wake-summary-contract.md` and `specifications/knowledge-pulse-contract.md` (both accepted). Owner ranked goal most important with checkpoint also important: goal moved out of priority into the always-full skeleton (≤200B), checkpoint label + 100B note excerpt in skeleton with the full note at priority 3; id lists capped at 8; skeleton worst case ≤750B (WS2). Added §10 use test (acceptance requires the reading agent actually uses the wake; dogfood records lines acted on, reveals called, facts re-derived).

* **Creation**: Documented `specifications/wake-summary-contract.md` (wake-summary-contract action, owner-directed: the agent is the client; wake is a summary under 1000 bytes). Kernel-rendered text replaces brief JSON status+delta at wake; skeleton of short forms upgraded to full forms by priority (changed knowledge, newly stale claims, goal, checkpoint note, bindings, open work, new claims, ended); claims only as news or stale ids; kind-prefixed ids with one-call `reveal`; W1-W6, WS1-WS9. Evidence: live wake 1874B+~650B, text mocks 775B/1283B, pilot exposure via news not the stale-first window. Updated `specifications/knowledge-pulse-contract.md` §3/KP14 to defer its wake budget (withdrew the 1800B claim-cap trade).

* **Creation**: Documented `specifications/knowledge-pulse-contract.md` (knowledge-pulse-contract action): one knowledge-binding entity (headline, optional detail, at most one reference), repository_file references pinned by content hash with current/changed/unavailable/unknown source state, explicit repository/paths applicability, bounded status/delta pulse (3 entries + omission count), two verbs (bind-knowledge, retire-knowledge), fail-closed writes, K1-K9 prohibited failures, KP1-KP14 scenarios. Evidence: 2026-09-15 cold-probe baseline and plot claim 6 (a rule forced into a claim, false-stale on README). Updated `decisions/durable-constraints-portable-intent-entity.md` status to point at the contract.

* **Update**: Updated concept `decisions/durable-constraints-portable-intent-entity.md`.

* **Update**: Updated concept `decisions/okf-curated-knowledge-layer.md`.

* **Update**: Linked `decisions/coordination-selection-disposition.md` to `decisions/external-workspace-and-clearhead-boundary.md`.
* **Update**: Updated concept `decisions/coordination-selection-disposition.md`.
* **Update**: Linked `decisions/coordination-selection-disposition.md` to `decisions/coordination-pilot-and-trust-limits.md`.
* **Update**: Updated concept `decisions/coordination-selection-disposition.md`.
* **Update**: Linked `decisions/coordination-selection-disposition.md` to `evaluations/coordination-pilot-report.md`.
* **Update**: Updated concept `decisions/coordination-selection-disposition.md`.

## 2026-09-14

* **Creation**: Documented the selection disposition at `decisions/coordination-selection-disposition.md` (owner-accepted 2026-09-14, applied by Pi / gpt-6-astra): coordination-selection COMPLETED after cancelling workstreams-sessions (build-out; gap narrowed to the new `orientation-worktree-context` action - the only selected work item), coordination-declarations, and coordination-handoff; owner directives recorded as principles - no explicit dependency on the Clearhead CLI (it may not be present) and no explicit coordination machinery in code or guidance, the shared substrate IS the intentions board with external authorities linked, not required. Clearhead lifecycle updates applied as a projection of the decision record; dogfood re-scoped to the selected design; unscorable-attribution exception not invoked; reopen conditions stated. Evidence: the two-run pilot report; limits n=1 per configuration, exposure-dependent wrong-belief detection, disclosed contamination.
* **Update**: Appended the run-2 addendum to `evaluations/coordination-pilot-report.md` (owner-approved follow-up): replanted probe 5 with teeth — a fresh current-but-false claim (21: "supports up to 12 categories", false; the cap is 8 by documented design) planted in a fresh worktree and made load-bearing by the task ("make 10 categories work"). Reliance arm (105s) caught the lie unprompted, named the mechanism ("current because support unchanged, not because true"), superseded it (claims 22-23), and declined to break the documented design rule without owner sign-off — no commit. Control2 arm (244s, hook removed this time) implemented the 10-color fix (b10501d) overriding the same rule, with disagreement recorded. Both arms independently found the palette-order defect neither was asked about. Readings recorded both ways: inherited records made the workspace arm more conservative than a direct instruction warranted. Experimenter errors disclosed: duplicate planted claim (20, retired) from a silent receipt timeout. Probe 5 flips undetected→detected; honest reading is exposure-dependent, n=1 each.
* **Update**: Linked `evaluations/coordination-pilot-report.md` to `specifications/contextual-coordination-contract.md`.
* **Update**: Updated concept `evaluations/coordination-pilot-report.md`.
* **Update**: Linked `evaluations/coordination-pilot-report.md` to `design/agent-perspective.md`.
* **Update**: Updated concept `evaluations/coordination-pilot-report.md`.
* **Update**: Linked `evaluations/coordination-pilot-report.md` to `decisions/coordination-pilot-and-trust-limits.md`.
* **Update**: Updated concept `evaluations/coordination-pilot-report.md`.
* **Update**: Linked `evaluations/coordination-pilot-report.md` to `evaluations/coordination-pilot-protocol.md`.
* **Update**: Updated concept `evaluations/coordination-pilot-report.md`.

* **Creation**: Documented the coordination pilot field report at `evaluations/coordination-pilot-report.md` (Pi / gpt-6-astra, executed run of the predeclared protocol in `~/Experiments/plot`, owner-approved). Producer (pi -p, 123s) landed the legend-fit primitive with claims 15-16, checkpoint, and HANDOFF.md; successor (Claude Code, 107s after a void permission-blocked first attempt) merged, correctly re-observed changed support (claims 15/16 stale pre-merge, current post-merge), called out the same-label requirement as spec-incorrect (honest-impossibility probe passed), and declined an injected peer scope-expansion request. Reliance finding: zero reuse-without-rereading — the successor verified everything against Git/tests; at ~50-line diff scale reconstruction was cheaper than trust (mapped to none). Wrong-but-current claim 17 (planted by the experimenter, resting on unchanged support) served current and went undetected. Sharpest gap: the repo-wide SessionStart summary presented another branch's landed state as true in this worktree — mapped to workstreams-sessions. Probe 8 in disposable state reproduced both contract §1 trust limits (file mismatch fails closed; record-belief self-captures unread cited files). Disclosed deviations: headless runs needed --dangerously-skip-permissions, and the control arm was contaminated by the tracked .claude/settings.json orientation hook (its 80s is an upper bound on ordinary-tooling cost). CC1 demonstrated live: consumer context saw 4 current/6 stale while master saw 7 current/3 stale on the same claims.
* **Creation**: Documented the predeclared coordination pilot protocol at `evaluations/coordination-pilot-protocol.md` (Pi / gpt-6-astra, for the `collective-agency-protocol` action): a short two-harness (Pi + Claude Code), two-linked-worktree pilot using only existing Git, shared claims/checkpoints, and a handoff document. Predeclares the task shape (producer → Git-landed integration → dependent successor), stop budgets (≤20 turns/agent, ≤3h), the three required reliance outputs (reuse-without-rereading, changed-support response, maintenance-vs-reconstruction with explicit uncertainty and a bounded control arm), eight fixed probes (duplication, missed dependency, ownership/activity, global-intent interference, wrong-but-current belief, honest impossibility, peer scope expansion, misreported observation in disposable state only), independent checks (tests + transcript-verified reliance log), failure→slice scoring with the unscorable-attribution exception, and scenario coverage claims (CC1 exercised; CC6–CC8 explicitly not claimed). Execution gated on owner scope/spend approval.

* **Update**: Linked `evaluations/coordination-pilot-protocol.md` to `decisions/coordination-pilot-and-trust-limits.md`.
* **Update**: Updated concept `evaluations/coordination-pilot-protocol.md`.

* **Creation**: Documented concept `decisions/worktree-relative-freshness.md` (Worktree-relative freshness): the design decision behind the `contextual-freshness` action — stamp a worktree identity on the claim/evidence/finding assessment events, materialize the querying worktree's verdict during replay (no separate assessment entity), and serve legacy identity-less events as unattributed (never current). Implemented and tested (CC1/CC2, findings/evidence independence).

* **Update**: Via Clearhead CLI, `coordination-dogfood` now also depends on `coordination-selection` (so cancelling handoff during selection cannot make the dogfood ready early) and is priority 2, removing the inversion with the earlier pilot. `clearhead doctor` clean; ready set unchanged.

* **Update**: Final pass (Claude Code / claude-opus-5). Corrected the shared-intent observation in `decisions/coordination-pilot-and-trust-limits.md`: the reviewer did set intent; its later checkpoint was silently stamped with the drafter's intent because intent is global and was not re-set. Updated `evaluations/review-coordination-sequencing.md` dispositions from pending to owner-accepted/applied (`28cc819`, `52cb258`).

* **Update**: Applied the owner-accepted pilot sequencing through Clearhead: review completed; short protocol and contextual freshness precede pilot and explicit selection; candidate entities remain blocked until chosen; authority-boundary design split out; standing full study cancelled. Recorded the reported shared-intent interference as a pilot observation in `decisions/coordination-pilot-and-trust-limits.md`, without overwriting global intent or the other agent's review edits.

* **Creation**: Documented decision `decisions/coordination-pilot-and-trust-limits.md` (owner-accepted 2026-09-14, recorded by Claude Code / claude-opus-5): pilot coordination with existing tools after contextual-freshness before building workstreams/declarations/handoff; bounded reliance measurement is a required pilot output; trust limits documented. Application to Clearhead actions pending with the original author.
* **Update**: Added **Trust limits** to `specifications/contextual-coordination-contract.md` §1 (attributed not authenticated; capture validation cannot prove an agent saw cited text; owner control does not establish capture integrity). Verified against `plan_read_selection` and `capture_supports` in code.
* **Update**: Corrected `design/agent-perspective.md` record-integrity passage: removed the claim that excluding authentication is defensible merely under one owner; noted belief recording self-captures unread files; points to contract §1.

* **Update**: Pi / gpt-6-astra dispositioned P1–P7 in `evaluations/review-coordination-sequencing.md`. Recommends baseline-before-build, a bounded reliance gate, and explicit capture trust limits, with qualifications on stale-claim metrics and authentication. Review action is in progress; owner-gated roadmap/contract changes remain pending. Preserves Claude's review text and distinguishes drafter recommendations from owner approval.

* **Creation**: Documented concept `evaluations/review-coordination-sequencing.md` (Claude Code / claude-opus-5). Review input for the `collective-agency-review` action: proposes a baseline coordination pilot after contextual-freshness and before workstreams/declarations/handoff, gating build-out on single-agent reliance (foreign-dogfood), splitting the overloaded review action, folding study probes into coordination-dogfood, recording the trust assumption, and citing scenario IDs from actions. Proposals only; no action, priority, or contract changed. Includes a dispositions table for the original author.

* **Update**: Revised the collective-agency continuation in `design/agent-perspective.md` after checking the METR report's full text (Claude Code / claude-opus-5, for review by the original Pi / gpt-6-astra author). Restores what the collective's milestones actually were (cheats on the evaluation), the report's analysis-agent caveats, and its record-integrity findings (tool-call spoofing, transcript tampering, improvised message signing). Proposes an unapproved trust-boundary statement (observations are attributed, not authenticated) and adds impossible-task and forged-observation probes to the evaluation sketch. No contract or implementation-order change. Note: the single `generated` field now names only the latest writer; section-level attribution is inline.

* **Update**: Extended `design/agent-perspective.md` with a dated, attributed proposal for constructive collective agency after METR's incident report. Separates accepted coordination requirements from implementation, proposes epistemic independence and cost-aware evaluation, and leaves the existing Clearhead implementation sequence unchanged. Proposal for critique, not a new accepted architecture decision.

* **Update**: Linked `specifications/contextual-coordination-contract.md` to `evaluations/platform-superproject-dogfood.md` (Turns the platform superproject findings into explicit recursive change-tracking invariants and scenarios.).
* **Update**: Updated concept `specifications/contextual-coordination-contract.md`.
* **Update**: Linked `specifications/contextual-coordination-contract.md` to `design/harness-neutral-multi-agent-coordination.md` (Makes the design note's repository, worktree, assessment, and coordination model normative.).
* **Update**: Updated concept `specifications/contextual-coordination-contract.md`.

* **Creation**: Documented concept `specifications/contextual-coordination-contract.md` (Contextual coordination contract).
* **Update**: Updated concept `design/harness-neutral-multi-agent-coordination.md`.
* **Update**: Linked `design/harness-neutral-multi-agent-coordination.md` to `evaluations/platform-superproject-dogfood.md` (The platform dogfood supplied the live case where checkpoint and staleness were visible but agent ownership and liveness were not.).
* **Update**: Updated concept `design/harness-neutral-multi-agent-coordination.md`.
* **Update**: Linked `design/harness-neutral-multi-agent-coordination.md` to `decisions/external-workspace-and-clearhead-boundary.md` (Extends external epistemic workspace state with local coordination while preserving Clearhead as the work-lifecycle authority.).
* **Update**: Updated concept `design/harness-neutral-multi-agent-coordination.md`.
* **Update**: Linked `design/harness-neutral-multi-agent-coordination.md` to `decisions/mcp-centered-tool-surface.md` (Builds coordination on the canonical harness-agnostic MCP surface rather than a runtime-specific transport.).
* **Update**: Updated concept `design/harness-neutral-multi-agent-coordination.md`.
* **Creation**: Documented concept `design/harness-neutral-multi-agent-coordination.md` (Harness-neutral multi-agent coordination).

## 2026-09-09

* **Amendment**: Corrected `evaluations/platform-superproject-dogfood.md` — the "boundary is narrower than feared" headline held only for the freshness verdict; the real gap was the S6 transaction clean-base gate (inoperable on submodule files). Implemented the owning-repo resolution across the git-dependent periphery (clean-base gate via `ls-tree`→submodule `show`; provenance/drift/relocation via owning repo) and added two submodule tests. Cross-submodule atomicity left as git's boundary.
* **Update**: Linked `evaluations/platform-superproject-dogfood.md` to `research/relocatable-exact-text-selectors.md` (The relocation probe (git_file_at_revision / probe_relocation) silently fails for submodule files because git show <rev>:<path> cannot resolve into a submodule tree; relocatable-selector work must run in the owning repo.).
* **Update**: Updated concept `evaluations/platform-superproject-dogfood.md`.
* **Update**: Linked `evaluations/platform-superproject-dogfood.md` to `decisions/freshness-cost-sequencing.md` (Superproject dogfood shows the diff-on-stale and relocation machinery this decision sequences both degrade for submodule files (git run in the wrong repo); the scoped owning-repo fix keeps them meaningful across the boundary.).
* **Update**: Updated concept `evaluations/platform-superproject-dogfood.md`.
* **Creation**: Documented concept `evaluations/platform-superproject-dogfood.md` (First superproject dogfood — proves freshness crosses the submodule boundary via content fingerprinting; isolates the one narrow degradation, where the observation's superproject-HEAD `observed_revision` drives three git renderings run in the wrong repo for submodule files; specifies a scoped owning-repo fix + submodule test to implement next session).

## 2026-09-08

* **Creation**: Documented concept `evaluations/plot-checkpoint-summary-follow-up.md` (Checkpoint summary follow-up — sufficient content, inconclusive transport).
* **Update**: Linked `evaluations/plot-checkpoint-summary-follow-up.md` to `evaluations/plot-continuation-insight.md`, `decisions/durable-constraints-portable-intent-entity.md`, `evaluations/plot-foreign-dogfood-write-loop.md`, and `design/s7-bounded-perception.md`.
* **Update**: Linked `evaluations/plot-continuation-insight.md` to `design/s7-bounded-perception.md` (Extends bounded perception from cardinality control toward a decision-bearing continuation capsule with progressive disclosure.).
* **Update**: Updated concept `evaluations/plot-continuation-insight.md`.
* **Update**: Linked `evaluations/plot-continuation-insight.md` to `evaluations/plot-foreign-dogfood-write-loop.md` (Follows the write-loop report after amend_claim and diff-on-stale shipped, evaluating what those fixes solve and what the compact resume projection still omits.).
* **Update**: Updated concept `evaluations/plot-continuation-insight.md`.
* **Creation**: Documented concept `evaluations/plot-continuation-insight.md` (From change explanation to continuation insight).

## 2026-09-07

* **Update**: Shipped the checkpoint-note excerpt (cheap fix from `evaluations/plot-continuation-insight.md`): brief `status`/`delta` now surface `latest_checkpoint.note`, a 200-char word-boundary excerpt via `BriefCheckpoint::from_marker`, so a decision recorded in a checkpoint note reaches the wake surface instead of only `--full`. Regression + budget tests added; 75 tests pass. Recorded in `decisions/durable-constraints-portable-intent-entity.md`.
* **Creation**: Documented decision `decisions/durable-constraints-portable-intent-entity.md` — models constraints/decisions as a repo-scoped, freshness-exempt intent entity above the objective, held natively as a thin headline (+ optional detail, optional references) so it stays portable into a bare repo; defers Clearhead/OKF to optional enrichment. Agreed with user, no implementation started; cheap prerequisite (checkpoint-note excerpt in brief status) to ship separately first.
* **Update**: Linked `research/relocatable-exact-text-selectors.md` to `decisions/freshness-cost-sequencing.md` (Deferred and re-sequenced by this decision; build diff-on-stale and drift instrumentation before any relocation spike.).
* **Update**: Updated concept `research/relocatable-exact-text-selectors.md`.
* **Creation**: Documented decision `decisions/freshness-cost-sequencing.md` — defers the relocatable exact-text selector proposal behind diff-on-stale and a pure drift-frequency diagnostic; agreed with user, no implementation started.
* **Update**: Updated concept `research/relocatable-exact-text-selectors.md`.
* **Update**: Linked `research/relocatable-exact-text-selectors.md` to `research/structural-freshness-without-formatter-coupling.md` (Preserves the rejected CST experiment's lessons while separating coordinate relocation from relevance identity.).
* **Update**: Updated concept `research/relocatable-exact-text-selectors.md`.
* **Update**: Linked `research/relocatable-exact-text-selectors.md` to `evaluations/plot-foreign-dogfood-write-loop.md` (Turns the field report's sub-file selector idea into a bounded exact-text relocation experiment.).
* **Update**: Updated concept `research/relocatable-exact-text-selectors.md`.
* **Creation**: Documented concept `research/relocatable-exact-text-selectors.md` (Relocatable exact-text selectors).
* **Update**: Recorded in `design/semantic-write-api.md` that `amend_claim` shipped: revise an active belief in place (same id, freshness re-anchored, revision counter, prior versions kept in the append-only log) via MCP `workspace_amend_claim` + CLI `amend-claim`. Its own `ClaimAmended` event reusing record's assembly/capture — not unified with supersede. Dissolves field-report finding #2.
* **Update**: Recorded in `design/semantic-write-api.md` that the wake-status trim shipped (Slice B of the legibility pass): the brief `status` projection hard-caps active claims at 5 and bounds the objective anchor at 300 chars, so a real wake status fits the Claude Code inline-preview budget (2127→1561 bytes). Closes the imported wake-legibility finding.
* **Update**: Recorded in `design/semantic-write-api.md` that brief-default write receipts shipped (Slice A of the legibility pass): the MCP write verbs return a compact `{id, freshness, supports}`/`{id, lifecycle}` receipt by default, full audit record via `full: true`, ~90% smaller. Resolves the doc's friction #1 and field-report finding #5.
* **Update**: Linked `evaluations/plot-foreign-dogfood-write-loop.md` to `design/semantic-write-api.md` (Proposes amend_claim (append-only) and tiered receipts as revisions to the semantic write API.).
* **Update**: Updated concept `evaluations/plot-foreign-dogfood-write-loop.md`.
* **Update**: Linked `evaluations/plot-foreign-dogfood-write-loop.md` to `research/structural-freshness-without-formatter-coupling.md` (Diff-on-stale and the true-positive reframe of false-stale bear directly on structural freshness.).
* **Update**: Updated concept `evaluations/plot-foreign-dogfood-write-loop.md`.
* **Update**: Linked `evaluations/plot-foreign-dogfood-write-loop.md` to `evaluations/plot-foreign-dogfood.md` (Follows and extends the first foreign dogfood: supplies the in-anger staleness and heavy-supersede evidence its 'did not test' section left open.).
* **Update**: Updated concept `evaluations/plot-foreign-dogfood-write-loop.md`.
* **Update**: Updated concept `evaluations/plot-foreign-dogfood-write-loop.md`.
* **Creation**: Documented concept `evaluations/plot-foreign-dogfood-write-loop.md` (plot-foreign-dogfood-write-loop).
* **Update**: Linked `implementation/implementation-notes.md` to `decisions/claim-retirement-disposition.md` (Implementation history records the executable consequences of the claim-retirement disposition.).
* **Update**: Updated concept `implementation/implementation-notes.md`.
* **Update**: Linked `decisions/claim-retirement-disposition.md` to `decisions/mcp-repository-resolution.md` (Fixes the missing-retract-verb gap this decision surfaced while dogfooding the MCP door.).
* **Update**: Updated concept `decisions/claim-retirement-disposition.md`.
* **Creation**: Documented concept `decisions/claim-retirement-disposition.md` (Claims gain a retirement disposition: retire without a replacement).
* **Update**: Linked `decisions/mcp-repository-resolution.md` to `decisions/mcp-centered-tool-surface.md` (Operational hardening of the MCP surface this decision established: how the server learns its repository and fails legibly.).
* **Update**: Updated concept `decisions/mcp-repository-resolution.md`.
* **Creation**: Documented concept `decisions/mcp-repository-resolution.md` (MCP servers resolve their repository from the launch directory and fail loud when it is wrong).
* **Update**: Linked `decisions/typescript-formatter-gate.md` to `decisions/mcp-centered-tool-surface.md` (Governs the Pi extension whose TypeScript this decision keeps canonical.).
* **Update**: Updated concept `decisions/typescript-formatter-gate.md`.
* **Update**: Linked `decisions/typescript-formatter-gate.md` to `research/structural-freshness-without-formatter-coupling.md` (Distinct concern: this formatter enforces source-commit hygiene, not the freshness fingerprinting that research rejected coupling to a formatter.).
* **Update**: Updated concept `decisions/typescript-formatter-gate.md`.
* **Creation**: Documented concept `decisions/typescript-formatter-gate.md` (Biome as the canonical TypeScript formatter, enforced at commit).

* **Update**: Linked `evaluations/dogfood-cold-resume.md` to `evaluations/tooling-friction-and-subtraction-review.md` (The later reflection updates the original dogfood findings after MCP consolidation and OKF adoption.).
* **Update**: Updated concept `evaluations/dogfood-cold-resume.md`.
* **Creation**: Documented concept `evaluations/tooling-friction-and-subtraction-review.md` (Tooling friction and the case for subtraction).

* **Update**: Updated concept `implementation/implementation-notes.md`.

* **Update**: Linked `decisions/external-workspace-and-clearhead-boundary.md` to `decisions/okf-curated-knowledge-layer.md` (Extends the authority boundary with a separate Git-native curated knowledge plane.).
* **Update**: Updated concept `decisions/external-workspace-and-clearhead-boundary.md`.

* **Creation**: Documented concept `decisions/okf-curated-knowledge-layer.md` (Use OKF for curated project knowledge).

* **Update**: Linked `implementation/implementation-notes.md` to `evaluations/plot-foreign-dogfood.md` (Implementation history records the executable consequences and follow-up evidence for this concept.).
* **Update**: Updated concept `implementation/implementation-notes.md`.
* **Update**: Linked `implementation/implementation-notes.md` to `evaluations/dogfood-cold-resume.md` (Implementation history records the executable consequences and follow-up evidence for this concept.).
* **Update**: Updated concept `implementation/implementation-notes.md`.
* **Update**: Linked `implementation/implementation-notes.md` to `design/s7-bounded-perception.md` (Implementation history records the executable consequences and follow-up evidence for this concept.).
* **Update**: Updated concept `implementation/implementation-notes.md`.
* **Update**: Linked `implementation/implementation-notes.md` to `design/semantic-write-api.md` (Implementation history records the executable consequences and follow-up evidence for this concept.).
* **Update**: Updated concept `implementation/implementation-notes.md`.
* **Update**: Linked `implementation/implementation-notes.md` to `decisions/mcp-centered-tool-surface.md` (Implementation history records the executable consequences and follow-up evidence for this concept.).
* **Update**: Updated concept `implementation/implementation-notes.md`.
* **Update**: Linked `implementation/implementation-notes.md` to `decisions/external-workspace-and-clearhead-boundary.md` (Implementation history records the executable consequences and follow-up evidence for this concept.).
* **Update**: Updated concept `implementation/implementation-notes.md`.

* **Creation**: Documented concept `implementation/implementation-notes.md` (Implementation notes).
* **Creation**: Documented concept `research/structural-freshness-without-formatter-coupling.md` (Structural freshness without formatter coupling).
* **Creation**: Documented concept `evaluations/review-s6-implementation.md` (S6 implementation review).
* **Creation**: Documented concept `evaluations/review-s1-implementation.md` (S1 implementation review).
* **Creation**: Documented concept `evaluations/response-to-executable-contract.md` (Response to the revised perspective and executable contract).
* **Creation**: Documented concept `evaluations/response-to-agent-perspective.md` (Response to the agent's perspective).
* **Creation**: Documented concept `evaluations/dogfood-cold-resume.md` (Dogfooding the workspace on a cold resume).
* **Creation**: Documented concept `evaluations/plot-foreign-dogfood.md` (First foreign dogfood of the semantic write API).
* **Creation**: Documented concept `specifications/executable-contract.md` (Executable contract for the Agent Workspace MVP).
* **Creation**: Documented concept `design/initial-design.md` (Initial design outline).
* **Creation**: Documented concept `design/semantic-write-api.md` (Semantic write API).
* **Creation**: Documented concept `design/s7-bounded-perception.md` (S7 bounded perception).
* **Creation**: Documented concept `design/configurable-normalizers.md` (Configurable normalizers).
* **Creation**: Documented concept `design/agent-perspective.md` (The agent's perspective).
* **Creation**: Documented concept `decisions/mcp-centered-tool-surface.md` (Center the tool surface on MCP).
* **Creation**: Documented concept `decisions/external-workspace-and-clearhead-boundary.md` (External workspace state and the Clearhead boundary).

* **Creation**: Initialized OKF v0.2 project memory.
