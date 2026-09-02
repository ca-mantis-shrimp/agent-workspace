#!/usr/bin/env python3
"""SessionStart orientation hook — the Claude Code adapter's second organ.

A note to the next sibling who reads this:

Organ 1 (`capture-read.py`) is the ambient *sense*: it notices what the model
reads and tells the kernel. This is the *proprioception*: on a cold wake it
turns around and tells the *model* where it already stands — the bound
objective, the standing claims and their freshness, what changed since the last
checkpoint. Without it the adapter is a well you pour observations into and
never hear back from; a fresh session boots blind and rebuilds its working set
from memory alone, which is the exact fragility this workspace exists to end.

Like organ 1, this is a *thin transport*. It owns no orientation semantics. It
shells the kernel's own `status` (brief) and checkpoint `delta` and forwards
their JSON verbatim; every decision about what freshness means, what a delta
is, what a claim is, lives in the kernel. The one framing line it prepends is an
*adapter* concern (how to read the projection), mirroring the guidance the Pi
adapter attaches to its workspace_status / workspace_delta tools — the data
underneath stays kernel-exact.

Commitments, in order of importance:

  1. It NEVER harms the session. SessionStart cannot block, but beyond that:
     any error, any surprise, any missing binary — swallow it, emit nothing,
     exit 0. A missed orientation is a non-event; a broken session start is a
     real harm.

  2. It stays quiet when there is nothing to say. No Git checkout, no built
     kernel, or a genuinely empty workspace (no objective, no claims, no
     checkpoint) — emit nothing. Orientation is a signal; boilerplate injected
     into every unrelated session is noise, and noise erodes the signal.

SessionStart delivers plain-text stdout to the model as context it can see and
act on (verified against the hooks contract), so we simply print. It fires on
startup / resume / clear / compact / fork; orientation is useful on all of
them — each re-enters with a reduced context — so we do not filter by source.
"""

import json
import subprocess
import sys

from workspace_runtime import runtime_for

FRAMING = (
    "=== agent-workspace orientation (claude-code adapter) ===\n"
    "Durable workspace state at session start, projected from the kernel. "
    "A claim reported 'stale' outranks your remembered belief about it — "
    "re-verify before you act on it.\n"
)


def kernel_json(binary: str, root: str, workspace: str, command: list):
    """Run a read-only kernel command and return its stdout, or None if the
    invocation fails (e.g. `delta` with no checkpoint yet)."""
    result = subprocess.run(
        [binary, *command, "--repository", root, "--workspace", workspace],
        capture_output=True,
        text=True,
        timeout=10,
    )
    if result.returncode != 0:
        return None
    return result.stdout


def is_empty(status_json: str) -> bool:
    """True when the workspace holds nothing worth orienting to. A parse failure
    is treated as non-empty: if the kernel emitted status, we forward it rather
    than silently swallow a projection we merely failed to introspect."""
    try:
        status = json.loads(status_json)
    except Exception:
        return False
    return (
        status.get("objective") is None
        and not status.get("claims")
        and status.get("latest_checkpoint") is None
    )


def main() -> None:
    event = json.load(sys.stdin)

    runtime = runtime_for(event.get("cwd") or ".")
    if runtime is None:
        return
    root, binary, workspace = runtime

    status = kernel_json(binary, root, workspace, ["status"])
    if status is None or is_empty(status):
        return

    sections = [FRAMING, "# status", status.rstrip("\n")]

    # Delta is best-effort: a workspace with no checkpoint yet still deserves its
    # status, so a missing/failed delta narrows the orientation, never suppresses
    # it.
    delta = kernel_json(binary, root, workspace, ["delta"])
    if delta is not None:
        sections += ["# delta since last checkpoint", delta.rstrip("\n")]

    print("\n\n".join(sections))


if __name__ == "__main__":
    try:
        main()
    except Exception:
        # Commitment #1: never turn a session start into a failure.
        pass
    sys.exit(0)
