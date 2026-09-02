"""Shared runtime resolution for the Claude Code adapter hooks.

Both adapter organs — the PostToolUse(Read) ambient sense and the SessionStart
orientation projection — need the same answer: given a working directory, where
is the repository root, the built kernel binary, and the workspace state
directory? Resolving that in one place keeps the two hooks from drifting apart,
which is precisely the failure mode this whole workspace exists to fight.

The `--workspace` argument the kernel expects is the *state directory path*
(`.agent-workspace`), not a workspace name; both adapters pass it that way.
"""

import os
import subprocess


def runtime_for(cwd: str):
    """Return (root, binary, workspace_dir) for the repo containing `cwd`, or
    None when there is no Git checkout or no built kernel binary here.

    A missing binary resolves to None rather than an error: an adapter with no
    kernel to talk to has nothing to do, and that is a non-event, never a harm.
    """
    try:
        top = subprocess.run(
            ["git", "-C", cwd, "rev-parse", "--show-toplevel"],
            capture_output=True,
            text=True,
            timeout=5,
        )
    except Exception:
        return None
    if top.returncode != 0:
        return None
    root = top.stdout.strip()
    binary = os.path.join(root, "target", "debug", "agent-workspace")
    if not os.path.exists(binary):
        return None
    return root, binary, os.path.join(root, ".agent-workspace")
