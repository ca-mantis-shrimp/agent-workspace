#!/usr/bin/env python3
"""SessionStart orientation hook — the Claude Code adapter's second organ.

A note to the next sibling who reads this:

Organ 1 (`capture-read.py`) is the ambient *sense*: it notices what the model
reads and tells the kernel. This is the *proprioception*: on a cold wake it
turns around and tells the *model* where it already stands — the goal, where
the last session stopped, what changed since that checkpoint, open work, and
which claims went stale. Without it the adapter is a well you pour
observations into and never hear back from; a fresh session boots blind and
rebuilds its working set from memory alone, which is the exact fragility this
workspace exists to end.

Like organ 1, this is a *thin transport*. It owns no orientation semantics. It
prints the kernel's wake summary (`status --summary`) verbatim: plain text of
at most 1000 bytes whose content, priority, budget, and quietness all live in
the kernel (see knowledge/specifications/wake-summary-contract.md). It adds no
framing, because every byte it added would be outside that budget.

Commitments, in order of importance:

  1. It NEVER harms the session. SessionStart cannot block, but beyond that:
     any error, any surprise, any missing binary — swallow it, emit nothing,
     exit 0. A missed orientation is a non-event; a broken session start is a
     real harm.

  2. It stays quiet when there is nothing to say. No Git checkout, no built
     kernel, or a workspace the kernel renders as empty — emit nothing.
     Orientation is a signal; boilerplate injected into every unrelated session
     is noise, and noise erodes the signal.

SessionStart delivers plain-text stdout to the model as context it can see and
act on (verified against the hooks contract), so we simply print. It fires on
startup / resume / clear / compact / fork; orientation is useful on all of
them — each re-enters with a reduced context — so we do not filter by source.
"""

import contextlib
import json
import subprocess
import sys

from workspace_runtime import runtime_for


def kernel_summary(binary: str, root: str):
    """The kernel's wake summary text, or None if the invocation fails.

    No `--workspace`: the kernel resolves the project-scoped state root from
    `--repository` alone, so orientation reads wherever state actually lives."""
    result = subprocess.run(
        [binary, "status", "--summary", "--repository", root],
        capture_output=True,
        text=True,
        timeout=10,
    )
    if result.returncode != 0:
        return None
    return result.stdout


def read_event():
    try:
        return json.load(sys.stdin)
    except Exception:
        return None


def main() -> None:
    event = read_event()
    if not isinstance(event, dict):
        return

    runtime = runtime_for(event.get("cwd") or ".")
    if runtime is None:
        return
    root, binary = runtime

    summary = kernel_summary(binary, root)
    if summary:
        sys.stdout.write(summary)


if __name__ == "__main__":
    # Commitment #1: never turn a session start into a failure.
    with contextlib.suppress(Exception):
        main()
    sys.exit(0)
