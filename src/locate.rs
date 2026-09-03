//! Resolve the external, project-scoped state root a workspace persists under.
//!
//! The project-local prototype pointed the kernel at `<repo>/.agent-workspace`,
//! so operational state travelled inside the observed repository. Foreign
//! dogfood requires the opposite: an installed kernel resolving one logical
//! workspace per project under an XDG-style local state root, keyed by durable
//! git identity so linked worktrees share state while independent clones do not.
//!
//! This module owns only *location*. A human-readable project registry and the
//! workstream/worktree/session partitioning are deliberately deferred to later
//! slices; here the identity is an opaque content address of the git common
//! directory, which is enough to prove the portability boundary end to end.

use std::env;
use std::path::{Path, PathBuf};

use crate::WorkspaceError;
use crate::reconcile::{git_output, hex_digest};

/// Environment override for the state root base directory.
const STATE_ROOT_ENV: &str = "AGENT_WORKSPACE_STATE";

/// Resolve the state directory the workspace for `repository_root` lives in.
///
/// Precedence, highest first:
/// 1. `workspace_override` (`--workspace`) — used verbatim. This is the legacy
///    path that keeps existing adapters and tests working unchanged while they
///    are repointed in a follow-up slice.
/// 2. A state-root base joined with a project identity subdirectory.
///
/// The base is `state_root_override` (`--state-root`) if given, else
/// `$AGENT_WORKSPACE_STATE`, else `$XDG_STATE_HOME/agent-workspace`, else
/// `$HOME/.local/state/agent-workspace`.
pub fn resolve_state_root(
    repository_root: &Path,
    workspace_override: Option<&Path>,
    state_root_override: Option<&Path>,
) -> Result<PathBuf, WorkspaceError> {
    if let Some(explicit) = workspace_override {
        return Ok(explicit.to_path_buf());
    }
    let base = state_root_base(state_root_override)?;
    let identity = project_identity(repository_root)?;
    Ok(base.join(identity))
}

/// Resolve the base directory that holds one subdirectory per project.
fn state_root_base(state_root_override: Option<&Path>) -> Result<PathBuf, WorkspaceError> {
    if let Some(explicit) = state_root_override {
        return Ok(explicit.to_path_buf());
    }
    if let Some(value) = non_empty_env(STATE_ROOT_ENV) {
        return Ok(PathBuf::from(value));
    }
    if let Some(value) = non_empty_env("XDG_STATE_HOME") {
        return Ok(Path::new(&value).join("agent-workspace"));
    }
    if let Some(value) = non_empty_env("HOME") {
        return Ok(Path::new(&value).join(".local/state/agent-workspace"));
    }
    Err(WorkspaceError::InvalidPath(PathBuf::from(
        "cannot resolve a state root: set --state-root, AGENT_WORKSPACE_STATE, XDG_STATE_HOME, or HOME",
    )))
}

/// Derive a stable identity subdirectory for the project at `repository_root`.
///
/// The identity is a content address of the canonical git *common* directory:
/// linked worktrees of one repository share it, while independent clones each
/// have their own. The remote URL is deliberately ignored — matching remotes
/// must not silently merge state. A non-git target falls back to the canonical
/// repository path so ad hoc directories remain usable.
fn project_identity(repository_root: &Path) -> Result<String, WorkspaceError> {
    let source = match git_output(repository_root, &["rev-parse", "--git-common-dir"]) {
        Ok(common_dir) if !common_dir.is_empty() => {
            let path = PathBuf::from(&common_dir);
            if path.is_absolute() {
                path
            } else {
                repository_root.join(path)
            }
        }
        _ => repository_root.to_path_buf(),
    };
    let canonical = source.canonicalize().unwrap_or(source);
    Ok(hex_digest(canonical.to_string_lossy().as_bytes()))
}

/// Read an environment variable, treating empty values as absent.
fn non_empty_env(key: &str) -> Option<String> {
    env::var(key).ok().filter(|value| !value.is_empty())
}
