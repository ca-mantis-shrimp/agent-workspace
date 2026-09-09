//! Repository reconciliation primitives: pure functions that turn repository
//! state into fingerprints and freshness verdicts, plus the git/filesystem and
//! read-capture helpers that requires. Extracted verbatim from `lib.rs`; this is
//! a leaf layer — nothing here references `Workspace` or `Projection`.

use serde::Serialize;
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
    let input_fingerprint = hex_digest(&normalize_unit(
        &bytes,
        normalizer,
        repository_root,
        relative_path,
    ));
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
        _ => hex_digest(&normalize_unit(unit, normalizer, repository_root, path)),
    };
    Ok((unit_fingerprint, hex_digest(&container)))
}

/// What a supporting file looks like now that a claim resting on it reads stale,
/// scoped to the observed region. This is an *investigation aid*, never a
/// freshness verdict — it is derived read-only after reconciliation has already
/// spoken, so it cannot influence the trust-critical accept path.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DriftView {
    /// A unified diff of the supporting file — its last committed form (`HEAD`)
    /// versus the working tree — filtered to the hunks overlapping the observed
    /// region. This is the case that hurts: committed code edited under a claim
    /// before the edit itself is committed.
    Diff { text: String, truncated: bool },
    /// No committed baseline diff is available to show — the file is untracked,
    /// or the drift was already committed so `HEAD` equals the working tree.
    /// Falls back to the current bytes at the observed selector, so the agent at
    /// least re-reads exactly its observation site without hunting for it.
    CurrentContent { text: String, truncated: bool },
    /// The supporting file no longer exists.
    Missing,
}

/// A claim's stale verdict made legible: the claim-level freshness and reason,
/// plus a per-input drift breakdown. The assembly a `explain-stale` invocation
/// returns.
#[derive(Clone, Debug, Serialize)]
pub struct StaleExplanation {
    pub claim_id: u64,
    pub statement: String,
    pub freshness: FreshnessWithinScope,
    pub reason: String,
    pub inputs: Vec<InputDrift>,
}

/// Whether one supporting input drifted, and (when it did) what it looks like now.
#[derive(Clone, Debug, Serialize)]
pub struct InputDrift {
    pub path: PathBuf,
    pub selector: ObservationSelector,
    pub status: DriftStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub view: Option<DriftView>,
    /// For a changed byte-range input, whether the exact observed bytes still
    /// occur in the current file. Distinguishes coordinate drift (the unit merely
    /// moved) from a genuine rewrite — the signal that measures whether
    /// relocatable selectors would pay off. `None` when the probe does not apply
    /// (whole-file selector, no retrievable capture baseline, unchanged input).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relocation: Option<RelocationProbe>,
}

/// Whether a changed unit's exact observed bytes survived elsewhere in the file.
/// A diagnostic only — it never affects a freshness verdict; it exists to measure
/// how often a stale verdict is pure coordinate drift.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RelocationProbe {
    /// The exact observed bytes still occur in the current file — the unit likely
    /// just moved. `occurrences` separates a unique relocation from an ambiguous
    /// one (a relocatable selector could only safely follow a unique match).
    Relocated { occurrences: usize },
    /// The exact observed bytes occur nowhere now — a genuine rewrite, which no
    /// amount of relocation could have saved.
    Rewritten,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftStatus {
    /// The recorded input fingerprint still stands — not a culprit.
    Unchanged,
    /// The observed unit's canonical form changed.
    Changed,
    /// The supporting file is gone.
    Unavailable,
    /// The unit could not be re-read to a fingerprint (non-NotFound error).
    Unverifiable,
}

/// Explain, per supporting input, why a claim may read stale. Reuses the exact
/// reconcile fingerprint reader so "changed" here means precisely what "stale"
/// means to the verdict — no second, drifting definition of change. Only culprits
/// (changed / unavailable / unverifiable) carry a [`DriftView`]; unchanged inputs
/// are reported without one to keep the explanation focused.
pub(crate) fn explain_claim_inputs(
    repository_root: &Path,
    inputs: &[ClaimInput],
    max_bytes: usize,
) -> Vec<InputDrift> {
    inputs
        .iter()
        .map(|input| {
            let assessed = read_observation_fingerprints(
                repository_root,
                &input.path,
                &input.selector,
                input.normalizer,
                input.recorded_raw_fingerprint.as_deref(),
                &input.recorded_input_fingerprint,
            );
            let (status, view, relocation) = match assessed {
                Ok((fingerprint, _)) if fingerprint == input.recorded_input_fingerprint => {
                    (DriftStatus::Unchanged, None, None)
                }
                Ok(_) => (
                    DriftStatus::Changed,
                    Some(investigate_drift(
                        repository_root,
                        &input.path,
                        &input.selector,
                        input.recorded_at_revision.as_deref(),
                        max_bytes,
                    )),
                    probe_relocation(
                        repository_root,
                        &input.path,
                        &input.selector,
                        input.recorded_at_revision.as_deref(),
                    ),
                ),
                Err(WorkspaceError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
                    (DriftStatus::Unavailable, Some(DriftView::Missing), None)
                }
                Err(_) => (
                    DriftStatus::Unverifiable,
                    Some(investigate_drift(
                        repository_root,
                        &input.path,
                        &input.selector,
                        input.recorded_at_revision.as_deref(),
                        max_bytes,
                    )),
                    None,
                ),
            };
            InputDrift {
                path: input.path.clone(),
                selector: input.selector.clone(),
                status,
                view,
                relocation,
            }
        })
        .collect()
}

/// Produce a bounded, selector-scoped view of a drifted supporting file. Diffs
/// the working tree against `revision` (the git baseline the input was captured
/// at; `HEAD` when `None`) and scopes the result to the observed region; degrades
/// to the current bytes at the selector when git has no baseline diff to show —
/// an untracked file, or a revision the file did not change against. Byte-capped
/// so a huge file never blows the projection budget.
pub(crate) fn investigate_drift(
    repository_root: &Path,
    path: &Path,
    selector: &ObservationSelector,
    revision: Option<&str>,
    max_bytes: usize,
) -> DriftView {
    let Ok(bytes) = fs::read(repository_root.join(path)) else {
        return DriftView::Missing;
    };
    let text = String::from_utf8_lossy(&bytes);
    let span = selector_line_span(&text, selector);

    // Content and span come from the working tree at the superproject path, but the
    // diff must run in the repo that owns the file (a submodule sees its own
    // history; the superproject sees only an opaque gitlink).
    let (git_root, git_path) = git_context(repository_root, path);
    if let Some(git_path) = git_path.to_str() {
        let base = revision.unwrap_or("HEAD");
        if let Ok(raw) = git_bytes(&git_root, &["diff", "--no-color", base, "--", git_path]) {
            let diff = String::from_utf8_lossy(&raw);
            let scoped = scope_diff_to_span(&diff, span);
            if !scoped.trim().is_empty() {
                let (text, truncated) = cap_text(&scoped, max_bytes);
                return DriftView::Diff { text, truncated };
            }
        }
    }

    let (text, truncated) = cap_text(selector_window(&text, selector), max_bytes);
    DriftView::CurrentContent { text, truncated }
}

/// Measure whether a changed unit's *exact observed bytes* still occur in the
/// current file. Fetches the unit from its capture revision (the only place the
/// observed bytes are retained — the kernel keeps no native payload for source
/// reads) and scans the current file for it. Only meaningful for a byte-range
/// unit against a retrievable baseline; `None` otherwise, and never consulted by
/// any freshness verdict.
fn probe_relocation(
    repository_root: &Path,
    path: &Path,
    selector: &ObservationSelector,
    revision: Option<&str>,
) -> Option<RelocationProbe> {
    // A whole-file unit cannot "relocate" — searching a whole old file inside the
    // new one answers nothing about coordinate drift.
    if matches!(selector, ObservationSelector::WholeFile) {
        return None;
    }
    let (git_root, git_path) = git_context(repository_root, path);
    let old_container = git_file_at_revision(&git_root, revision?, &git_path).ok()?;
    let old_unit = select_observation_unit(&old_container, selector).ok()?;
    if old_unit.is_empty() {
        return None;
    }
    let current = fs::read(repository_root.join(path)).ok()?;
    match count_subslice(&current, old_unit) {
        0 => Some(RelocationProbe::Rewritten),
        occurrences => Some(RelocationProbe::Relocated { occurrences }),
    }
}

/// Count non-overlapping occurrences of `needle` in `haystack`.
fn count_subslice(haystack: &[u8], needle: &[u8]) -> usize {
    if needle.is_empty() || needle.len() > haystack.len() {
        return 0;
    }
    let mut count = 0;
    let mut index = 0;
    while index + needle.len() <= haystack.len() {
        if &haystack[index..index + needle.len()] == needle {
            count += 1;
            index += needle.len();
        } else {
            index += 1;
        }
    }
    count
}

/// One-indexed inclusive line span the selector covers in `text`. `WholeFile`
/// returns `None` — no hunk scoping, show the whole diff.
fn selector_line_span(text: &str, selector: &ObservationSelector) -> Option<(usize, usize)> {
    match selector {
        ObservationSelector::WholeFile => None,
        ObservationSelector::ByteRange { start, end } => {
            let line_at = |byte: usize| {
                let byte = byte.min(text.len());
                text.as_bytes()[..byte]
                    .iter()
                    .filter(|b| **b == b'\n')
                    .count()
                    + 1
            };
            Some((line_at(*start), line_at(*end)))
        }
    }
}

/// The current bytes at the selector, boundary-safe. Falls back to the whole file
/// if the recorded byte range is not on UTF-8 boundaries (it should be, but this
/// is an investigation aid and must never panic).
fn selector_window<'a>(text: &'a str, selector: &ObservationSelector) -> &'a str {
    match selector {
        ObservationSelector::WholeFile => text,
        ObservationSelector::ByteRange { start, end } => {
            text.get(*start..(*end).max(*start)).unwrap_or(text)
        }
    }
}

/// Keep only the unified-diff hunks whose new-file line span overlaps `span`,
/// preserving the pre-hunk header. `None` keeps everything.
fn scope_diff_to_span(diff: &str, span: Option<(usize, usize)>) -> String {
    let Some((lo, hi)) = span else {
        return diff.to_owned();
    };
    let mut out = String::new();
    let mut in_hunk = false;
    let mut keep_hunk = false;
    for line in diff.split_inclusive('\n') {
        if line.starts_with("@@") {
            in_hunk = true;
            keep_hunk = hunk_overlaps_new_span(line, lo, hi);
        } else if !in_hunk {
            out.push_str(line);
            continue;
        }
        if keep_hunk {
            out.push_str(line);
        }
    }
    out
}

/// Does a `@@ -a,b +c,d @@` hunk's new-file line span `[c, c+d)` overlap the
/// inclusive one-indexed `[lo, hi]`? Unparseable headers are kept (fail open on
/// display only — this cannot affect a verdict).
fn hunk_overlaps_new_span(header: &str, lo: usize, hi: usize) -> bool {
    let Some(plus) = header
        .split_whitespace()
        .find(|token| token.starts_with('+'))
    else {
        return true;
    };
    let mut numbers = plus[1..].split(',');
    let Some(start) = numbers.next().and_then(|value| value.parse::<usize>().ok()) else {
        return true;
    };
    let count = numbers
        .next()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(1);
    let hunk_hi = start + count.saturating_sub(1);
    start <= hi && lo <= hunk_hi
}

/// Truncate on a UTF-8 boundary, reporting whether anything was cut.
fn cap_text(text: &str, max_bytes: usize) -> (String, bool) {
    if text.len() <= max_bytes {
        return (text.to_owned(), false);
    }
    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    (text[..end].to_owned(), true)
}

/// Canonicalize an observed unit before fingerprinting. `None` returns the bytes
/// unchanged. `Rustfmt`/`Prettier` return the formatter-canonical form, falling
/// back to the raw bytes whenever the formatter is unavailable or the unit does
/// not parse — so a mid-edit or non-standalone fragment simply fingerprints as
/// its literal bytes (and thus reads as changed), never as an error.
///
/// `repository_root`/`path` are the file's location, needed by formatters whose
/// canonical form is project-contextual: prettier resolves its parser and
/// `.prettierrc` from the file path, and uses the repo-local pinned binary.
/// rustfmt ignores them (its edition is fixed here).
pub(crate) fn normalize_unit(
    unit: &[u8],
    normalizer: Normalizer,
    repository_root: &Path,
    path: &Path,
) -> Vec<u8> {
    let raw = || unit.to_vec();
    match normalizer {
        Normalizer::None => unit.to_vec(),
        Normalizer::Rustfmt => rustfmt_canonical(unit).unwrap_or_else(raw),
        Normalizer::Prettier => prettier_canonical(unit, repository_root, path).unwrap_or_else(raw),
        Normalizer::Black => black_canonical(unit, repository_root, path).unwrap_or_else(raw),
        Normalizer::RuffFormat => {
            ruff_format_canonical(unit, repository_root, path).unwrap_or_else(raw)
        }
    }
}

pub(crate) fn rustfmt_canonical(unit: &[u8]) -> Option<Vec<u8>> {
    run_formatter(Command::new("rustfmt").args([
        "--emit",
        "stdout",
        "--edition",
        "2021",
        "--quiet",
    ]))(unit)
}

/// Drive prettier over stdin. The project-local `node_modules/.bin/prettier` is
/// preferred — it is the version the repo's lockfile pins and the version
/// format-on-save actually runs — so the canonical form matches the editor's and
/// is reproducible; a bare `prettier` on PATH is the fallback. `--stdin-filepath`
/// hands prettier the path so it selects the parser (`.ts`, `.tsx`, `.js`, …) and
/// discovers the project's own config, and running from the repo root anchors
/// that discovery.
pub(crate) fn prettier_canonical(
    unit: &[u8],
    repository_root: &Path,
    path: &Path,
) -> Option<Vec<u8>> {
    let program =
        resolve_formatter_binary(repository_root, &["node_modules/.bin/prettier"], "prettier");
    let mut command = Command::new(program);
    command
        .current_dir(repository_root)
        .arg("--stdin-filepath")
        .arg(path);
    run_formatter(&mut command)(unit)
}

/// Drive `black` over stdin, preferring a project virtualenv binary. `black -`
/// reads stdin and writes the formatted result to stdout; `--stdin-filename`
/// gives it the path for config discovery (`pyproject.toml [tool.black]`) and
/// per-file excludes.
pub(crate) fn black_canonical(unit: &[u8], repository_root: &Path, path: &Path) -> Option<Vec<u8>> {
    let program = resolve_formatter_binary(
        repository_root,
        &[".venv/bin/black", "venv/bin/black"],
        "black",
    );
    let mut command = Command::new(program);
    command
        .current_dir(repository_root)
        .arg("-q")
        .arg("--stdin-filename")
        .arg(path)
        .arg("-");
    run_formatter(&mut command)(unit)
}

/// Drive `ruff format` over stdin (black-compatible), preferring a project
/// virtualenv binary. `ruff format --stdin-filename <path> -` reads stdin, writes
/// the formatted result to stdout, and uses the path to discover the project's
/// `pyproject.toml`/`ruff.toml` config.
pub(crate) fn ruff_format_canonical(
    unit: &[u8],
    repository_root: &Path,
    path: &Path,
) -> Option<Vec<u8>> {
    let program = resolve_formatter_binary(
        repository_root,
        &[".venv/bin/ruff", "venv/bin/ruff"],
        "ruff",
    );
    let mut command = Command::new(program);
    command
        .current_dir(repository_root)
        .arg("format")
        .arg("--stdin-filename")
        .arg(path)
        .arg("-");
    run_formatter(&mut command)(unit)
}

/// Resolve a formatter binary, preferring a repo-local install (which the
/// project's environment pins) over a PATH binary. `candidates` are repo-relative
/// paths tried in order; the first that exists wins, else the PATH `fallback`.
/// This is what lets a canonical form be reproducible across environments rather
/// than hostage to whatever version happens to be on PATH.
fn resolve_formatter_binary(
    repository_root: &Path,
    candidates: &[&str],
    fallback: &str,
) -> std::ffi::OsString {
    for candidate in candidates {
        let local = repository_root.join(candidate);
        if local.is_file() {
            return local.into_os_string();
        }
    }
    fallback.into()
}

/// Shared formatter-subprocess plumbing: pipe `unit` to the configured command's
/// stdin and return its stdout only on a clean exit. Any spawn/IO/non-zero-exit
/// outcome is `None`, which the callers turn into the raw-bytes fallback.
fn run_formatter(command: &mut Command) -> impl FnOnce(&[u8]) -> Option<Vec<u8>> + '_ {
    use std::process::Stdio;
    move |unit| {
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        child.stdin.take()?.write_all(unit).ok()?;
        let output = child.wait_with_output().ok()?;
        output.status.success().then_some(output.stdout)
    }
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

/// A submodule that owns an observed/mutated path, when the path lives inside one.
/// `None` means the superproject (or a plain checkout) owns the file directly, and
/// callers should keep today's superproject-rooted git behavior.
pub(crate) struct SubmoduleContext {
    /// Absolute toplevel of the owning submodule's working tree.
    pub(crate) root: PathBuf,
    /// The submodule's directory relative to the superproject — the gitlink path
    /// recorded in the superproject tree, i.e. the argument to `git ls-tree`.
    pub(crate) gitlink_path: PathBuf,
    /// The path relative to the submodule root.
    pub(crate) relative_path: PathBuf,
}

/// Resolve the git repository that actually owns `path`. Git at the superproject
/// root sees a submodule as one opaque gitlink, so a `git show`/`git diff` for a
/// file *inside* a submodule must run in the submodule's own repo instead. Returns
/// `None` — degrade to superproject behavior — whenever git cannot place the file,
/// so plain checkouts and any resolution failure behave exactly as before.
pub(crate) fn owning_submodule(repository_root: &Path, path: &Path) -> Option<SubmoduleContext> {
    let absolute = repository_root.join(path);
    let start = absolute.parent().unwrap_or(absolute.as_path());
    let toplevel = git_output(start, &["rev-parse", "--show-toplevel"]).ok()?;
    let root = fs::canonicalize(toplevel).ok()?;
    let super_root = fs::canonicalize(repository_root).ok()?;
    if root == super_root {
        return None;
    }
    let gitlink_path = root.strip_prefix(&super_root).ok()?.to_path_buf();
    let relative_path = fs::canonicalize(&absolute)
        .ok()?
        .strip_prefix(&root)
        .ok()?
        .to_path_buf();
    Some(SubmoduleContext {
        root,
        gitlink_path,
        relative_path,
    })
}

/// The `(git root, git-relative path)` to run a `git` operation for `path` in —
/// the owning submodule when `path` lives in one, else the superproject unchanged.
/// Use this when the revision being passed already belongs to the owning repo (an
/// observation's capture revision, recorded via [`owning_revision`]); the base for
/// a *transaction* is a superproject SHA and must go through [`clean_base_bytes`].
fn git_context(repository_root: &Path, path: &Path) -> (PathBuf, PathBuf) {
    match owning_submodule(repository_root, path) {
        Some(context) => (context.root, context.relative_path),
        None => (repository_root.to_path_buf(), path.to_path_buf()),
    }
}

/// The HEAD of the git repository that owns `path`: the owning submodule's HEAD for
/// a submodule file — so an observation's provenance names the revision that
/// actually contains the file, and later drift/relocation `git` reads resolve in
/// the repo where that revision exists — else the superproject HEAD.
pub(crate) fn owning_revision(
    repository_root: &Path,
    path: &Path,
) -> Result<String, WorkspaceError> {
    let (git_root, _) = git_context(repository_root, path);
    git_output(&git_root, &["rev-parse", "HEAD"])
}

/// Fetch a path's bytes at a transaction's clean base, resolving through the
/// owning git repository. In the superproject this is a plain
/// `git show <base>:<path>`. For a submodule file the superproject records only an
/// opaque gitlink at `base`, so recover the submodule's pinned commit from that
/// gitlink and show the file from the submodule's own history — keeping the S6
/// clean-base check meaningful across the boundary with no change to the recorded
/// base revision. (A dirty submodule still fails the caller's equality check,
/// which is correct: a dirty submodule is not a clean base.)
pub(crate) fn clean_base_bytes(
    repository_root: &Path,
    base_revision: &str,
    path: &Path,
) -> Result<Vec<u8>, WorkspaceError> {
    match owning_submodule(repository_root, path) {
        None => git_file_at_revision(repository_root, base_revision, path),
        Some(context) => {
            let pinned =
                pinned_submodule_revision(repository_root, base_revision, &context.gitlink_path)?;
            git_file_at_revision(&context.root, &pinned, &context.relative_path)
        }
    }
}

/// Recover a submodule's pinned commit from the superproject's gitlink at
/// `base_revision`. `git ls-tree <base> <gitlink>` prints
/// `160000 commit <sha>\t<path>`; the third whitespace-delimited field is the SHA.
fn pinned_submodule_revision(
    repository_root: &Path,
    base_revision: &str,
    gitlink_path: &Path,
) -> Result<String, WorkspaceError> {
    let gitlink = gitlink_path.to_str().ok_or_else(|| {
        WorkspaceError::Git("non-UTF-8 submodule path is not yet supported".to_owned())
    })?;
    let entry = git_output(repository_root, &["ls-tree", base_revision, gitlink])?;
    entry
        .split_whitespace()
        .nth(2)
        .map(str::to_owned)
        .ok_or_else(|| {
            WorkspaceError::Git(format!(
                "superproject base has no gitlink for submodule {gitlink}"
            ))
        })
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

// NOTE (superproject caution): this enumerates via `git ls-files` at the given
// root, which lists a submodule as a single gitlink entry and never descends into
// it — so a submodule collapses to a `<directory>` marker and this fingerprint is
// blind to any change inside a submodule. That is harmless today only because the
// value it produces (`initial_worktree_fingerprint`) is recorded at
// TransactionBegan but never read back to drive a decision. Do NOT wire this into
// a freshness/coverage check on a superproject without first making it descend
// into submodules; otherwise it will silently pass submodule drift.
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

#[cfg(test)]
mod drift_tests {
    use super::*;

    const TWO_HUNK_DIFF: &str = "diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1,3 +1,3 @@\n a\n-b\n+B\n c\n@@ -20,3 +20,3 @@\n x\n-y\n+Y\n z\n";

    #[test]
    fn scope_keeps_only_hunks_overlapping_the_selector_lines() {
        let early = scope_diff_to_span(TWO_HUNK_DIFF, Some((1, 3)));
        assert!(early.contains("+B"), "first hunk kept: {early}");
        assert!(!early.contains("+Y"), "second hunk dropped: {early}");
        assert!(early.contains("+++ b/x"), "header preserved: {early}");

        let late = scope_diff_to_span(TWO_HUNK_DIFF, Some((20, 22)));
        assert!(late.contains("+Y"), "second hunk kept: {late}");
        assert!(!late.contains("+B"), "first hunk dropped: {late}");
    }

    #[test]
    fn whole_file_selector_keeps_the_entire_diff() {
        assert_eq!(scope_diff_to_span(TWO_HUNK_DIFF, None), TWO_HUNK_DIFF);
    }

    #[test]
    fn hunk_overlap_handles_zero_count_and_unparseable_headers() {
        // Fail open on display: an unreadable header is kept, never silently lost.
        assert!(hunk_overlaps_new_span("@@ garbage @@", 1, 1));
        // `+5,0` is a pure deletion anchored at new line 5.
        assert!(hunk_overlaps_new_span("@@ -5,2 +5,0 @@", 5, 5));
        assert!(!hunk_overlaps_new_span("@@ -5,2 +5,0 @@", 1, 3));
    }

    #[test]
    fn selector_line_span_counts_newlines() {
        let text = "one\ntwo\nthree\n";
        // Bytes 4..7 are "two" — the second line.
        assert_eq!(
            selector_line_span(text, &ObservationSelector::ByteRange { start: 4, end: 7 }),
            Some((2, 2))
        );
        assert_eq!(
            selector_line_span(text, &ObservationSelector::WholeFile),
            None
        );
    }

    #[test]
    fn count_subslice_counts_non_overlapping_matches() {
        assert_eq!(count_subslice(b"abcabcabc", b"abc"), 3);
        assert_eq!(count_subslice(b"aaaa", b"aa"), 2); // non-overlapping
        assert_eq!(count_subslice(b"hello world", b"xyz"), 0);
        assert_eq!(count_subslice(b"", b"a"), 0);
        assert_eq!(count_subslice(b"a", b""), 0);
        assert_eq!(count_subslice(b"short", b"longer needle"), 0);
    }

    #[test]
    fn cap_text_truncates_on_a_char_boundary() {
        let (text, truncated) = cap_text("abcdef", 3);
        assert_eq!(text, "abc");
        assert!(truncated);

        let (text, truncated) = cap_text("abc", 10);
        assert_eq!(text, "abc");
        assert!(!truncated);

        // A cap landing mid-multibyte-char backs off to the boundary below it.
        let (text, truncated) = cap_text("aé", 2);
        assert_eq!(text, "a");
        assert!(truncated);
    }
}
