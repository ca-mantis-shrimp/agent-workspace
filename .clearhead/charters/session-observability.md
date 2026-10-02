---
id: 01a0cce3-ae0a-73e1-ba39-3b00db9d02bd
alias: session-observability
parent: agent-workspace
state: New
---
# Agent session observability

Make agent behaviour across proprietary harnesses (Claude Code, Pi, …)
observable by archiving and normalizing each harness's own session files, and
join that behaviour to workspace intent for offline analysis. The design and its
boundaries are recorded in
[the session-log observability decision](../../knowledge/decisions/session-log-observability.md);
this charter tracks the work, not the rationale.

## Outcome

The owner can answer "what did the agents do overnight, what did it cost, where
did they get stuck" from one local archive, and can compare what agents *did*
(session logs) with what they *asserted* (workspace claims) on a shared session
identity.

## Out of scope

- Transcript ingestion into the kernel or the wake projection.
- Always-on observability services before DuckDB proves insufficient.
- Autonomous self-modification from the analysis; changes stay owner-approved.
