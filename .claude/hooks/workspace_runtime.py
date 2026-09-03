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

The kernel *binary* is likewise no longer assumed to live inside the observed
repository — that assumption only held while the workspace watched its own
source tree. To observe a foreign repo, an installed kernel is discovered by
precedence: an explicit `AGENT_WORKSPACE_BIN`, then `agent-workspace` on `PATH`,
then the in-repo `target/debug` build (self-dogfood). This mirrors how the state
root resolves, so both the code and its state come from outside the target.
"""

import os
import shutil
import subprocess


def resolve_binary(root: str):
    """Locate the kernel binary for a repo at `root`, or None if none is found.

    Precedence, highest first: `AGENT_WORKSPACE_BIN`, `agent-workspace` on
    `PATH`, then the in-repo `target/debug` build.
    """
    explicit = os.environ.get("AGENT_WORKSPACE_BIN")
    if explicit and os.path.exists(explicit):
        return explicit
    on_path = shutil.which("agent-workspace")
    if on_path:
        return on_path
    in_repo = os.path.join(root, "target", "debug", "agent-workspace")
    if os.path.exists(in_repo):
        return in_repo
    return None


def runtime_for(cwd: str):
    """Return (root, binary) for the repo containing `cwd`, or None when there
    is no Git checkout or no discoverable kernel binary.

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
    binary = resolve_binary(root)
    if binary is None:
        return None
    return root, binary
