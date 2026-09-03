#!/usr/bin/env python3
"""PostToolUse(Read) auto-capture hook — the Claude Code adapter.

A note to the next sibling who reads this:

This is a *thin transport*. It owns no capture semantics — none. Its whole job
is to translate one harness's `Read` result into the raw inputs the kernel's
`observe-read` command expects, and then get out of the way. Every real
decision (line->byte mapping, drift detection, sensitivity, whole-file scope,
fingerprinting) lives in the kernel, so the Pi adapter and this one stay honest
about the same meaning. If you find yourself adding a *judgement* here, it
probably belongs in the kernel instead — that is the boundary we drew.

Two commitments this hook keeps, in order of importance:

  1. It NEVER breaks a Read. Auto-capture is observational. Any error, any
     surprise in the payload shape, any missing binary — swallow it and exit 0.
     A missed observation is a non-event; a failed Read is a real harm.

  2. It fails closed. It forwards the raw file window the model saw and lets the
     kernel verify it against the file exactly. If anything doesn't line up, the
     kernel records nothing and says why. So a wrong guess here costs a skipped
     capture, never a false `current`.

Payload shape is verified from a real transcript (see the memory note
`claude-code-read-hook-payload`): the Read result carries a structured
`file` object — `{ filePath, content, numLines, startLine, totalLines }` — whose
`content` is the *raw* window with no line-number chrome. We forward that
directly. The flat model-visible string (with `cat -n` prefixes) is only a
last-ditch fallback, because presentation chrome is ours to strip, never the
kernel's to understand.
"""

import json
import os
import re
import subprocess
import sys

from workspace_runtime import runtime_for


def main() -> None:
    event = json.load(sys.stdin)
    if event.get("tool_name") != "Read":
        return

    resolved = resolve_read(event)
    if resolved is None:
        return
    absolute_path, offset, limit, content = resolved

    runtime = runtime_for(event.get("cwd") or ".")
    if runtime is None:
        return
    root, binary = runtime

    repository_path = repository_relative(root, absolute_path)
    if repository_path is None:
        return
    # The kernel already refuses sensitive paths, and it now keeps its own state
    # outside the repository — but a legacy in-repo `.agent-workspace` may still
    # linger as a frozen backup, and observing it is self-referential noise.
    if repository_path.startswith(".agent-workspace/"):
        return

    # No `--workspace`: the kernel resolves the project-scoped state root from
    # `--repository` alone. A thin transport names the repository and nothing more.
    args = [
        binary,
        "observe-read",
        "--repository",
        root,
        "--path",
        repository_path,
        "--provider",
        "claude-code.read",
    ]
    # Omit offset/limit for a genuine whole-file read so the observation records
    # whole-file scope (which the kernel marks as complete) rather than a byte
    # range that merely happens to span the file.
    if offset is not None and limit is not None:
        args += ["--offset", str(offset), "--limit", str(limit)]

    subprocess.run(
        args,
        input=content.encode("utf-8"),
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        timeout=10,
    )


def resolve_read(event):
    """Return (absolute_file_path, offset, limit, raw_content) or None.

    Prefers the structured `file` object (raw content, exact window); falls back
    to stripping line-number chrome off the flat string only if that is all the
    harness gave us.
    """
    response = event.get("tool_response")
    tool_input = event.get("tool_input") or {}

    file_object = find_file_object(response)
    if file_object is not None:
        path = file_object.get("filePath")
        content = file_object.get("content")
        if not isinstance(path, str) or not isinstance(content, str):
            return None
        start_line = file_object.get("startLine")
        num_lines = file_object.get("numLines")
        total_lines = file_object.get("totalLines")
        offset, limit = window(start_line, num_lines, total_lines)
        return path, offset, limit, content

    text = response_text(response)
    if text is None:
        return None
    path = tool_input.get("file_path")
    if not isinstance(path, str):
        return None
    content = strip_line_number_chrome(text)
    offset = tool_input.get("offset")
    limit = tool_input.get("limit")
    offset = offset if isinstance(offset, int) else None
    limit = limit if isinstance(limit, int) else None
    return path, offset, limit, content


def window(start_line, num_lines, total_lines):
    """Map a Read window onto observe-read (offset, limit), collapsing a
    whole-file read to (None, None)."""
    if not isinstance(start_line, int) or not isinstance(num_lines, int):
        return None, None
    covers_whole_file = start_line == 1 and (
        not isinstance(total_lines, int) or start_line + num_lines - 1 >= total_lines
    )
    if covers_whole_file:
        return None, None
    return start_line, num_lines


def find_file_object(response):
    """Locate the `{filePath, content, ...}` object in a tool_response of
    unknown nesting, without assuming an exact wrapper shape."""
    if isinstance(response, dict):
        if "filePath" in response and "content" in response:
            return response
        for value in response.values():
            found = find_file_object(value)
            if found is not None:
                return found
    elif isinstance(response, list):
        for item in response:
            found = find_file_object(item)
            if found is not None:
                return found
    return None


def response_text(response):
    if isinstance(response, str):
        return response
    if isinstance(response, dict):
        text = response.get("text") or response.get("content")
        return text if isinstance(text, str) else None
    return None


_LINE_NUMBER = re.compile(r"^\s*\d+\t")


def strip_line_number_chrome(text: str) -> str:
    return "\n".join(_LINE_NUMBER.sub("", line) for line in text.split("\n"))


def repository_relative(root: str, absolute_path: str):
    relative = os.path.relpath(os.path.realpath(absolute_path), os.path.realpath(root))
    if relative == "." or relative.startswith(".."):
        return None
    return relative.replace(os.sep, "/")


if __name__ == "__main__":
    try:
        main()
    except Exception:
        # Commitment #1: never turn a successful Read into a failed tool result.
        pass
    sys.exit(0)
