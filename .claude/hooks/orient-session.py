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
is, what a claim is, lives in the kernel. Claude bounds the inline preview of a
command hook's stdout, so the adapter first emits a compact *preview index* of
kernel-reported fields (objective, checkpoint, claim freshness/scope, bounded
headlines). That index computes no verdict; it is transport framing that keeps
the essential orientation ahead of Claude's preview boundary. The complete
kernel projections follow unchanged and remain revealable from Claude's saved
hook output.

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

import contextlib
import json
import subprocess
import sys

from workspace_runtime import runtime_for

FRAMING = (
    "=== agent-workspace orientation (claude-code adapter) ===\n"
    "Kernel projection. 'stale' outranks memory: re-verify before acting. "
    "The preview index stays inline; full status/delta follow and may be saved "
    "by Claude when its hook-output preview is exceeded.\n"
)
PREVIEW_HEADLINE_CHARS = 48


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


def parse_status(status_json: str):
    """Parse status only for adapter framing. A parse failure returns None: the
    kernel payload is still forwarded verbatim rather than reinterpreted."""
    try:
        return json.loads(status_json)
    except Exception:
        return None


def is_empty(status) -> bool:
    """True when the workspace holds nothing worth orienting to."""
    return status is not None and (
        status.get("objective") is None
        and not status.get("claims")
        and status.get("latest_checkpoint") is None
    )


def preview_index(status) -> str:
    """A compact table of contents for Claude's bounded hook-output preview.

    Values and verdicts come straight from kernel brief status. The adapter only
    selects, orders, and transport-truncates already-bounded headlines.
    """
    if status is None:
        return "status unavailable in preview index; see verbatim payload below"

    claims = []
    for claim in status.get("claims", []):
        headline = claim.get("headline", "")
        if len(headline) > PREVIEW_HEADLINE_CHARS:
            headline = headline[: PREVIEW_HEADLINE_CHARS - 1].rstrip() + "…"
        claims.append(
            {
                "id": claim.get("id"),
                "freshness": claim.get("freshness"),
                "scope": claim.get("scope"),
                "headline": headline,
            }
        )

    counts = status.get("counts") or {}
    index = {
        "objective": status.get("objective"),
        "latest_checkpoint": status.get("latest_checkpoint"),
        "claims": claims,
        "counts": {
            "active_claims": counts.get("active_claims"),
            "open_transactions": counts.get("open_transactions"),
        },
    }
    return json.dumps(index, ensure_ascii=False, separators=(",", ":"))


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
    root, binary, workspace = runtime

    status = kernel_json(binary, root, workspace, ["status"])
    if status is None:
        return
    parsed_status = parse_status(status)
    if is_empty(parsed_status):
        return

    sections = [
        FRAMING,
        "# preview index (transport summary of kernel status)",
        preview_index(parsed_status),
        "# status (verbatim kernel JSON)",
        status.rstrip("\n"),
    ]

    # Delta is best-effort: a workspace with no checkpoint yet still deserves its
    # status, so a missing/failed delta narrows the orientation, never suppresses
    # it.
    delta = kernel_json(binary, root, workspace, ["delta"])
    if delta is not None:
        sections += [
            "# delta since last checkpoint (verbatim kernel JSON)",
            delta.rstrip("\n"),
        ]

    print("\n\n".join(sections))


if __name__ == "__main__":
    # Commitment #1: never turn a session start into a failure.
    with contextlib.suppress(Exception):
        main()
    sys.exit(0)
