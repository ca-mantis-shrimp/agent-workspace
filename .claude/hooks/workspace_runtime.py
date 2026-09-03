"""Shared runtime resolution for the Claude Code adapter hooks.

Both adapter organs — the PostToolUse(Read) ambient sense and the SessionStart
orientation projection — need the same answer: given a working directory, where
is the repository root and the built kernel binary? Resolving that in one place
keeps the two hooks from drifting apart, which is precisely the failure mode
this whole workspace exists to fight.

Note what is *no longer* here: the workspace state directory. The adapter used
to hand the kernel `--workspace <repo>/.agent-workspace`, pinning operational
state inside the observed repository. That is exactly the coupling foreign
dogfood removes. Resolution is now the kernel's job — given only `--repository`,
it locates one project-scoped workspace under an external state root keyed by
git identity (see `src/locate.rs`). A thin transport must not second-guess it,
so we pass only the repository and let the kernel decide where state lives.
"""

import os
import subprocess


def runtime_for(cwd: str):
    """Return (root, binary) for the repo containing `cwd`, or None when there
    is no Git checkout or no built kernel binary here.

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
    return root, binary
