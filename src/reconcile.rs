//! Repository reconciliation primitives: pure functions that turn repository
//! state into fingerprints and freshness verdicts, plus the git/filesystem and
//! read-capture helpers that requires. Extracted verbatim from `lib.rs`; this is
//! a leaf layer — nothing here references `Workspace` or `Projection`.

use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use crate::*;

pub(crate) fn validate_relative_path(path: &Path) -> Result<PathBuf, WorkspaceError> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(WorkspaceError::InvalidPath(path.to_owned()));
    }
    Ok(path.to_owned())
}

pub(crate) fn fingerprint_file(path: &Path) -> Result<String, WorkspaceError> {
    let bytes = fs::read(path)?;
    Ok(hex_digest(&bytes))
}

pub(crate) fn conservative_sibling_dependencies(
    repository_root: &Path,
    supporting_paths: &[PathBuf],
) -> Result<Vec<PathBuf>, WorkspaceError> {
    let mut dependencies = BTreeSet::new();
    for supporting_path in supporting_paths {
        let parent = supporting_path.parent().unwrap_or_else(|| Path::new(""));
        let extension = supporting_path.extension();
        for entry in fs::read_dir(repository_root.join(parent))? {
            let entry = entry?;
            if !entry.file_type()?.is_file() || entry.path().extension() != extension {
                continue;
            }
            let path = parent.join(entry.file_name());
            if !supporting_paths.contains(&path) {
                dependencies.insert(path);
            }
        }
    }
    Ok(dependencies.into_iter().collect())
}

/// A reconciliation is a no-op when the recomputed verdict is identical to the
/// last persisted one. Suppression is the only sanctioned status optimization
/// (see the F9 guard): the verdict is always recomputed from current inputs;
/// only the redundant re-emission of an unchanged verdict is skipped. All other
/// report fields are static between reconciles — they are set by record events
/// and never touched by `*Reconciled` events — so an unchanged verdict means
/// the stored item already equals what re-projection would return.
pub(crate) fn verdict_unchanged(
    report: &FreshnessReport,
    freshness: &FreshnessWithinScope,
    reason: &str,
    reconciliation_fingerprint: &str,
) -> bool {
    report.freshness_within_scope == *freshness
        && report.reason == reason
        && report.operational_coverage.reconciliation_fingerprint == reconciliation_fingerprint
}

pub(crate) fn assess_claim_inputs(
    repository_root: &Path,
    inputs: &[ClaimInput],
) -> ClaimAssessment {
    let mut freshness = FreshnessWithinScope::Current;
    let mut reason = "recorded claim inputs unchanged".to_owned();
    let mut fingerprint_inputs = Vec::with_capacity(inputs.len());

    for input in inputs {
        let current = read_observation_fingerprints(
            repository_root,
            &input.path,
            &input.selector,
            input.normalizer,
            input.recorded_raw_fingerprint.as_deref(),
            &input.recorded_input_fingerprint,
        )
        .map(|(unit, _)| unit);
        match &current {
            Ok(fingerprint) if fingerprint == &input.recorded_input_fingerprint => {}
            Ok(_) => {
                freshness = FreshnessWithinScope::Stale;
                reason = "recorded claim input changed".to_owned();
            }
            Err(WorkspaceError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
                freshness = FreshnessWithinScope::Stale;
                reason = "recorded claim input unavailable".to_owned();
            }
            Err(_) if freshness != FreshnessWithinScope::Stale => {
                freshness = FreshnessWithinScope::Unknown;
                reason = "recorded claim input could not be verified".to_owned();
            }
            Err(_) => {}
        }
        fingerprint_inputs.push((input.path.clone(), input.selector.clone(), current.ok()));
    }

    (freshness, reason, fingerprint_inputs)
}

pub(crate) fn resolve_repository_file(
    repository_root: &Path,
    relative_path: &Path,
) -> Result<PathBuf, WorkspaceError> {
    let resolved = repository_root.join(relative_path).canonicalize()?;
    if !resolved.starts_with(repository_root) {
        return Err(WorkspaceError::InvalidPath(relative_path.to_owned()));
    }
    if !resolved.is_file() {
        return Err(WorkspaceError::InvalidObservation(format!(
            "{} is not a regular file",
            relative_path.display()
        )));
    }
    Ok(resolved)
}

/// Fingerprint a whole-file claim dependency, auto-detecting the canonical
/// normalizer from the path (the kernel-side half of the `auto` default). The
/// raw fingerprint is returned only when the normalizer makes it distinct in
/// meaning from the input fingerprint, for the reconcile fast path.
pub(crate) fn fingerprint_dependency(
    repository_root: &Path,
    relative_path: &Path,
) -> Result<(Normalizer, String, Option<String>), WorkspaceError> {
    let bytes = fs::read(resolve_repository_file(repository_root, relative_path)?)?;
    let normalizer = crate::normalizer_config::resolve_for_path(repository_root, relative_path)?;
    let input_fingerprint = hex_digest(&normalize_unit(&bytes, normalizer));
    let raw_fingerprint = (normalizer != Normalizer::None).then(|| hex_digest(&bytes));
    Ok((normalizer, input_fingerprint, raw_fingerprint))
}

pub(crate) fn select_observation_unit<'a>(
    container: &'a [u8],
    selector: &ObservationSelector,
) -> Result<&'a [u8], WorkspaceError> {
    match selector {
        ObservationSelector::WholeFile => Ok(container),
        ObservationSelector::ByteRange { start, end } => {
            if start > end || *end > container.len() {
                return Err(WorkspaceError::InvalidObservation(format!(
                    "byte range {start}:{end} is outside a {}-byte file",
                    container.len()
                )));
            }
            let text = std::str::from_utf8(container).map_err(|_| {
                WorkspaceError::InvalidObservation("source is not valid UTF-8".to_owned())
            })?;
            if !text.is_char_boundary(*start) || !text.is_char_boundary(*end) {
                return Err(WorkspaceError::InvalidObservation(format!(
                    "byte range {start}:{end} does not align to UTF-8 boundaries"
                )));
            }
            Ok(&container[*start..*end])
        }
    }
}

/// Recompute the freshness verdict for a single bound location against the live
/// worktree, returning `(freshness, reason, reconciliation_fingerprint)`. Shared
/// verbatim by observation and finding reconciliation — both bind to one
/// location, so both must decide "did the input under this change" identically;
/// keeping the decision here is what guarantees they never drift.
pub(crate) fn location_freshness_verdict(
    repository_root: &Path,
    path: &Path,
    selector: &ObservationSelector,
    normalizer: Normalizer,
    observed_raw_fingerprint: Option<&str>,
    observed_input_fingerprint: &str,
    observed_container_fingerprint: &str,
) -> Result<(FreshnessWithinScope, String, String), WorkspaceError> {
    let current = read_observation_fingerprints(
        repository_root,
        path,
        selector,
        normalizer,
        observed_raw_fingerprint,
        observed_input_fingerprint,
    );
    let (current_unit, current_container) = current
        .as_ref()
        .map(|(unit, container)| (Some(unit.as_str()), Some(container.as_str())))
        .unwrap_or((None, None));
    let reconciliation_fingerprint = observation_reconciliation_fingerprint(
        repository_root,
        path,
        selector,
        current_unit,
        current_container,
    )?;
    let (freshness, reason) = match &current {
        Ok((unit, container)) if unit == observed_input_fingerprint => {
            let reason = if container == observed_container_fingerprint {
                "supporting input unchanged"
            } else {
                "observed unit unchanged; container changed outside mediated unit"
            };
            (FreshnessWithinScope::Current, reason.to_owned())
        }
        Ok(_) => (
            FreshnessWithinScope::Stale,
            "supporting input changed".to_owned(),
        ),
        Err(WorkspaceError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => (
            FreshnessWithinScope::Stale,
            "supporting input unavailable".to_owned(),
        ),
        Err(_) => (
            FreshnessWithinScope::Unknown,
            "supporting input could not be verified".to_owned(),
        ),
    };
    Ok((freshness, reason, reconciliation_fingerprint))
}

pub(crate) fn read_observation_fingerprints(
    repository_root: &Path,
    path: &Path,
    selector: &ObservationSelector,
    normalizer: Normalizer,
    recorded_raw_fingerprint: Option<&str>,
    recorded_input_fingerprint: &str,
) -> Result<(String, String), WorkspaceError> {
    let container = fs::read(resolve_repository_file(repository_root, path)?)?;
    let unit = select_observation_unit(&container, selector)?;
    // Fast path: unchanged raw bytes imply an unchanged canonical form (the
    // normalizer is deterministic), so the recorded input fingerprint still
    // stands and no formatter subprocess is needed. Records without a raw
    // fingerprint — everything written before this existed, and every `None`
    // record — simply never fast-path.
    let unit_fingerprint = match recorded_raw_fingerprint {
        Some(raw) if raw == hex_digest(unit) => recorded_input_fingerprint.to_owned(),
        _ => hex_digest(&normalize_unit(unit, normalizer)),
    };
    Ok((unit_fingerprint, hex_digest(&container)))
}

/// Canonicalize an observed unit before fingerprinting. `None` returns the bytes
/// unchanged. `Rustfmt` returns the rustfmt-canonical form, falling back to the
/// raw bytes whenever rustfmt is unavailable or the unit does not parse — so a
/// mid-edit or non-standalone fragment simply fingerprints as its literal bytes
/// (and thus reads as changed), never as an error.
pub(crate) fn normalize_unit(unit: &[u8], normalizer: Normalizer) -> Vec<u8> {
    match normalizer {
        Normalizer::None => unit.to_vec(),
        Normalizer::Rustfmt => rustfmt_canonical(unit).unwrap_or_else(|| unit.to_vec()),
    }
}

pub(crate) fn rustfmt_canonical(unit: &[u8]) -> Option<Vec<u8>> {
    use std::process::{Command, Stdio};
    let mut child = Command::new("rustfmt")
        .args(["--emit", "stdout", "--edition", "2021", "--quiet"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    child.stdin.take()?.write_all(unit).ok()?;
    let output = child.wait_with_output().ok()?;
    output.status.success().then_some(output.stdout)
}

pub(crate) fn observation_reconciliation_fingerprint(
    repository_root: &Path,
    path: &Path,
    selector: &ObservationSelector,
    unit_fingerprint: Option<&str>,
    container_fingerprint: Option<&str>,
) -> Result<String, WorkspaceError> {
    let revision = git_output(repository_root, &["rev-parse", "HEAD"])?;
    let mut material = revision.into_bytes();
    material.push(0);
    material.extend(path.as_os_str().as_encoded_bytes());
    material.push(0);
    append_selector_fingerprint(&mut material, selector);
    material.push(0);
    material.extend(unit_fingerprint.unwrap_or("<missing>").as_bytes());
    material.push(0);
    material.extend(container_fingerprint.unwrap_or("<missing>").as_bytes());
    Ok(hex_digest(&material))
}

pub(crate) fn append_selector_fingerprint(material: &mut Vec<u8>, selector: &ObservationSelector) {
    match selector {
        ObservationSelector::WholeFile => material.extend(b"whole_file"),
        ObservationSelector::ByteRange { start, end } => {
            material.extend(b"byte_range");
            material.extend(start.to_le_bytes());
            material.extend(end.to_le_bytes());
        }
    }
}

pub(crate) fn scoped_reconciliation_fingerprint(
    repository_root: &Path,
    inputs: &[FingerprintInput],
) -> Result<String, WorkspaceError> {
    let revision = git_output(repository_root, &["rev-parse", "HEAD"])?;
    let mut material = revision.into_bytes();
    for (path, selector, input_fingerprint) in inputs {
        material.push(0);
        material.extend(path.as_os_str().as_encoded_bytes());
        material.push(0);
        append_selector_fingerprint(&mut material, selector);
        material.push(0);
        material.extend(
            input_fingerprint
                .as_ref()
                .map(String::as_bytes)
                .unwrap_or(b"<missing>"),
        );
    }
    Ok(hex_digest(&material))
}

pub(crate) fn git_file_at_revision(
    repository_root: &Path,
    revision: &str,
    path: &Path,
) -> Result<Vec<u8>, WorkspaceError> {
    let path = path
        .to_str()
        .ok_or_else(|| WorkspaceError::Git("non-UTF-8 Git path is not yet supported".to_owned()))?;
    let object = format!("{revision}:{path}");
    git_bytes(repository_root, &["show", &object])
}

pub(crate) fn write_file_atomically(path: &Path, contents: &[u8]) -> Result<(), WorkspaceError> {
    let parent = path
        .parent()
        .ok_or_else(|| WorkspaceError::InvalidPath(path.to_owned()))?;
    let file_name = path
        .file_name()
        .ok_or_else(|| WorkspaceError::InvalidPath(path.to_owned()))?;
    let temporary = parent.join(format!(
        ".{}.agent-workspace-{}-tmp",
        file_name.to_string_lossy(),
        std::process::id()
    ));
    let result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)?;
        if let Ok(metadata) = fs::metadata(path) {
            file.set_permissions(metadata.permissions())?;
        }
        file.write_all(contents)?;
        file.sync_all()?;
        fs::rename(&temporary, path)?;
        File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

pub(crate) fn worktree_fingerprint(repository_root: &Path) -> Result<String, WorkspaceError> {
    let listed = git_bytes(
        repository_root,
        &[
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
        ],
    )?;
    let mut paths: Vec<_> = listed
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| path.to_vec())
        .collect();
    paths.sort();
    let mut material = Vec::new();
    for encoded_path in paths {
        let path = PathBuf::from(String::from_utf8(encoded_path).map_err(|error| {
            WorkspaceError::Git(format!("non-UTF-8 Git path is not yet supported: {error}"))
        })?);
        material.extend(path.as_os_str().as_encoded_bytes());
        material.push(0);
        let absolute_path = repository_root.join(&path);
        match fs::symlink_metadata(&absolute_path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                material.extend(b"<symlink>");
                let target = fs::read_link(&absolute_path)?;
                material.extend(target.as_os_str().as_encoded_bytes());
            }
            Ok(metadata) if metadata.is_dir() => material.extend(b"<directory>"),
            Ok(_) => material.extend(fs::read(&absolute_path)?),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                material.extend(b"<missing>")
            }
            Err(error) => return Err(WorkspaceError::Io(error)),
        }
        material.push(0);
    }
    Ok(hex_digest(&material))
}

pub(crate) fn git_output(
    repository_root: &Path,
    arguments: &[&str],
) -> Result<String, WorkspaceError> {
    let output = git_bytes(repository_root, arguments)?;
    String::from_utf8(output)
        .map(|value| value.trim().to_owned())
        .map_err(|error| WorkspaceError::Git(error.to_string()))
}

pub(crate) fn git_bytes(
    repository_root: &Path,
    arguments: &[&str],
) -> Result<Vec<u8>, WorkspaceError> {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(repository_root)
        .output()?;
    if !output.status.success() {
        return Err(WorkspaceError::Git(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ));
    }
    Ok(output.stdout)
}

/// A read-capture byte-window plan: the byte selector the observation records
/// and the raw fingerprint of the selected unit, used to fail closed if the
/// file drifts between the harness read and the kernel's own read.
pub(crate) struct ReadSelectionPlan {
    pub(crate) selector: ObservationSelector,
    pub(crate) expected_raw_fingerprint: String,
}

/// Concise `ReadCaptureOutcome::Skipped` constructor for the capture guards.
pub(crate) fn skip(reason: impl Into<String>) -> ReadCaptureOutcome {
    ReadCaptureOutcome::Skipped {
        reason: reason.into(),
    }
}

/// Map a `read` tool's one-indexed line window onto a UTF-8 byte range and
/// verify the model actually saw it. `file_text` is the current file; `offset`
/// and `limit` are the read's line window (`None` = whole file); `visible` is
/// the raw selected text the model saw. Returns the byte selector plus the
/// selected unit's fingerprint, or a fail-closed skip reason.
///
/// The match is exact and the kernel is harness-agnostic by design: `visible`
/// must be *only* the selected lines, with any harness chrome (line-number
/// prefixes, pagination or truncation notices) already stripped by the adapter.
/// The kernel knows no harness's presentation format; each adapter decodes its
/// own back to raw text before forwarding.
pub(crate) fn plan_read_selection(
    file_text: &str,
    offset: Option<usize>,
    limit: Option<usize>,
    visible: &str,
) -> Result<ReadSelectionPlan, &'static str> {
    let lines: Vec<&str> = file_text.split('\n').collect();
    let start_line = offset.unwrap_or(1) - 1;
    if start_line >= lines.len() {
        return Err("read starts beyond the current file");
    }
    let end_line = match limit {
        Some(limit) => (start_line + limit).min(lines.len()),
        None => lines.len(),
    };
    let selected = lines[start_line..end_line].join("\n");

    // Fail closed unless the model saw exactly the current selected bytes. Any
    // difference means the file drifted under the read (or the adapter forwarded
    // un-stripped chrome, which is the adapter's bug to fix, not the kernel's).
    if visible != selected {
        return Err("model-visible read result does not match the current file selection");
    }

    // The prefix is every line before the window plus its terminating newline;
    // its byte length is where the selected unit begins in the container.
    let start = if start_line == 0 {
        0
    } else {
        lines[..start_line].join("\n").len() + 1
    };
    let end = start + selected.len();
    let whole_file = offset.is_none() && limit.is_none() && start == 0 && end == file_text.len();
    let selector = if whole_file {
        ObservationSelector::WholeFile
    } else {
        ObservationSelector::ByteRange { start, end }
    };
    Ok(ReadSelectionPlan {
        selector,
        expected_raw_fingerprint: hex_digest(selected.as_bytes()),
    })
}

/// Whether a repository-relative path names a file auto-capture must never
/// ingest — dotfiles and directories that conventionally hold secrets, and
/// key/certificate extensions. Matching is per path component so a match is a
/// whole segment (`secrets/…`, `credentials.json`), never a substring.
pub(crate) fn is_sensitive_repository_path(path: &Path) -> bool {
    const SENSITIVE_DIRECTORIES: [&str; 3] = [".ssh", ".aws", ".gnupg"];
    const SENSITIVE_NAMES: [&str; 4] = ["secret", "secrets", "credential", "credentials"];
    const SENSITIVE_EXTENSIONS: [&str; 4] = ["pem", "key", "p12", "pfx"];

    let components: Vec<String> = path
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .map(|component| component.to_ascii_lowercase())
        .collect();
    for (index, component) in components.iter().enumerate() {
        let is_directory = index + 1 < components.len();
        if component == ".env" || component.starts_with(".env.") {
            return true;
        }
        if is_directory && SENSITIVE_DIRECTORIES.contains(&component.as_str()) {
            return true;
        }
        for name in SENSITIVE_NAMES {
            if component == name || component.starts_with(&format!("{name}.")) {
                return true;
            }
        }
    }
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .is_some_and(|extension| SENSITIVE_EXTENSIONS.contains(&extension.as_str()))
}

pub(crate) fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub(crate) fn hex_digest(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}
