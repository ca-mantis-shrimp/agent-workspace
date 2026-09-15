#!/usr/bin/env python3
"""End-to-end acceptance drive for the SessionStart orientation hook.

Invokes `orient-session.py` exactly as Claude Code does — a JSON event on stdin —
and asserts its two commitments and its happy path against the *real* built
kernel binary. Run from the repository root:  python3 this_file.py

The kernel's own Rust suite owns wake summary semantics; this drive owns the
adapter boundary: never harm (always exit 0), stay quiet when there is nothing
to say, and forward the kernel's summary verbatim when there is.
"""

import json
import os
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
HOOK = os.path.join(HERE, "orient-session.py")
REPO = os.path.dirname(os.path.dirname(HERE))
BINARY = os.path.join(REPO, "target", "debug", "agent-workspace")


def drive(payload, stdin_override=None):
    stdin = json.dumps(payload) if stdin_override is None else stdin_override
    # Pin the binary under test: the hook otherwise prefers `agent-workspace` on
    # PATH, which may be an older install than the build this drive verifies.
    proc = subprocess.run(
        ["python3", HOOK],
        input=stdin,
        capture_output=True,
        text=True,
        timeout=20,
        env={**os.environ, "AGENT_WORKSPACE_BIN": BINARY},
    )
    return proc.returncode, proc.stdout


def expect(name, condition):
    mark = "ok  " if condition else "FAIL"
    print(f"  [{mark}] {name}")
    return condition


def make_dir(path):
    try:
        os.makedirs(path)
        return True
    except Exception:
        return False


def main() -> int:
    if not os.path.exists(BINARY):
        print(f"build the kernel first: {BINARY} is missing", file=sys.stderr)
        return 2

    passed = True

    # Happy path: inside this repo, orientation is the kernel's wake summary,
    # byte for byte, and never fails the session.
    code, out = drive(
        {"hook_event_name": "SessionStart", "source": "startup", "cwd": REPO}
    )
    passed &= expect("happy: exit 0", code == 0)
    # Recompute the reference with the SAME resolution the hook uses (no
    # `--workspace`): the kernel locates the project-scoped state root from
    # `--repository` alone, so both this reference and the hook read the same
    # place.
    kernel_summary = subprocess.run(
        [BINARY, "status", "--summary", "--repository", REPO],
        capture_output=True,
        text=True,
        check=True,
        timeout=20,
    ).stdout
    passed &= expect(
        "happy: forwards the kernel wake summary verbatim", out == kernel_summary
    )
    passed &= expect("happy: wake header present", out.startswith("wake · "))
    passed &= expect("happy: wake fits 1000 bytes", len(out.encode()) <= 1_000)

    # No Git checkout: quiet and harmless.
    with tempfile.TemporaryDirectory() as plain:
        code, out = drive(
            {"hook_event_name": "SessionStart", "source": "startup", "cwd": plain}
        )
        passed &= expect("no-git: exit 0", code == 0)
        passed &= expect("no-git: emits nothing", out == "")

    # Git checkout with a built binary but an empty workspace: the kernel
    # renders nothing, so the hook emits nothing.
    with tempfile.TemporaryDirectory() as fresh:
        subprocess.run(["git", "-C", fresh, "init", "-q"], check=True)
        passed &= expect(
            "empty-workspace: fixture directory created",
            make_dir(os.path.join(fresh, "target", "debug")),
        )
        os.symlink(BINARY, os.path.join(fresh, "target", "debug", "agent-workspace"))
        code, out = drive(
            {"hook_event_name": "SessionStart", "source": "startup", "cwd": fresh}
        )
        passed &= expect("empty-workspace: exit 0", code == 0)
        passed &= expect("empty-workspace: emits nothing", out == "")

    # Malformed stdin: swallowed, exit 0, nothing emitted.
    code, out = drive(None, stdin_override="this is not json")
    passed &= expect("malformed-stdin: exit 0", code == 0)
    passed &= expect("malformed-stdin: emits nothing", out == "")

    print("PASS" if passed else "FAILURES ABOVE")
    return 0 if passed else 1


if __name__ == "__main__":
    sys.exit(main())
