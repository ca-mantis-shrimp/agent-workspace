//! Which normalizer a fresh capture uses for a given path — as declared
//! configuration rather than a hard-coded `match`.
//!
//! The kernel canonicalizes source before fingerprinting so a semantics-
//! preserving reformat does not stale a claim (see [`crate::Normalizer`]). This
//! module owns the *selection* half: extension → registered normalizer. It is a
//! builtin default (`rs` → rustfmt, everything else raw bytes — the kernel's
//! original behavior) overlaid by an optional, committed repo file:
//!
//! ```toml
//! # .agent-workspace/normalizers.toml
//! [normalizers]
//! rs = { tool = "rustfmt" }
//! ts = { tool = "prettier" }   # opt-in: uses the repo's node_modules prettier
//! py = { tool = "ruff" }       # or { tool = "black" }
//! ```
//!
//! Every non-`rs` mapping is opt-in rather than a builtin default on purpose: a
//! repo may format TypeScript with biome/dprint/nothing, and Python with
//! black/ruff/nothing, so the kernel must not presume a canonicalizer. Declaring
//! one is a one-line, reviewable commitment that this repo's canonical form for
//! that extension is that tool's. Registered tools:
//! [`Normalizer::from_tool_name`] (`rustfmt`, `prettier`, `black`, `ruff`).
//!
//! Determinism caveat, sharpest for Python: black and ruff shift their *stable
//! style* across versions, so a claim's canonical form is only comparable across
//! environments running the same formatter version. Preferring a project-venv
//! binary (`.venv/bin`) mitigates this; the `version` field is the reserved slot
//! to enforce it.
//!
//! Selection is read at capture time and the *resolved* scheme is persisted on
//! the observation, so reconcile always compares like with like and config
//! evolution is forward-only. Config names a tool only from the registry
//! ([`Normalizer::from_tool_name`]) — never an arbitrary command — because a
//! normalizer is only sound if it is deterministic across environments. An
//! unknown tool or a malformed file fails closed with a named error rather than
//! silently falling through to raw-byte comparison (which would fabricate a
//! freshness verdict).

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use crate::{Normalizer, WorkspaceError};

/// Repo-relative location of the optional config file. Reserved dir: adapters
/// do not capture reads under `.agent-workspace/`, and it is not state (state
/// lives in an external XDG root), so a config file here travels with the clone
/// and is reviewable in a pull request.
const CONFIG_RELATIVE_PATH: &str = ".agent-workspace/normalizers.toml";

/// Resolve the normalizer a fresh capture of `path` should use under this
/// repository's configuration. Replaces the former hard-coded
/// `Normalizer::detect_for_path`.
pub(crate) fn resolve_for_path(
    repository_root: &Path,
    path: &Path,
) -> Result<Normalizer, WorkspaceError> {
    Ok(NormalizerConfig::load(repository_root)?.for_path(path))
}

/// Lowercased extension (no dot) → the normalizer captures of that file type use.
struct NormalizerConfig {
    by_extension: BTreeMap<String, Normalizer>,
}

impl NormalizerConfig {
    /// The mapping used when no config file overrides it — byte-for-byte the
    /// kernel's original hard-coded behavior: Rust via rustfmt, all else raw.
    fn builtin_default() -> Self {
        let mut by_extension = BTreeMap::new();
        by_extension.insert("rs".to_owned(), Normalizer::Rustfmt);
        Self { by_extension }
    }

    /// Load `.agent-workspace/normalizers.toml` (when present) overlaid on the
    /// builtin default. Absent file → builtin (unchanged behavior). Malformed
    /// file or an unknown tool name → fail closed with a named error.
    fn load(repository_root: &Path) -> Result<Self, WorkspaceError> {
        let mut config = Self::builtin_default();
        let path = repository_root.join(CONFIG_RELATIVE_PATH);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(config),
            Err(error) => return Err(WorkspaceError::Io(error)),
        };
        let file: NormalizerConfigFile = toml::from_str(&text).map_err(|error| {
            WorkspaceError::InvalidConfig(format!("{CONFIG_RELATIVE_PATH}: {error}"))
        })?;
        for (extension, entry) in file.normalizers {
            let normalizer = Normalizer::from_tool_name(&entry.tool).ok_or_else(|| {
                WorkspaceError::InvalidConfig(format!(
                    "{CONFIG_RELATIVE_PATH}: extension `{extension}` names unknown normalizer \
                     tool `{}`",
                    entry.tool
                ))
            })?;
            config
                .by_extension
                .insert(extension.to_ascii_lowercase(), normalizer);
        }
        Ok(config)
    }

    fn for_path(&self, path: &Path) -> Normalizer {
        path.extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| extension.to_ascii_lowercase())
            .and_then(|extension| self.by_extension.get(&extension).copied())
            .unwrap_or(Normalizer::None)
    }
}

#[derive(Deserialize)]
struct NormalizerConfigFile {
    #[serde(default)]
    normalizers: BTreeMap<String, NormalizerEntry>,
}

#[derive(Deserialize)]
struct NormalizerEntry {
    tool: String,
    /// Pinned formatter version. Reserved for the version-pinning slice; parsed
    /// so authors can declare it today, but not yet enforced.
    #[serde(default)]
    #[allow(dead_code)]
    version: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn write_config(root: &Path, body: &str) {
        let dir = root.join(".agent-workspace");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("normalizers.toml"), body).unwrap();
    }

    #[test]
    fn builtin_default_matches_the_former_hard_coded_match() {
        let root = tempfile::tempdir().unwrap();
        // No config file: rust → rustfmt, everything else → raw bytes.
        assert_eq!(
            resolve_for_path(root.path(), &PathBuf::from("src/task.rs")).unwrap(),
            Normalizer::Rustfmt
        );
        assert_eq!(
            resolve_for_path(root.path(), &PathBuf::from("README.md")).unwrap(),
            Normalizer::None
        );
        assert_eq!(
            resolve_for_path(root.path(), &PathBuf::from("Makefile")).unwrap(),
            Normalizer::None
        );
    }

    #[test]
    fn config_overlays_the_builtin_and_can_disable_normalization() {
        let root = tempfile::tempdir().unwrap();
        write_config(root.path(), "[normalizers]\nrs = { tool = \"none\" }\n");
        // The overlay wins: the seam actually changes behavior, not just shape.
        assert_eq!(
            resolve_for_path(root.path(), &PathBuf::from("src/task.rs")).unwrap(),
            Normalizer::None
        );
    }

    #[test]
    fn prettier_is_registered_and_selectable_for_typescript() {
        let root = tempfile::tempdir().unwrap();
        write_config(root.path(), "[normalizers]\nts = { tool = \"prettier\" }\n");
        assert_eq!(
            resolve_for_path(root.path(), &PathBuf::from("src/index.ts")).unwrap(),
            Normalizer::Prettier
        );
        // Still opt-in: a TS file without the config stays raw bytes.
        assert_eq!(
            resolve_for_path(tempfile::tempdir().unwrap().path(), &PathBuf::from("a.ts")).unwrap(),
            Normalizer::None
        );
    }

    #[test]
    fn python_formatters_are_registered_and_selectable() {
        let root = tempfile::tempdir().unwrap();
        write_config(root.path(), "[normalizers]\npy = { tool = \"ruff\" }\n");
        assert_eq!(
            resolve_for_path(root.path(), &PathBuf::from("hooks/orient.py")).unwrap(),
            Normalizer::RuffFormat
        );
        // `black` is the other registered Python tool.
        assert_eq!(Normalizer::from_tool_name("black"), Some(Normalizer::Black));
        // Still opt-in: a .py file with no config stays raw bytes.
        assert_eq!(
            resolve_for_path(tempfile::tempdir().unwrap().path(), &PathBuf::from("a.py")).unwrap(),
            Normalizer::None
        );
    }

    #[test]
    fn an_unknown_tool_fails_closed() {
        let root = tempfile::tempdir().unwrap();
        // `biome` is a real formatter but not a registered normalizer; naming it
        // must fail closed rather than silently degrade to raw-byte comparison.
        write_config(root.path(), "[normalizers]\nts = { tool = \"biome\" }\n");
        let error = resolve_for_path(root.path(), &PathBuf::from("index.ts")).unwrap_err();
        assert!(
            matches!(error, WorkspaceError::InvalidConfig(_)),
            "an unregistered tool must fail closed, not fall back to raw bytes: {error:?}"
        );
    }

    #[test]
    fn a_malformed_config_fails_closed() {
        let root = tempfile::tempdir().unwrap();
        write_config(root.path(), "this is not = valid = toml");
        let error = resolve_for_path(root.path(), &PathBuf::from("src/task.rs")).unwrap_err();
        assert!(matches!(error, WorkspaceError::InvalidConfig(_)));
    }
}
