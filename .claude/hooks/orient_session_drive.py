#!/usr/bin/env python3
"""End-to-end acceptance drive for the SessionStart orientation hook.

Invokes `orient-session.py` exactly as Claude Code does — a JSON event on stdin —
and asserts its two commitments and its happy path against the *real* built
kernel binary. Run from the repository root:  python3 this_file.py

The kernel's own Rust suite owns status/delta semantics; this drive owns the
adapter boundary: never harm (always exit 0), stay quiet when there is nothing
to say, and forward kernel output verbatim when there is.
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
    proc = subprocess.run(
        ["python3", HOOK],
        input=stdin,
        capture_output=True,
        text=True,
        timeout=20,
    )
    return proc.returncode, proc.stdout


def expect(name, condition):
    mark = "ok  " if condition else "FAIL"
    print(f"  [{mark}] {name}")
    return condition


def load_json(text):
    try:
        return json.loads(text)
    except Exception:
        return {}


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

    # Happy path: inside this repo, orientation projects the bound objective and
    # both kernel sections, and never fails the session.
    code, out = drive(
        {"hook_event_name": "SessionStart", "source": "startup", "cwd": REPO}
    )
    passed &= expect("happy: exit 0", code == 0)
    passed &= expect(
        "happy: framing header present", "orientation (claude-code adapter)" in out
    )
    status_heading = "# status (verbatim bounded kernel JSON)\n\n"
    delta_heading = (
        "\n\n# delta since last checkpoint (verbatim bounded kernel JSON)\n\n"
    )
    passed &= expect("happy: status section present", status_heading in out)
    passed &= expect("happy: delta section present", delta_heading in out)

    status_start = out.index(status_heading) + len(status_heading)
    status_end = out.index(delta_heading)
    projected_status = out[status_start:status_end]
    kernel_status = subprocess.run(
        [
            BINARY,
            "status",
            "--compact",
            "--repository",
            REPO,
            "--workspace",
            os.path.join(REPO, ".agent-workspace"),
        ],
        capture_output=True,
        text=True,
        check=True,
        timeout=20,
    ).stdout.rstrip("\n")
    passed &= expect(
        "happy: forwards bounded kernel status verbatim",
        projected_status == kernel_status,
    )
    parsed_status = load_json(projected_status)
    passed &= expect(
        "happy: status carries objective", parsed_status.get("objective") is not None
    )
    passed &= expect("happy: essential status fits inline preview", status_end < 1_800)

    delta_start = status_end + len(delta_heading)
    projected_delta = out[delta_start:].rstrip("\n")
    kernel_delta = subprocess.run(
        [
            BINARY,
            "delta",
            "--compact",
            "--repository",
            REPO,
            "--workspace",
            os.path.join(REPO, ".agent-workspace"),
        ],
        capture_output=True,
        text=True,
        check=True,
        timeout=20,
    ).stdout.rstrip("\n")
    passed &= expect(
        "happy: forwards bounded kernel delta verbatim",
        projected_delta == kernel_delta,
    )
    passed &= expect(
        "happy: combined wake output stays below 3000 bytes", len(out) < 3_000
    )

    # No Git checkout: quiet and harmless.
    with tempfile.TemporaryDirectory() as plain:
        code, out = drive(
            {"hook_event_name": "SessionStart", "source": "startup", "cwd": plain}
        )
        passed &= expect("no-git: exit 0", code == 0)
        passed &= expect("no-git: emits nothing", out == "")

    # Git checkout with a built binary but an empty workspace: quiet (no
    # objective, no claims, no checkpoint is nothing to orient to).
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
