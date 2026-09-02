use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Component, Path, PathBuf};
use std::process::Command;

const EVENT_SCHEMA_VERSION: u32 = 2;
const MINIMUM_EVENT_SCHEMA_VERSION: u32 = 1;
const EVENT_LOG_NAME: &str = "events.jsonl";
const LOCK_FILE_NAME: &str = "events.lock";
const MAX_RETAINED_PAYLOAD_BYTES: usize = 1024 * 1024;
type FingerprintInput = (PathBuf, ObservationSelector, Option<String>);
type ClaimAssessment = (FreshnessWithinScope, String, Vec<FingerprintInput>);

#[derive(Debug)]
pub enum WorkspaceError {
    Io(std::io::Error),
    Json(serde_json::Error),
    InvalidPath(PathBuf),
    InvalidObjective(String),
    InvalidWorkingSet(String),
    Git(String),
    ObservationNotFound(u64),
    InvalidObservation(String),
    ClaimNotFound(u64),
    InvalidClaim(String),
    EvidenceNotFound(u64),
    InvalidEvidence(String),
    FindingNotFound(u64),
    InvalidFinding(String),
    TransactionNotFound(u64),
    InvalidTransaction(String),
    InvalidCheckpoint(String),
    CheckpointNotFound(String),
    CorruptLog(String),
}

impl fmt::Display for WorkspaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "I/O error: {error}"),
            Self::Json(error) => write!(formatter, "JSON error: {error}"),
            Self::InvalidPath(path) => {
                write!(
                    formatter,
                    "path must be repository-relative: {}",
                    path.display()
                )
            }
            Self::InvalidObjective(message) => write!(formatter, "invalid objective: {message}"),
            Self::InvalidWorkingSet(message) => {
                write!(formatter, "invalid working set entry: {message}")
            }
            Self::Git(message) => write!(formatter, "Git error: {message}"),
            Self::ObservationNotFound(id) => write!(formatter, "observation {id} not found"),
            Self::InvalidObservation(message) => {
                write!(formatter, "invalid observation: {message}")
            }
            Self::ClaimNotFound(id) => write!(formatter, "claim {id} not found"),
            Self::InvalidClaim(message) => write!(formatter, "invalid claim: {message}"),
            Self::EvidenceNotFound(id) => write!(formatter, "evidence {id} not found"),
            Self::InvalidEvidence(message) => write!(formatter, "invalid evidence: {message}"),
            Self::FindingNotFound(id) => write!(formatter, "finding {id} not found"),
            Self::InvalidFinding(message) => write!(formatter, "invalid finding: {message}"),
            Self::TransactionNotFound(id) => write!(formatter, "transaction {id} not found"),
            Self::InvalidTransaction(message) => {
                write!(formatter, "invalid transaction: {message}")
            }
            Self::InvalidCheckpoint(message) => write!(formatter, "invalid checkpoint: {message}"),
            Self::CheckpointNotFound(label) => {
                write!(formatter, "checkpoint {label:?} not found")
            }
            Self::CorruptLog(message) => write!(formatter, "corrupt event log: {message}"),
        }
    }
}

impl std::error::Error for WorkspaceError {}

impl From<std::io::Error> for WorkspaceError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for WorkspaceError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FreshnessWithinScope {
    Current,
    Stale,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScopeSource {
    Declared,
    Derived,
    Conservative,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScopeCompleteness {
    AssertedComplete,
    NotAsserted,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ScopeAssurance {
    pub source: ScopeSource,
    pub completeness: ScopeCompleteness,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MediatedUnit {
    pub path: PathBuf,
    #[serde(default)]
    pub selector: ObservationSelector,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OperationalCoverage {
    pub mediated_paths: Vec<PathBuf>,
    #[serde(default)]
    pub mediated_units: Vec<MediatedUnit>,
    pub reconciliation_fingerprint: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FreshnessReport {
    pub freshness_within_scope: FreshnessWithinScope,
    pub scope_assurance: ScopeAssurance,
    pub operational_coverage: OperationalCoverage,
    pub reason: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ObservationSelector {
    #[default]
    WholeFile,
    ByteRange {
        start: usize,
        end: usize,
    },
}

/// How an observed unit's bytes are canonicalized before fingerprinting.
/// `None`) fingerprints raw bytes — any change, cosmetic or not, is a
/// change. A formatter normalizer fingerprints the *canonical* form instead, so
/// a pure reformat (same meaning, different bytes) is not seen as a change while
/// a real edit still is. Because the normalizer rides beside the selector on
/// both observations and claim inputs, record-time and reconcile-time
/// fingerprints are computed the same way and stay comparable. The serde
/// default stays `None` so records written before normalizers existed (or via
/// the `--normalize none` escape hatch) keep their byte-exact meaning; fresh
/// captures resolve the CLI's `auto` default through
/// [`Normalizer::detect_for_path`] and persist the *concrete* normalizer, so
/// reconcile always applies the scheme the record was written with.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Normalizer {
    #[default]
    None,
    /// Canonicalize Rust source with `rustfmt`. Falls back to raw bytes when
    /// rustfmt is absent or the unit does not parse (e.g. a mid-edit file, or a
    /// byte-range fragment that is not a standalone item).
    Rustfmt,
}

impl Normalizer {
    /// The normalizer a fresh capture uses for `path` when the caller did not
    /// pick one explicitly (the CLI's `auto` default, and the kernel's default
    /// for claim dependencies). Recognized source types get their canonical
    /// formatter; everything else fingerprints raw bytes. Recognition is by
    /// extension only — cheap, deterministic, and honest about not detecting
    /// anything deeper.
    pub fn detect_for_path(path: &Path) -> Self {
        match path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| extension.to_ascii_lowercase())
            .as_deref()
        {
            Some("rs") => Self::Rustfmt,
            _ => Self::None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Observation {
    pub id: u64,
    pub path: PathBuf,
    pub provider: String,
    pub observed_revision: String,
    #[serde(default)]
    pub selector: ObservationSelector,
    #[serde(default)]
    pub normalizer: Normalizer,
    pub observed_input_fingerprint: String,
    /// Fingerprint of the unit's *raw* bytes, recorded whenever the normalizer
    /// makes it differ in meaning from `observed_input_fingerprint`. Reconcile
    /// compares raw bytes first and skips the formatter subprocess when they
    /// match — a deterministic normalizer maps identical bytes to an identical
    /// canonical form. Absent on old records (and on `None` records, where the
    /// input fingerprint already is the raw one); absence simply never
    /// fast-paths.
    #[serde(default)]
    pub observed_raw_fingerprint: Option<String>,
    #[serde(default)]
    pub observed_container_fingerprint: String,
    #[serde(default)]
    pub native_payload_reference: Option<String>,
    #[serde(default)]
    pub ingested_bytes: usize,
    /// Bytes of text that the harness actually placed in model context for
    /// this observation. `None` means the capture happened outside an
    /// instrumented model boundary (for example through the standalone CLI).
    #[serde(default)]
    pub model_visible_bytes: Option<usize>,
    pub report: FreshnessReport,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ObservationCapture {
    #[serde(flatten)]
    pub observation: Observation,
    pub content: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ObservationCaptureOptions {
    pub selector: ObservationSelector,
    pub normalizer: Normalizer,
    pub retain_native_payload: bool,
    pub model_visible_bytes: Option<usize>,
    pub expected_raw_fingerprint: Option<String>,
}

/// The read-tool-native inputs an adapter forwards for auto-capture. Adapters
/// speak *lines* (the shape a coding agent's `read` tool exposes) and hold the
/// exact text the harness put in front of the model; the kernel owns the
/// translation into a byte-range observation. Keeping these raw means every
/// adapter is a thin transport and the planning semantics live in one place.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadCaptureRequest {
    /// One-indexed first line the native read returned. `None` reads from the top.
    pub offset: Option<usize>,
    /// Line count the native read returned. `None` runs through end of file.
    pub limit: Option<usize>,
    /// The raw selected text the model saw — the adapter must strip its harness's
    /// chrome (line-number prefixes, pagination/truncation notices) first. The
    /// kernel matches this exactly against the file and knows no presentation
    /// format of its own.
    pub model_visible_text: String,
    /// Total bytes delivered at the model boundary, including harness chrome
    /// stripped from `model_visible_text` before kernel matching.
    pub model_visible_bytes: Option<usize>,
    /// Whether the harness reported the native read result as truncated.
    pub truncated: bool,
}

/// The outcome of a read auto-capture. `Skipped` is deliberately a first-class
/// value, not a silent no-op: an adapter can surface *why* a read was not
/// captured (drift, truncation, sensitive path, binary file) instead of the
/// capture vanishing without trace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReadCaptureOutcome {
    Captured(Box<ObservationCapture>),
    Skipped { reason: String },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RevealedObservation {
    pub observation_id: u64,
    pub path: PathBuf,
    pub provider: String,
    pub observed_revision: String,
    pub observed_container_fingerprint: String,
    pub content: String,
    pub ingested_bytes: usize,
}

/// How a finding binds to its location and whether the provider's raw payload is
/// retained. `native_payload` is the provider's own output (e.g. diagnostic
/// JSON); when present it is retained in the CAS so the original result stays
/// retrievable (S8). The `selector`/`normalizer` describe the location the
/// finding is bound to for freshness.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FindingCaptureOptions {
    pub selector: ObservationSelector,
    pub normalizer: Normalizer,
    pub native_payload: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RevealedFinding {
    pub finding_id: u64,
    pub provider: String,
    pub path: PathBuf,
    pub observed_revision: String,
    pub native_payload_fingerprint: String,
    pub content: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimInputSource {
    SupportingObservation,
    DeclaredDependency,
    ConservativeDependency,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ClaimInput {
    pub path: PathBuf,
    #[serde(default)]
    pub selector: ObservationSelector,
    #[serde(default)]
    pub normalizer: Normalizer,
    pub recorded_input_fingerprint: String,
    /// Raw-unit fingerprint at record time; enables the same reconcile fast
    /// path as `Observation::observed_raw_fingerprint`.
    #[serde(default)]
    pub recorded_raw_fingerprint: Option<String>,
    pub source: ClaimInputSource,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimScopeStrategy {
    #[default]
    Declared,
    ConservativeSiblingFiles,
}

impl ClaimScopeStrategy {
    fn assurance_source(&self) -> ScopeSource {
        match self {
            Self::Declared => ScopeSource::Declared,
            Self::ConservativeSiblingFiles => ScopeSource::Conservative,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ClaimLifecycle {
    #[default]
    Active,
    Superseded {
        replacement_claim_id: u64,
        reason: String,
    },
}

impl ClaimLifecycle {
    fn is_active(&self) -> bool {
        matches!(self, Self::Active)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Claim {
    pub id: u64,
    pub statement: String,
    pub supporting_observation_ids: Vec<u64>,
    pub scope_strategy: ClaimScopeStrategy,
    pub inputs: Vec<ClaimInput>,
    #[serde(default)]
    pub lifecycle: ClaimLifecycle,
    pub report: FreshnessReport,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Objective {
    pub intent: String,
    pub external_reference: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WorkingSetEntry {
    pub observation_id: u64,
    pub reason: String,
    /// Monotonic focus ordinal stamped at projection time from the order of
    /// `ObservationFocused` events. It is the *recency* signal — higher means
    /// focused more recently — and the position a location occupies in the
    /// navigation trail. Derived, never stored on the event, so the append-only
    /// log stays untouched and old logs replay identically (absent → 0).
    #[serde(default)]
    pub focus_sequence: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceOutcome {
    Passed,
    Failed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Evidence {
    pub id: u64,
    pub transaction_id: u64,
    pub claim_id: u64,
    pub check_name: String,
    pub invocation: String,
    pub provider: String,
    pub outcome: EvidenceOutcome,
    pub inputs: Vec<ClaimInput>,
    /// The content-addressed candidate the transaction owned *when this evidence
    /// was recorded* — a fold over its mutation bytes (see
    /// [`Transaction::candidate_fingerprint`]). Acceptance requires this still
    /// equals the transaction's current candidate, so a passing check recorded
    /// against an earlier candidate (e.g. before another file was mutated) can
    /// never be counted as proof for the bytes actually being committed. Empty
    /// for legacy evidence replayed from before candidate binding existed;
    /// unbound legacy evidence therefore never satisfies the binding gate.
    #[serde(default)]
    pub candidate_fingerprint: String,
    pub report: FreshnessReport,
}

/// Severity of a provider-reported finding, ordered most-urgent first so a
/// bounded queue surfaces errors ahead of hints (derived `Ord`).
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingSeverity {
    Error,
    Warning,
    Info,
    Hint,
}

/// How a finding has been dispositioned. Orthogonal to freshness — the lesson
/// carried over from claim supersession: a `stale` finding can still be `open`,
/// and a `resolved` finding can still be `current`. Every non-open disposition
/// names its actor and rationale (invariant 8). Filled by sub-slice B; sub-slice
/// A only ever records `Open`.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum FindingDisposition {
    #[default]
    Open,
    Resolved {
        actor: String,
        rationale: String,
    },
    Deferred {
        actor: String,
        rationale: String,
    },
    Suppressed {
        actor: String,
        rationale: String,
    },
    FalsePositive {
        actor: String,
        rationale: String,
    },
}

impl FindingDisposition {
    fn is_open(&self) -> bool {
        matches!(self, Self::Open)
    }
}

/// A provider-reported issue bound to a repository location (a quickfix-like
/// queue entry). Its freshness reconciles from that single location exactly like
/// an [`Observation`] — an edit under the finding stales it, so a diagnostic that
/// may no longer apply never silently counts as a current issue. Provider
/// identity and the native payload (or its CAS reference) are always retained so
/// the provider's original result stays retrievable (S8).
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Finding {
    pub id: u64,
    pub provider: String,
    pub severity: FindingSeverity,
    #[serde(default)]
    pub rule: Option<String>,
    pub message: String,
    pub path: PathBuf,
    #[serde(default)]
    pub selector: ObservationSelector,
    #[serde(default)]
    pub normalizer: Normalizer,
    pub observed_revision: String,
    pub observed_input_fingerprint: String,
    #[serde(default)]
    pub observed_raw_fingerprint: Option<String>,
    #[serde(default)]
    pub observed_container_fingerprint: String,
    /// CAS reference to the provider's own raw output (e.g. the diagnostic JSON),
    /// retained so the provider's original result stays retrievable (S8). This is
    /// the *provider payload*, distinct from the source file at `path`, which is
    /// only the location this finding is bound to for freshness.
    #[serde(default)]
    pub native_payload_reference: Option<String>,
    /// Digest of that native payload — the CAS key, verified on reveal. `None`
    /// when the provider supplied no raw payload.
    #[serde(default)]
    pub native_payload_fingerprint: Option<String>,
    #[serde(default)]
    pub disposition: FindingDisposition,
    pub report: FreshnessReport,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionState {
    Open,
    Accepted,
    Reverted,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Mutation {
    pub path: PathBuf,
    pub before_fingerprint: String,
    pub after_fingerprint: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Transaction {
    pub id: u64,
    /// Why this change exists — the transaction's thesis, recorded at `begin`.
    /// `Option` for backward-compatible replay of pre-intent transactions; new
    /// transactions always carry one.
    #[serde(default)]
    pub intent: Option<String>,
    pub base_revision: String,
    pub initial_worktree_fingerprint: String,
    pub acceptance_claim_ids: Vec<u64>,
    pub evidence_ids: Vec<u64>,
    /// Findings this transaction addresses. Association only — disposing a
    /// finding stays an explicit, separately-actored act.
    #[serde(default)]
    pub finding_ids: Vec<u64>,
    /// Known residual risks the author chose to accept, in record order.
    #[serde(default)]
    pub residual_risks: Vec<String>,
    pub mutations: Vec<Mutation>,
    pub state: TransactionState,
    pub last_rejection: Option<String>,
}

impl Transaction {
    /// A single content address for the transaction's owned candidate state: the
    /// set of `(path, after_fingerprint)` mutation bytes, folded in path order so
    /// it is independent of the order mutations were applied. This is a pure
    /// projection over the mutations — no stored field, one source of truth — and
    /// it is what evidence binds to and what acceptance re-verifies against disk.
    /// A transaction with no mutations yields a stable empty-candidate digest.
    pub fn candidate_fingerprint(&self) -> String {
        let mut entries: Vec<(&Path, &str)> = self
            .mutations
            .iter()
            .map(|mutation| (mutation.path.as_path(), mutation.after_fingerprint.as_str()))
            .collect();
        entries.sort_by(|left, right| left.0.cmp(right.0));
        let mut material = Vec::new();
        for (path, after_fingerprint) in entries {
            material.extend_from_slice(path.as_os_str().as_encoded_bytes());
            material.push(0);
            material.extend_from_slice(after_fingerprint.as_bytes());
            material.push(b'\n');
        }
        hex_digest(&material)
    }
}

/// A named point in the event log. Recording a checkpoint captures *where in the
/// log* a line was drawn (its `sequence`) together with the objective in force at
/// that moment. It creates no entity state of its own; it is the anchor a delta
/// projection diffs against.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CheckpointMarker {
    pub label: String,
    #[serde(default)]
    pub note: Option<String>,
    pub git_revision: String,
    pub objective: Option<Objective>,
    pub sequence: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WorkspaceStatus {
    pub objective: Option<Objective>,
    pub working_set: Vec<WorkingSetEntry>,
    #[serde(default)]
    pub navigation_trail: Vec<WorkingSetEntry>,
    pub observations: Vec<Observation>,
    pub claims: Vec<Claim>,
    #[serde(default)]
    pub superseded_claims: Vec<Claim>,
    pub evidence: Vec<Evidence>,
    #[serde(default)]
    pub findings: Vec<Finding>,
    pub transactions: Vec<Transaction>,
    #[serde(default)]
    pub checkpoints: Vec<CheckpointMarker>,
}

/// The default `status` output: the orientation surface an agent resumes from,
/// not the full audit dump (`--full`, [`WorkspaceStatus`]). It carries the
/// objective in force, a bounded stale-first window of active claims as
/// scannable headlines with freshness and scope, a freshness histogram, and
/// counts — nothing heavier. `claims_omitted` makes truncation explicit.
/// Observations, superseded claims, evidence, transactions, and per-claim
/// operational coverage collapse to counts you can expand with `--full` or
/// `reveal`. This is a pure projection over an already-reconciled
/// [`WorkspaceStatus`]: it moves no verdict computation, it only decides what
/// to serialize.
#[derive(Clone, Debug, Serialize)]
pub struct BriefStatus {
    pub objective: Option<Objective>,
    pub claims: Vec<BriefClaim>,
    pub claims_omitted: usize,
    pub counts: BriefCounts,
    pub latest_checkpoint: Option<BriefCheckpoint>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BriefClaim {
    pub id: u64,
    pub freshness: FreshnessWithinScope,
    pub scope: ScopeAssurance,
    /// The claim's thesis, truncated to a scannable headline (full statement is
    /// one `--full` away). Claims authored thesis-first read cleanly here; a
    /// trailing `…` marks that the statement continues.
    pub headline: String,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct FreshnessHistogram {
    pub current: usize,
    pub stale: usize,
    pub unknown: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct BriefCounts {
    pub active_claims: usize,
    pub superseded_claims: usize,
    pub observations: usize,
    pub open_findings: usize,
    pub open_transactions: usize,
    pub checkpoints: usize,
    pub freshness: FreshnessHistogram,
}

#[derive(Clone, Debug, Serialize)]
pub struct BriefCheckpoint {
    pub label: String,
    pub sequence: u64,
}

impl WorkspaceStatus {
    /// Collapse the full audit status into the [`BriefStatus`] orientation
    /// surface. Freshness is read straight off each claim's already-computed
    /// report — this method never touches the log or the worktree.
    pub fn brief(&self) -> BriefStatus {
        let mut freshness = FreshnessHistogram::default();
        for claim in &self.claims {
            match claim.report.freshness_within_scope {
                FreshnessWithinScope::Current => freshness.current += 1,
                FreshnessWithinScope::Stale => freshness.stale += 1,
                FreshnessWithinScope::Unknown => freshness.unknown += 1,
            }
        }
        let mut ranked_claims: Vec<&Claim> = self.claims.iter().collect();
        ranked_claims.sort_by_key(|claim| {
            let freshness_rank = match claim.report.freshness_within_scope {
                FreshnessWithinScope::Stale => 0,
                FreshnessWithinScope::Unknown => 1,
                FreshnessWithinScope::Current => 2,
            };
            (freshness_rank, claim.id)
        });
        let claims = ranked_claims
            .into_iter()
            .take(BRIEF_CLAIM_LIMIT)
            .map(|claim| BriefClaim {
                id: claim.id,
                freshness: claim.report.freshness_within_scope.clone(),
                scope: claim.report.scope_assurance.clone(),
                headline: claim_headline(&claim.statement, BRIEF_HEADLINE_MAX_CHARS),
            })
            .collect();
        BriefStatus {
            objective: self.objective.clone(),
            claims,
            claims_omitted: self.claims.len().saturating_sub(BRIEF_CLAIM_LIMIT),
            counts: BriefCounts {
                active_claims: self.claims.len(),
                superseded_claims: self.superseded_claims.len(),
                observations: self.observations.len(),
                open_findings: self
                    .findings
                    .iter()
                    .filter(|finding| finding.disposition.is_open())
                    .count(),
                open_transactions: self
                    .transactions
                    .iter()
                    .filter(|transaction| transaction.state == TransactionState::Open)
                    .count(),
                checkpoints: self.checkpoints.len(),
                freshness,
            },
            latest_checkpoint: self.checkpoints.last().map(|marker| BriefCheckpoint {
                label: marker.label.clone(),
                sequence: marker.sequence,
            }),
        }
    }
}

/// A focused working-set entry joined to the observation it cites: the durable
/// pointer (`observation_id`, `reason`, `focus_sequence`) resolved into the
/// coordinates that make it a *location* — path, selector, the revision it was
/// observed at, and a relocation anchor. This is a pure projection, never a
/// stored entity: the observation already persists these coordinates, so
/// materializing them here keeps one source of truth and cannot drift from it.
#[derive(Clone, Debug, Serialize)]
pub struct SemanticLocation {
    pub observation_id: u64,
    pub path: PathBuf,
    pub selector: ObservationSelector,
    pub observed_revision: String,
    /// The enclosing container's fingerprint at observation time — the anchor a
    /// later reconcile compares against to notice the file moved underneath the
    /// location. Not a symbol identity: perfect relocation across refactors is a
    /// declared non-goal for this slice.
    pub relocation_fingerprint: String,
    /// Freshness read straight off the already-reconciled observation. `stale`
    /// here means an edit landed under a location you are still attending to.
    pub freshness: FreshnessWithinScope,
    pub reason: String,
    pub focus_sequence: u64,
}

/// A current observation not yet cited by any active claim and not already in
/// the working set: an attention *candidate*, the raw material a focus turns
/// into a location. Deliberately thin — enough to decide whether to focus it,
/// no heavier.
#[derive(Clone, Debug, Serialize)]
pub struct UncitedObservation {
    pub observation_id: u64,
    pub path: PathBuf,
    pub selector: ObservationSelector,
}

/// The bounded attention model: ranked semantic locations currently in focus,
/// the uncited candidates that could join them, and the ordered trail of how
/// focus moved. Every section is hard-capped with an explicit `_omitted` count
/// so a cap is always visible, never silent — the same contract `BriefStatus`
/// keeps for claims. It retains no whole-file payload: locations point at
/// observations, which is where reveal-on-demand already lives.
#[derive(Clone, Debug, Serialize)]
pub struct WorkingSetView {
    pub locations: Vec<SemanticLocation>,
    pub locations_omitted: usize,
    pub uncited: Vec<UncitedObservation>,
    pub uncited_omitted: usize,
    /// Most-recent focus first; revisits included. Recovers verbatim on restart
    /// because it is replayed from the `ObservationFocused` event order.
    pub trail: Vec<WorkingSetEntry>,
    pub trail_omitted: usize,
}

impl WorkspaceStatus {
    /// Project the reconciled status into the bounded working-set attention
    /// model. Pure over an already-reconciled [`WorkspaceStatus`]: it reads each
    /// observation's stored freshness and coordinates and touches neither the
    /// log nor the worktree, exactly like [`WorkspaceStatus::brief`].
    pub fn working_set_view(&self) -> WorkingSetView {
        self.working_set_view_with_uncited_candidates(None)
    }

    fn working_set_view_with_uncited_candidates(
        &self,
        uncited_candidate_ids: Option<&BTreeSet<u64>>,
    ) -> WorkingSetView {
        let observations: BTreeMap<u64, &Observation> =
            self.observations.iter().map(|o| (o.id, o)).collect();

        // Ranked, bounded locations: stale-first (a cap must never preferentially
        // hide invalidated attention), then most-recently focused.
        let mut located: Vec<SemanticLocation> = self
            .working_set
            .iter()
            .filter_map(|entry| {
                let observation = observations.get(&entry.observation_id)?;
                Some(SemanticLocation {
                    observation_id: entry.observation_id,
                    path: observation.path.clone(),
                    selector: observation.selector.clone(),
                    observed_revision: observation.observed_revision.clone(),
                    relocation_fingerprint: observation.observed_container_fingerprint.clone(),
                    freshness: observation.report.freshness_within_scope.clone(),
                    reason: entry.reason.clone(),
                    focus_sequence: entry.focus_sequence,
                })
            })
            .collect();
        located.sort_by_key(|location| {
            (
                freshness_rank(&location.freshness),
                Reverse(location.focus_sequence),
            )
        });
        let locations_omitted = located.len().saturating_sub(WORKING_SET_LOCATION_LIMIT);
        if located.len() > WORKING_SET_LOCATION_LIMIT {
            // Stale attention remains first, but it must not crowd the active
            // location out of the bounded view. Reserve the final slot for the
            // newest current focus when stale history fills the cap.
            let newest_current = located
                .iter()
                .filter(|location| location.freshness == FreshnessWithinScope::Current)
                .max_by_key(|location| location.focus_sequence)
                .cloned();
            located.truncate(WORKING_SET_LOCATION_LIMIT);
            if let Some(newest_current) = newest_current
                && !located
                    .iter()
                    .any(|location| location.observation_id == newest_current.observation_id)
            {
                located[WORKING_SET_LOCATION_LIMIT - 1] = newest_current;
            }
        }

        // Uncited candidates: current observations no active claim supports and
        // that are not already focused. Bounded orientation may supply a recent
        // candidate window; full audit projections consider every observation.
        let cited: BTreeSet<u64> = self
            .claims
            .iter()
            .flat_map(|claim| claim.supporting_observation_ids.iter().copied())
            .collect();
        let focused: BTreeSet<u64> = self.working_set.iter().map(|e| e.observation_id).collect();
        let is_uncited = |observation: &&Observation| {
            !cited.contains(&observation.id) && !focused.contains(&observation.id)
        };
        let uncited_candidate_count = self.observations.iter().filter(is_uncited).count();
        // Split candidates into the served window and everything outside it.
        // Outside-window observations are not reconciled in bounded orientation,
        // so their stored verdict is inherited and cannot be served (F9): they
        // count as omitted candidates. Inside-window stale observations are
        // known non-candidates and are excluded without counting as omitted.
        let (window_candidates, outside_window_count): (Vec<&Observation>, usize) =
            match uncited_candidate_ids {
                Some(candidates) => {
                    let in_window: Vec<&Observation> = self
                        .observations
                        .iter()
                        .filter(is_uncited)
                        .filter(|observation| candidates.contains(&observation.id))
                        .collect();
                    let outside = uncited_candidate_count - in_window.len();
                    (in_window, outside)
                }
                None => (self.observations.iter().filter(is_uncited).collect(), 0),
            };
        let mut uncited: Vec<UncitedObservation> = window_candidates
            .iter()
            .filter(|observation| {
                observation.report.freshness_within_scope == FreshnessWithinScope::Current
            })
            .map(|observation| UncitedObservation {
                observation_id: observation.id,
                path: observation.path.clone(),
                selector: observation.selector.clone(),
            })
            .collect();
        uncited.sort_by_key(|observation| Reverse(observation.observation_id));
        uncited.truncate(WORKING_SET_UNCITED_LIMIT);
        let served = uncited.len();
        let current_candidates = window_candidates
            .iter()
            .filter(|observation| {
                observation.report.freshness_within_scope == FreshnessWithinScope::Current
            })
            .count();
        let uncited_omitted = outside_window_count + current_candidates.saturating_sub(served);

        // Trail: most recent focus first, bounded to the recent window.
        let trail_omitted = self
            .navigation_trail
            .len()
            .saturating_sub(WORKING_SET_TRAIL_LIMIT);
        let trail: Vec<WorkingSetEntry> = self
            .navigation_trail
            .iter()
            .rev()
            .take(WORKING_SET_TRAIL_LIMIT)
            .cloned()
            .collect();

        WorkingSetView {
            locations: located,
            locations_omitted,
            uncited,
            uncited_omitted,
            trail,
            trail_omitted,
        }
    }
}

/// Order freshness so a truncating cap surfaces invalidated state first.
fn freshness_rank(freshness: &FreshnessWithinScope) -> u8 {
    match freshness {
        FreshnessWithinScope::Stale => 0,
        FreshnessWithinScope::Unknown => 1,
        FreshnessWithinScope::Current => 2,
    }
}

/// Hard cardinality bounds for the working-set attention surface. Like the brief
/// claim cap, these keep model-entry cost bounded; each is paired with an
/// explicit omission count so truncation is always visible.
const WORKING_SET_LOCATION_LIMIT: usize = 12;
const WORKING_SET_UNCITED_LIMIT: usize = 12;
const WORKING_SET_UNCITED_CANDIDATE_LIMIT: usize = 24;
const WORKING_SET_TRAIL_LIMIT: usize = 16;

/// One open finding as a scannable queue row: enough to triage (severity, rule,
/// a headline, where, and whether it is still current) without the native
/// payload, which stays one `reveal-finding` away.
#[derive(Clone, Debug, Serialize)]
pub struct FindingQueueEntry {
    pub id: u64,
    pub provider: String,
    pub severity: FindingSeverity,
    #[serde(default)]
    pub rule: Option<String>,
    pub headline: String,
    pub path: PathBuf,
    pub selector: ObservationSelector,
    pub freshness: FreshnessWithinScope,
}

/// The bounded quickfix-like queue: open findings ranked most-severe first and
/// hard-capped with an explicit omission count, a freshness histogram over the
/// open set, and the count of dispositioned findings that have left the queue
/// but remain in the audit record. Retains no native payload.
#[derive(Clone, Debug, Serialize)]
pub struct FindingsView {
    pub open: Vec<FindingQueueEntry>,
    pub open_omitted: usize,
    pub disposed: usize,
    pub freshness: FreshnessHistogram,
}

impl WorkspaceStatus {
    /// Project the reconciled status into the bounded findings queue. Pure over
    /// an already-reconciled [`WorkspaceStatus`] — reads each finding's stored
    /// severity, disposition, and freshness; touches neither log nor worktree.
    pub fn findings_view(&self) -> FindingsView {
        let mut open: Vec<&Finding> = self
            .findings
            .iter()
            .filter(|finding| finding.disposition.is_open())
            .collect();
        let disposed = self.findings.len() - open.len();
        let mut freshness = FreshnessHistogram::default();
        for finding in &open {
            match finding.report.freshness_within_scope {
                FreshnessWithinScope::Current => freshness.current += 1,
                FreshnessWithinScope::Stale => freshness.stale += 1,
                FreshnessWithinScope::Unknown => freshness.unknown += 1,
            }
        }
        // Most-severe first (a cap must never hide an error under a hint), then
        // stable by id. Freshness rides each row rather than the ranking, so a
        // severe issue is never demoted merely because an edit landed near it.
        open.sort_by_key(|finding| (finding.severity, finding.id));
        let open_omitted = open.len().saturating_sub(FINDING_QUEUE_LIMIT);
        let entries = open
            .into_iter()
            .take(FINDING_QUEUE_LIMIT)
            .map(|finding| FindingQueueEntry {
                id: finding.id,
                provider: finding.provider.clone(),
                severity: finding.severity,
                rule: finding.rule.clone(),
                headline: claim_headline(&finding.message, BRIEF_HEADLINE_MAX_CHARS),
                path: finding.path.clone(),
                selector: finding.selector.clone(),
                freshness: finding.report.freshness_within_scope.clone(),
            })
            .collect();
        FindingsView {
            open: entries,
            open_omitted,
            disposed,
            freshness,
        }
    }
}

const FINDING_QUEUE_LIMIT: usize = 12;

/// A finding as it appears in a transaction preview: enough to judge relevance,
/// with freshness so a stale association is visible.
#[derive(Clone, Debug, Serialize)]
pub struct PreviewFinding {
    pub id: u64,
    pub severity: FindingSeverity,
    pub freshness: FreshnessWithinScope,
    pub headline: String,
}

/// One evidence record bearing on a transaction's acceptance.
#[derive(Clone, Debug, Serialize)]
pub struct PreviewEvidence {
    pub id: u64,
    pub claim_id: u64,
    pub check_name: String,
    pub outcome: EvidenceOutcome,
    pub freshness: FreshnessWithinScope,
}

/// A review-before-accept surface for one transaction: its intent, the locations
/// it touches, the findings it addresses, the evidence and acceptance claims
/// bearing on it, the residual risks its author accepted, and whether it is
/// ready to accept right now. Pure projection over the reconciled status — the
/// advisory mirror of what `accept-transaction` will enforce.
#[derive(Clone, Debug, Serialize)]
pub struct TransactionPreview {
    pub id: u64,
    pub intent: Option<String>,
    pub state: TransactionState,
    pub base_revision: String,
    /// Distinct paths the transaction's mutations touch, in first-touch order.
    /// The honest, derivable "affected locations"; symbol-level blast radius is a
    /// deferred non-goal.
    pub affected_locations: Vec<PathBuf>,
    pub mutation_count: usize,
    pub acceptance_claim_ids: Vec<u64>,
    pub associated_findings: Vec<PreviewFinding>,
    pub evidence: Vec<PreviewEvidence>,
    pub residual_risks: Vec<String>,
    pub ready_to_accept: bool,
    pub readiness_reason: String,
}

impl WorkspaceStatus {
    /// Project a review surface for one transaction, or `None` if no such
    /// transaction exists. Pure over the already-reconciled status.
    pub fn transaction_preview(
        &self,
        transaction_id: u64,
        candidate_drift: Option<String>,
    ) -> Option<TransactionPreview> {
        let transaction = self
            .transactions
            .iter()
            .find(|transaction| transaction.id == transaction_id)?;

        let mut affected_locations: Vec<PathBuf> = Vec::new();
        for mutation in &transaction.mutations {
            if !affected_locations.contains(&mutation.path) {
                affected_locations.push(mutation.path.clone());
            }
        }

        let associated_findings = transaction
            .finding_ids
            .iter()
            .filter_map(|finding_id| self.findings.iter().find(|f| f.id == *finding_id))
            .map(|finding| PreviewFinding {
                id: finding.id,
                severity: finding.severity,
                freshness: finding.report.freshness_within_scope.clone(),
                headline: claim_headline(&finding.message, BRIEF_HEADLINE_MAX_CHARS),
            })
            .collect();

        let evidence: Vec<PreviewEvidence> = self
            .evidence
            .iter()
            .filter(|evidence| evidence.transaction_id == transaction_id)
            .map(|evidence| PreviewEvidence {
                id: evidence.id,
                claim_id: evidence.claim_id,
                check_name: evidence.check_name.clone(),
                outcome: evidence.outcome.clone(),
                freshness: evidence.report.freshness_within_scope.clone(),
            })
            .collect();

        // Disk-drift takes precedence over the pure rule while the transaction is
        // still open: bytes that no longer match what was applied are the most
        // fundamental blocker, and surfacing that reason is what keeps this
        // preview from ever promising a readiness `accept-transaction` would deny.
        let (ready_to_accept, readiness_reason) = match candidate_drift {
            Some(drift) if transaction.state == TransactionState::Open => (false, drift),
            _ => self.acceptance_readiness(transaction),
        };

        Some(TransactionPreview {
            id: transaction.id,
            intent: transaction.intent.clone(),
            state: transaction.state.clone(),
            base_revision: transaction.base_revision.clone(),
            affected_locations,
            mutation_count: transaction.mutations.len(),
            acceptance_claim_ids: transaction.acceptance_claim_ids.clone(),
            associated_findings,
            evidence,
            residual_risks: transaction.residual_risks.clone(),
            ready_to_accept,
            readiness_reason,
        })
    }

    /// Whether every acceptance claim is current and backed by current passing
    /// evidence — the same rule `accept-transaction` enforces, evaluated
    /// read-only here so a preview never claims readiness the accept would deny.
    fn acceptance_readiness(&self, transaction: &Transaction) -> (bool, String) {
        if transaction.state != TransactionState::Open {
            return (false, format!("transaction is {:?}", transaction.state));
        }
        let candidate = transaction.candidate_fingerprint();
        for claim_id in &transaction.acceptance_claim_ids {
            let Some(claim) = self.claims.iter().find(|claim| claim.id == *claim_id) else {
                return (false, format!("acceptance claim {claim_id} is not active"));
            };
            if claim.report.freshness_within_scope != FreshnessWithinScope::Current {
                return (false, format!("acceptance claim {claim_id} is not current"));
            }
            // The passing evidence must also be bound to *this* candidate: a check
            // recorded against an earlier candidate (before another path was
            // mutated) never proves the bytes now being committed. This is
            // orthogonal to the evidence's own input-freshness above.
            let has_bound_passing_evidence = self.evidence.iter().any(|evidence| {
                evidence.claim_id == *claim_id
                    && evidence.outcome == EvidenceOutcome::Passed
                    && evidence.report.freshness_within_scope == FreshnessWithinScope::Current
                    && evidence.candidate_fingerprint == candidate
            });
            if !has_bound_passing_evidence {
                return (
                    false,
                    format!(
                        "acceptance claim {claim_id} lacks current passing evidence bound to the current candidate"
                    ),
                );
            }
        }
        (
            true,
            "all acceptance claims current with passing evidence bound to the current candidate"
                .to_owned(),
        )
    }
}

/// Hard cardinality and per-headline bounds for model-entry orientation. Stale
/// claims rank first so a cap never preferentially hides invalidated beliefs.
const BRIEF_CLAIM_LIMIT: usize = 8;
const BRIEF_HEADLINE_MAX_CHARS: usize = 80;

/// Truncate a claim statement to a scannable headline on a word boundary,
/// marking the cut with a trailing `…`. Statements at or under the budget are
/// returned whole (no ellipsis). Char-aware, so a multibyte statement never
/// splits inside a code point.
fn claim_headline(statement: &str, max_chars: usize) -> String {
    if statement.chars().count() <= max_chars {
        return statement.to_owned();
    }
    let cutoff = statement
        .char_indices()
        .nth(max_chars)
        .map(|(index, _)| index)
        .unwrap_or(statement.len());
    let window = &statement[..cutoff];
    let head = match window.rfind(char::is_whitespace) {
        Some(boundary) => &window[..boundary],
        None => window,
    };
    format!("{}…", head.trim_end())
}

/// The objective in force shifted between the checkpoint and now.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ObjectiveChange {
    pub before: Option<Objective>,
    pub after: Option<Objective>,
}

/// What changed since a checkpoint. Every field is derived by projecting the log
/// twice — once up to the checkpoint's sequence, once to now — and diffing the two
/// states. It introduces no new freshness axis: `claims_staled` are claims whose
/// *recorded* freshness at the checkpoint was `current` and is now `stale`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DeltaStatus {
    pub checkpoint: CheckpointMarker,
    pub objective_change: Option<ObjectiveChange>,
    pub claims_recorded: Vec<Claim>,
    pub claims_superseded: Vec<Claim>,
    pub claims_staled: Vec<Claim>,
    pub observations_recorded: Vec<Observation>,
    pub transactions_opened: Vec<Transaction>,
    pub transactions_closed: Vec<Transaction>,
}

/// Bounded default delta. Full entities remain available through `delta --full`;
/// this projection answers only what kind of change occurred and which recent
/// entity ids to reveal next.
#[derive(Clone, Debug, Serialize)]
pub struct BriefDeltaStatus {
    pub checkpoint: BriefCheckpoint,
    pub objective_change: Option<BriefObjectiveChange>,
    pub claims_recorded: BriefIdSet,
    pub claims_superseded: BriefIdSet,
    pub claims_staled: BriefIdSet,
    pub observations_recorded: BriefIdSet,
    pub transactions_opened: BriefIdSet,
    pub transactions_closed: BriefIdSet,
}

#[derive(Clone, Debug, Serialize)]
pub struct BriefObjectiveChange {
    pub before: Option<String>,
    pub after: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BriefIdSet {
    pub total: usize,
    pub recent_ids: Vec<u64>,
    pub omitted: usize,
}

impl BriefIdSet {
    fn from_ids(ids: impl IntoIterator<Item = u64>) -> Self {
        let ids: Vec<u64> = ids.into_iter().collect();
        let omitted = ids.len().saturating_sub(BRIEF_DELTA_ID_LIMIT);
        Self {
            total: ids.len(),
            recent_ids: ids.into_iter().skip(omitted).collect(),
            omitted,
        }
    }
}

impl DeltaStatus {
    pub fn brief(&self) -> BriefDeltaStatus {
        BriefDeltaStatus {
            checkpoint: BriefCheckpoint {
                label: self.checkpoint.label.clone(),
                sequence: self.checkpoint.sequence,
            },
            objective_change: self
                .objective_change
                .as_ref()
                .map(|change| BriefObjectiveChange {
                    before: change.before.as_ref().map(|objective| {
                        claim_headline(&objective.intent, BRIEF_OBJECTIVE_MAX_CHARS)
                    }),
                    after: change.after.as_ref().map(|objective| {
                        claim_headline(&objective.intent, BRIEF_OBJECTIVE_MAX_CHARS)
                    }),
                }),
            claims_recorded: BriefIdSet::from_ids(self.claims_recorded.iter().map(|item| item.id)),
            claims_superseded: BriefIdSet::from_ids(
                self.claims_superseded.iter().map(|item| item.id),
            ),
            claims_staled: BriefIdSet::from_ids(self.claims_staled.iter().map(|item| item.id)),
            observations_recorded: BriefIdSet::from_ids(
                self.observations_recorded.iter().map(|item| item.id),
            ),
            transactions_opened: BriefIdSet::from_ids(
                self.transactions_opened.iter().map(|item| item.id),
            ),
            transactions_closed: BriefIdSet::from_ids(
                self.transactions_closed.iter().map(|item| item.id),
            ),
        }
    }
}

const BRIEF_DELTA_ID_LIMIT: usize = 16;
const BRIEF_OBJECTIVE_MAX_CHARS: usize = 120;

#[derive(Debug, Deserialize, Serialize)]
struct EventRecord {
    schema_version: u32,
    sequence: u64,
    event: Event,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Event {
    ObjectiveBound {
        intent: String,
        external_reference: Option<String>,
    },
    ObservationFocused {
        observation_id: u64,
        reason: String,
    },
    ObservationRecorded {
        observation_id: u64,
        path: PathBuf,
        provider: String,
        git_revision: String,
        #[serde(default)]
        selector: ObservationSelector,
        #[serde(default)]
        normalizer: Normalizer,
        input_fingerprint: String,
        #[serde(default)]
        raw_fingerprint: Option<String>,
        #[serde(default)]
        container_fingerprint: Option<String>,
        #[serde(default)]
        native_payload_reference: Option<String>,
        #[serde(default)]
        ingested_bytes: usize,
        #[serde(default)]
        model_visible_bytes: Option<usize>,
        #[serde(alias = "repository_fingerprint")]
        reconciliation_fingerprint: String,
    },
    ObservationReconciled {
        observation_id: u64,
        freshness: FreshnessWithinScope,
        reason: String,
        #[serde(alias = "repository_fingerprint")]
        reconciliation_fingerprint: String,
    },
    ClaimRecorded {
        claim_id: u64,
        statement: String,
        supporting_observation_ids: Vec<u64>,
        #[serde(default)]
        scope_strategy: ClaimScopeStrategy,
        inputs: Vec<ClaimInput>,
        freshness: FreshnessWithinScope,
        reason: String,
        reconciliation_fingerprint: String,
    },
    ClaimReconciled {
        claim_id: u64,
        freshness: FreshnessWithinScope,
        reason: String,
        reconciliation_fingerprint: String,
    },
    ClaimSuperseded {
        claim_id: u64,
        replacement_claim_id: u64,
        reason: String,
    },
    TransactionBegan {
        transaction_id: u64,
        #[serde(default)]
        intent: Option<String>,
        base_revision: String,
        initial_worktree_fingerprint: String,
        acceptance_claim_ids: Vec<u64>,
    },
    TransactionFindingAssociated {
        transaction_id: u64,
        finding_id: u64,
    },
    TransactionResidualRiskRecorded {
        transaction_id: u64,
        risk: String,
    },
    EvidenceRecorded {
        evidence_id: u64,
        transaction_id: u64,
        claim_id: u64,
        check_name: String,
        invocation: String,
        provider: String,
        outcome: EvidenceOutcome,
        inputs: Vec<ClaimInput>,
        #[serde(default)]
        candidate_fingerprint: String,
        freshness: FreshnessWithinScope,
        reason: String,
        reconciliation_fingerprint: String,
    },
    EvidenceReconciled {
        evidence_id: u64,
        freshness: FreshnessWithinScope,
        reason: String,
        reconciliation_fingerprint: String,
    },
    FindingRecorded {
        finding_id: u64,
        provider: String,
        severity: FindingSeverity,
        #[serde(default)]
        rule: Option<String>,
        message: String,
        path: PathBuf,
        #[serde(default)]
        selector: ObservationSelector,
        #[serde(default)]
        normalizer: Normalizer,
        git_revision: String,
        input_fingerprint: String,
        #[serde(default)]
        raw_fingerprint: Option<String>,
        container_fingerprint: String,
        #[serde(default)]
        native_payload_reference: Option<String>,
        #[serde(default)]
        native_payload_fingerprint: Option<String>,
        freshness: FreshnessWithinScope,
        reason: String,
        reconciliation_fingerprint: String,
    },
    FindingReconciled {
        finding_id: u64,
        freshness: FreshnessWithinScope,
        reason: String,
        reconciliation_fingerprint: String,
    },
    FindingDispositionChanged {
        finding_id: u64,
        disposition: FindingDisposition,
    },
    MutationApplied {
        transaction_id: u64,
        mutation: Mutation,
    },
    TransactionAccepted {
        transaction_id: u64,
    },
    TransactionReverted {
        transaction_id: u64,
    },
    TransactionAcceptanceRejected {
        transaction_id: u64,
        reason: String,
    },
    Checkpointed {
        label: String,
        #[serde(default)]
        note: Option<String>,
        git_revision: String,
    },
}

#[derive(Debug)]
pub struct Workspace {
    repository_root: PathBuf,
    workspace_root: PathBuf,
    /// Diagnostic: how many times this handle has replayed the event log from
    /// disk. It exists to make the single-pass `status` guarantee observable and
    /// testable — a settled `resume_status` must read the log O(1) times, never
    /// O(entities). See [`Workspace::event_log_reads`].
    event_log_reads: std::sync::atomic::AtomicUsize,
}

/// RAII guard for the workspace's exclusive inter-process write lock. The lock
/// is held for as long as this value lives and released the moment it is
/// dropped (or the owning process exits). See [`Workspace::lock_exclusive`].
pub struct WorkspaceLock {
    _file: File,
}

impl Workspace {
    pub fn open(
        repository_root: impl Into<PathBuf>,
        workspace_root: impl Into<PathBuf>,
    ) -> Result<Self, WorkspaceError> {
        let repository_root = repository_root.into().canonicalize()?;
        let workspace_root = workspace_root.into();
        fs::create_dir_all(&workspace_root)?;
        let workspace_root = workspace_root.canonicalize()?;
        Ok(Self {
            repository_root,
            workspace_root,
            event_log_reads: std::sync::atomic::AtomicUsize::new(0),
        })
    }

    /// How many times this handle has replayed the event log from disk since it
    /// was opened. A diagnostic for the single-pass `status` invariant: a
    /// settled `resume_status` reads the log a small constant number of times,
    /// independent of how many observations/claims/evidence the workspace holds.
    pub fn event_log_reads(&self) -> usize {
        self.event_log_reads
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn bind_objective(
        &self,
        intent: impl Into<String>,
        external_reference: Option<String>,
    ) -> Result<Objective, WorkspaceError> {
        let intent = intent.into();
        if intent.trim().is_empty() {
            return Err(WorkspaceError::InvalidObjective(
                "intent must not be empty".to_owned(),
            ));
        }
        self.append(Event::ObjectiveBound {
            intent,
            external_reference,
        })?;
        self.project()?.objective.ok_or_else(|| {
            WorkspaceError::CorruptLog("objective event was not projected".to_owned())
        })
    }

    pub fn focus_observation(
        &self,
        observation_id: u64,
        reason: impl Into<String>,
    ) -> Result<WorkingSetEntry, WorkspaceError> {
        let reason = reason.into();
        if reason.trim().is_empty() {
            return Err(WorkspaceError::InvalidWorkingSet(
                "focus reason must not be empty".to_owned(),
            ));
        }
        if !self.project()?.observations.contains_key(&observation_id) {
            return Err(WorkspaceError::ObservationNotFound(observation_id));
        }
        self.append(Event::ObservationFocused {
            observation_id,
            reason,
        })?;
        self.project()?
            .working_set
            .remove(&observation_id)
            .ok_or_else(|| WorkspaceError::CorruptLog("focus event was not projected".to_owned()))
    }

    pub fn resume_status(&self) -> Result<WorkspaceStatus, WorkspaceError> {
        // The full audit path deliberately reconciles every entity. Bounded
        // orientation methods below reconcile only the verdicts they serve.
        let projection = self.project()?;
        let mut pending = Vec::new();
        for observation in projection.observations.values() {
            pending.extend(self.observation_reconcile_event(observation)?);
        }
        for claim in projection.claims.values() {
            pending.extend(self.claim_reconcile_event(claim)?);
        }
        for evidence in projection.evidence.values() {
            pending.extend(self.evidence_reconcile_event(evidence)?);
        }
        for finding in projection.findings.values() {
            pending.extend(self.finding_reconcile_event(finding)?);
        }
        let projection = self.apply_reconciliations(projection, pending)?;
        Ok(Self::status_from_projection(projection))
    }

    /// Reconcile only active claims because those are the only freshness
    /// verdicts the bounded status serves. Counts and lifecycle state are pure
    /// replay facts. Full audit status remains exhaustive via [`resume_status`].
    pub fn resume_brief_status(&self) -> Result<BriefStatus, WorkspaceError> {
        let projection = self.project()?;
        let mut pending = Vec::new();
        for claim in projection
            .claims
            .values()
            .filter(|claim| claim.lifecycle.is_active())
        {
            pending.extend(self.claim_reconcile_event(claim)?);
        }
        let projection = self.apply_reconciliations(projection, pending)?;
        Ok(Self::status_from_projection(projection).brief())
    }

    /// Reconcile focused observations plus a bounded recent uncited-candidate
    /// window. The resulting view never serves an inherited freshness verdict:
    /// observations outside that bounded window are counted as omitted.
    pub fn resume_working_set_view(&self) -> Result<WorkingSetView, WorkspaceError> {
        let projection = self.project()?;
        let focused: BTreeSet<u64> = projection.working_set.keys().copied().collect();
        let cited: BTreeSet<u64> = projection
            .claims
            .values()
            .filter(|claim| claim.lifecycle.is_active())
            .flat_map(|claim| claim.supporting_observation_ids.iter().copied())
            .collect();
        let uncited_candidates: BTreeSet<u64> = projection
            .observations
            .keys()
            .rev()
            .filter(|id| !focused.contains(id) && !cited.contains(id))
            .take(WORKING_SET_UNCITED_CANDIDATE_LIMIT)
            .copied()
            .collect();
        let reconcile_ids: BTreeSet<u64> = focused.union(&uncited_candidates).copied().collect();
        let mut pending = Vec::new();
        for observation_id in reconcile_ids {
            if let Some(observation) = projection.observations.get(&observation_id) {
                pending.extend(self.observation_reconcile_event(observation)?);
            }
        }
        let projection = self.apply_reconciliations(projection, pending)?;
        Ok(Self::status_from_projection(projection)
            .working_set_view_with_uncited_candidates(Some(&uncited_candidates)))
    }

    /// Reconcile only open findings: disposed findings are audit history and do
    /// not appear in the quickfix-like queue.
    pub fn resume_findings_view(&self) -> Result<FindingsView, WorkspaceError> {
        let projection = self.project()?;
        let mut pending = Vec::new();
        for finding in projection
            .findings
            .values()
            .filter(|finding| finding.disposition.is_open())
        {
            pending.extend(self.finding_reconcile_event(finding)?);
        }
        let projection = self.apply_reconciliations(projection, pending)?;
        Ok(Self::status_from_projection(projection).findings_view())
    }

    /// Reconcile exactly the claims, evidence, and associated findings exposed
    /// by one transaction preview.
    pub fn resume_transaction_preview(
        &self,
        transaction_id: u64,
    ) -> Result<Option<TransactionPreview>, WorkspaceError> {
        let projection = self.project()?;
        let Some(transaction) = projection.transactions.get(&transaction_id) else {
            return Ok(None);
        };
        let claim_ids = transaction.acceptance_claim_ids.clone();
        let evidence_ids = transaction.evidence_ids.clone();
        let finding_ids = transaction.finding_ids.clone();
        let mut pending = Vec::new();
        for claim_id in claim_ids {
            if let Some(claim) = projection.claims.get(&claim_id) {
                pending.extend(self.claim_reconcile_event(claim)?);
            }
        }
        for evidence_id in evidence_ids {
            if let Some(evidence) = projection.evidence.get(&evidence_id) {
                pending.extend(self.evidence_reconcile_event(evidence)?);
            }
        }
        for finding_id in finding_ids {
            if let Some(finding) = projection.findings.get(&finding_id) {
                pending.extend(self.finding_reconcile_event(finding)?);
            }
        }
        let projection = self.apply_reconciliations(projection, pending)?;
        let status = Self::status_from_projection(projection);
        let drift = status
            .transactions
            .iter()
            .find(|transaction| transaction.id == transaction_id)
            .and_then(|transaction| self.candidate_drift(transaction));
        Ok(status.transaction_preview(transaction_id, drift))
    }

    fn apply_reconciliations(
        &self,
        projection: Projection,
        pending: Vec<Event>,
    ) -> Result<Projection, WorkspaceError> {
        if pending.is_empty() {
            return Ok(projection);
        }
        for event in pending {
            self.append(event)?;
        }
        self.project()
    }

    fn status_from_projection(projection: Projection) -> WorkspaceStatus {
        let (claims, superseded_claims) = projection
            .claims
            .into_values()
            .partition(|claim| claim.lifecycle.is_active());
        WorkspaceStatus {
            objective: projection.objective,
            working_set: projection.working_set.into_values().collect(),
            navigation_trail: projection.navigation_trail,
            observations: projection.observations.into_values().collect(),
            claims,
            superseded_claims,
            evidence: projection.evidence.into_values().collect(),
            findings: projection.findings.into_values().collect(),
            transactions: projection.transactions.into_values().collect(),
            checkpoints: projection.checkpoints,
        }
    }

    /// Recompute an observation's freshness verdict from the live worktree and
    /// return the `ObservationReconciled` event that would record it — or `None`
    /// when the verdict is unchanged (no-op suppression). Reads the observed
    /// file; does not touch the log. Shared by `reconcile_observation` (single
    /// entity) and `resume_status` (single-pass over all).
    fn observation_reconcile_event(
        &self,
        observation: &Observation,
    ) -> Result<Option<Event>, WorkspaceError> {
        let (freshness, reason, reconciliation_fingerprint) = location_freshness_verdict(
            &self.repository_root,
            &observation.path,
            &observation.selector,
            observation.normalizer,
            observation.observed_raw_fingerprint.as_deref(),
            &observation.observed_input_fingerprint,
            &observation.observed_container_fingerprint,
        )?;
        if verdict_unchanged(
            &observation.report,
            &freshness,
            &reason,
            &reconciliation_fingerprint,
        ) {
            return Ok(None);
        }
        Ok(Some(Event::ObservationReconciled {
            observation_id: observation.id,
            freshness,
            reason,
            reconciliation_fingerprint,
        }))
    }

    /// Recompute a finding's freshness from its bound location and return the
    /// `FindingReconciled` event, or `None` when unchanged. A finding is bound to
    /// a single location, so it reconciles through the exact same verdict a
    /// same-location observation would — an edit under the finding stales it.
    fn finding_reconcile_event(&self, finding: &Finding) -> Result<Option<Event>, WorkspaceError> {
        let (freshness, reason, reconciliation_fingerprint) = location_freshness_verdict(
            &self.repository_root,
            &finding.path,
            &finding.selector,
            finding.normalizer,
            finding.observed_raw_fingerprint.as_deref(),
            &finding.observed_input_fingerprint,
            &finding.observed_container_fingerprint,
        )?;
        if verdict_unchanged(
            &finding.report,
            &freshness,
            &reason,
            &reconciliation_fingerprint,
        ) {
            return Ok(None);
        }
        Ok(Some(Event::FindingReconciled {
            finding_id: finding.id,
            freshness,
            reason,
            reconciliation_fingerprint,
        }))
    }

    /// Recompute a claim's verdict and return the `ClaimReconciled` event, or
    /// `None` when unchanged. Reads the claim's inputs; does not touch the log.
    fn claim_reconcile_event(&self, claim: &Claim) -> Result<Option<Event>, WorkspaceError> {
        let (freshness, reason, fingerprint_inputs) =
            assess_claim_inputs(&self.repository_root, &claim.inputs);
        let reconciliation_fingerprint =
            scoped_reconciliation_fingerprint(&self.repository_root, &fingerprint_inputs)?;
        if verdict_unchanged(
            &claim.report,
            &freshness,
            &reason,
            &reconciliation_fingerprint,
        ) {
            return Ok(None);
        }
        Ok(Some(Event::ClaimReconciled {
            claim_id: claim.id,
            freshness,
            reason,
            reconciliation_fingerprint,
        }))
    }

    /// Recompute an evidence record's verdict and return the
    /// `EvidenceReconciled` event, or `None` when unchanged. Reads the
    /// evidence's inputs; does not touch the log.
    fn evidence_reconcile_event(
        &self,
        evidence: &Evidence,
    ) -> Result<Option<Event>, WorkspaceError> {
        let (freshness, reason, fingerprint_inputs) =
            assess_claim_inputs(&self.repository_root, &evidence.inputs);
        let reconciliation_fingerprint =
            scoped_reconciliation_fingerprint(&self.repository_root, &fingerprint_inputs)?;
        if verdict_unchanged(
            &evidence.report,
            &freshness,
            &reason,
            &reconciliation_fingerprint,
        ) {
            return Ok(None);
        }
        Ok(Some(Event::EvidenceReconciled {
            evidence_id: evidence.id,
            freshness,
            reason,
            reconciliation_fingerprint,
        }))
    }

    /// Draw a named line in the log. Reconciles everything to current truth first
    /// so the checkpoint anchors an accurate freshness baseline, then records the
    /// marker. Labels must be unique so `delta_since` can resolve them.
    pub fn checkpoint(
        &self,
        label: impl Into<String>,
        note: Option<String>,
    ) -> Result<CheckpointMarker, WorkspaceError> {
        let label = label.into();
        if label.trim().is_empty() {
            return Err(WorkspaceError::InvalidCheckpoint(
                "label must not be empty".to_owned(),
            ));
        }
        if self
            .project()?
            .checkpoints
            .iter()
            .any(|marker| marker.label == label)
        {
            return Err(WorkspaceError::InvalidCheckpoint(format!(
                "label {label:?} is already used"
            )));
        }

        // Bring recorded freshness up to date so the baseline this checkpoint
        // anchors reflects the world as it is now, not as it was last reconciled.
        self.resume_status()?;
        let git_revision = git_output(&self.repository_root, &["rev-parse", "HEAD"])?;
        self.append(Event::Checkpointed {
            label: label.clone(),
            note,
            git_revision,
        })?;

        self.project()?
            .checkpoints
            .into_iter()
            .find(|marker| marker.label == label)
            .ok_or(WorkspaceError::CheckpointNotFound(label))
    }

    /// Project what changed since a checkpoint. With `label`, diffs against that
    /// checkpoint; without one, diffs against the most recent checkpoint — the
    /// ergonomic cold-resume default of "what changed since I last drew a line."
    /// Bounded resume delta. Only active claims have freshness in this surface,
    /// so only those claims are reconciled; IDs, lifecycle transitions, and
    /// counts are derived directly from replayed events.
    pub fn delta_brief_since(
        &self,
        label: Option<&str>,
    ) -> Result<BriefDeltaStatus, WorkspaceError> {
        let current = self.project()?;
        let mut pending = Vec::new();
        for claim in current
            .claims
            .values()
            .filter(|claim| claim.lifecycle.is_active())
        {
            pending.extend(self.claim_reconcile_event(claim)?);
        }
        let current = self.apply_reconciliations(current, pending)?;
        let checkpoint = match label {
            Some(label) => current
                .checkpoints
                .iter()
                .find(|marker| marker.label == label)
                .cloned()
                .ok_or_else(|| WorkspaceError::CheckpointNotFound(label.to_owned()))?,
            None => current
                .checkpoints
                .last()
                .cloned()
                .ok_or_else(|| WorkspaceError::CheckpointNotFound("<latest>".to_owned()))?,
        };
        let baseline = self.project_upto(Some(checkpoint.sequence))?;

        let objective_change =
            (baseline.objective != current.objective).then(|| BriefObjectiveChange {
                before: baseline
                    .objective
                    .as_ref()
                    .map(|objective| claim_headline(&objective.intent, BRIEF_OBJECTIVE_MAX_CHARS)),
                after: current
                    .objective
                    .as_ref()
                    .map(|objective| claim_headline(&objective.intent, BRIEF_OBJECTIVE_MAX_CHARS)),
            });
        let active_claims = current
            .claims
            .values()
            .filter(|claim| claim.lifecycle.is_active());
        let claims_recorded = BriefIdSet::from_ids(
            active_claims
                .clone()
                .filter(|claim| !baseline.claims.contains_key(&claim.id))
                .map(|claim| claim.id),
        );
        let claims_staled = BriefIdSet::from_ids(
            active_claims
                .filter(|claim| {
                    baseline.claims.get(&claim.id).is_some_and(|before| {
                        before.lifecycle.is_active()
                            && before.report.freshness_within_scope == FreshnessWithinScope::Current
                            && claim.report.freshness_within_scope == FreshnessWithinScope::Stale
                    })
                })
                .map(|claim| claim.id),
        );
        let claims_superseded = BriefIdSet::from_ids(
            current
                .claims
                .values()
                .filter(|claim| !claim.lifecycle.is_active())
                .filter(|claim| {
                    baseline
                        .claims
                        .get(&claim.id)
                        .is_some_and(|before| before.lifecycle.is_active())
                })
                .map(|claim| claim.id),
        );
        let observations_recorded = BriefIdSet::from_ids(
            current
                .observations
                .keys()
                .filter(|id| !baseline.observations.contains_key(id))
                .copied(),
        );
        let mut transactions_opened = Vec::new();
        let mut transactions_closed = Vec::new();
        for transaction in current.transactions.values() {
            match baseline.transactions.get(&transaction.id) {
                None => transactions_opened.push(transaction.id),
                Some(before)
                    if before.state == TransactionState::Open
                        && transaction.state != TransactionState::Open =>
                {
                    transactions_closed.push(transaction.id);
                }
                Some(_) => {}
            }
        }

        Ok(BriefDeltaStatus {
            checkpoint: BriefCheckpoint {
                label: checkpoint.label,
                sequence: checkpoint.sequence,
            },
            objective_change,
            claims_recorded,
            claims_superseded,
            claims_staled,
            observations_recorded,
            transactions_opened: BriefIdSet::from_ids(transactions_opened),
            transactions_closed: BriefIdSet::from_ids(transactions_closed),
        })
    }

    pub fn delta_since(&self, label: Option<&str>) -> Result<DeltaStatus, WorkspaceError> {
        let checkpoints = self.project()?.checkpoints;
        let checkpoint = match label {
            Some(label) => checkpoints
                .into_iter()
                .find(|marker| marker.label == label)
                .ok_or_else(|| WorkspaceError::CheckpointNotFound(label.to_owned()))?,
            None => checkpoints
                .into_iter()
                .next_back()
                .ok_or_else(|| WorkspaceError::CheckpointNotFound("<latest>".to_owned()))?,
        };

        // Baseline is a pure read of the log up to the checkpoint (no
        // reconciliation); current reconciles against the live worktree. Diffing
        // the two is what makes "staled since" honest without a new axis.
        let baseline = self.project_upto(Some(checkpoint.sequence))?;
        let current = self.resume_status()?;

        let objective_change = if baseline.objective != current.objective {
            Some(ObjectiveChange {
                before: baseline.objective.clone(),
                after: current.objective.clone(),
            })
        } else {
            None
        };

        let mut claims_recorded = Vec::new();
        let mut claims_staled = Vec::new();
        for claim in &current.claims {
            match baseline.claims.get(&claim.id) {
                None => claims_recorded.push(claim.clone()),
                Some(before) => {
                    if before.lifecycle.is_active()
                        && before.report.freshness_within_scope == FreshnessWithinScope::Current
                        && claim.report.freshness_within_scope == FreshnessWithinScope::Stale
                    {
                        claims_staled.push(claim.clone());
                    }
                }
            }
        }

        let claims_superseded = current
            .superseded_claims
            .iter()
            .filter(|claim| {
                baseline
                    .claims
                    .get(&claim.id)
                    .is_some_and(|before| before.lifecycle.is_active())
            })
            .cloned()
            .collect();

        let observations_recorded = current
            .observations
            .iter()
            .filter(|observation| !baseline.observations.contains_key(&observation.id))
            .cloned()
            .collect();

        let mut transactions_opened = Vec::new();
        let mut transactions_closed = Vec::new();
        for transaction in &current.transactions {
            match baseline.transactions.get(&transaction.id) {
                None => transactions_opened.push(transaction.clone()),
                Some(before) => {
                    if before.state == TransactionState::Open
                        && transaction.state != TransactionState::Open
                    {
                        transactions_closed.push(transaction.clone());
                    }
                }
            }
        }

        Ok(DeltaStatus {
            checkpoint,
            objective_change,
            claims_recorded,
            claims_superseded,
            claims_staled,
            observations_recorded,
            transactions_opened,
            transactions_closed,
        })
    }

    pub fn capture_file_observation(
        &self,
        path: impl AsRef<Path>,
        provider: impl Into<String>,
        options: ObservationCaptureOptions,
    ) -> Result<ObservationCapture, WorkspaceError> {
        let ObservationCaptureOptions {
            selector,
            normalizer,
            retain_native_payload,
            model_visible_bytes,
            expected_raw_fingerprint,
        } = options;
        let path = validate_relative_path(path.as_ref())?;
        let resolved_path = resolve_repository_file(&self.repository_root, &path)?;
        let container = fs::read(resolved_path)?;
        let unit = select_observation_unit(&container, &selector)?;
        if let Some(expected) = expected_raw_fingerprint.as_deref() {
            if !is_sha256_hex(expected) {
                return Err(WorkspaceError::InvalidObservation(
                    "expected raw fingerprint must be a lowercase SHA-256 hex digest".to_owned(),
                ));
            }
            if hex_digest(unit) != expected {
                return Err(WorkspaceError::InvalidObservation(
                    "selected input changed after the provider result was finalized".to_owned(),
                ));
            }
        }
        let content = String::from_utf8(unit.to_vec()).map_err(|_| {
            WorkspaceError::InvalidObservation("selected source is not valid UTF-8".to_owned())
        })?;
        let input_fingerprint = hex_digest(&normalize_unit(unit, normalizer));
        // Record the raw unit fingerprint whenever the normalizer makes it
        // distinct in meaning from the input fingerprint, so reconcile can
        // skip the formatter subprocess while the bytes are unchanged.
        let raw_fingerprint = (normalizer != Normalizer::None).then(|| hex_digest(unit));
        let container_fingerprint = hex_digest(&container);
        let native_payload_reference = if retain_native_payload {
            if container.len() > MAX_RETAINED_PAYLOAD_BYTES {
                return Err(WorkspaceError::InvalidObservation(format!(
                    "{}-byte payload exceeds the {}-byte retention limit",
                    container.len(),
                    MAX_RETAINED_PAYLOAD_BYTES
                )));
            }
            Some(self.persist_native_payload(&container_fingerprint, &container)?)
        } else {
            None
        };
        let git_revision = git_output(&self.repository_root, &["rev-parse", "HEAD"])?;
        let reconciliation_fingerprint = observation_reconciliation_fingerprint(
            &self.repository_root,
            &path,
            &selector,
            Some(&input_fingerprint),
            Some(&container_fingerprint),
        )?;
        let projection = self.project()?;
        let observation_id = projection.next_observation_id;
        let ingested_bytes = unit.len();

        self.append(Event::ObservationRecorded {
            observation_id,
            path,
            provider: provider.into(),
            git_revision,
            selector,
            normalizer,
            input_fingerprint,
            raw_fingerprint,
            container_fingerprint: Some(container_fingerprint),
            native_payload_reference,
            ingested_bytes,
            model_visible_bytes,
            reconciliation_fingerprint,
        })?;

        let observation = self
            .project()?
            .observations
            .remove(&observation_id)
            .ok_or(WorkspaceError::ObservationNotFound(observation_id))?;
        Ok(ObservationCapture {
            observation,
            content,
        })
    }

    /// Plan and record an observation from a coding agent's `read` tool result.
    ///
    /// This is the kernel-owned counterpart to what adapters used to compute in
    /// their own language: it maps the read's one-indexed line window onto a
    /// UTF-8 byte range, verifies the model-visible text still matches the file
    /// (fail-closed on drift), rejects truncated, binary, and sensitive reads,
    /// and then delegates to [`capture_file_observation`] so fingerprinting and
    /// persistence happen exactly once. Adapters forward raw read inputs and
    /// stay thin transports; every capture decision lives here.
    pub fn capture_read_observation(
        &self,
        path: impl AsRef<Path>,
        provider: impl Into<String>,
        request: ReadCaptureRequest,
    ) -> Result<ReadCaptureOutcome, WorkspaceError> {
        let path = validate_relative_path(path.as_ref())?;

        if request.truncated {
            return Ok(skip("native read result was byte/line truncated"));
        }
        if matches!(request.offset, Some(0)) {
            return Ok(skip("read offset is not a positive integer"));
        }
        if matches!(request.limit, Some(0)) {
            return Ok(skip("read limit is not a positive integer"));
        }
        if is_sensitive_repository_path(&path) {
            return Ok(skip("path matches a sensitive-file pattern"));
        }

        let resolved_path = resolve_repository_file(&self.repository_root, &path)?;
        let container = fs::read(resolved_path)?;
        let file_text = match std::str::from_utf8(&container) {
            Ok(text) => text,
            Err(_) => return Ok(skip("file is not valid UTF-8")),
        };

        let plan = match plan_read_selection(
            file_text,
            request.offset,
            request.limit,
            &request.model_visible_text,
        ) {
            Ok(plan) => plan,
            Err(reason) => return Ok(skip(reason)),
        };

        let options = ObservationCaptureOptions {
            selector: plan.selector,
            normalizer: Normalizer::detect_for_path(&path),
            retain_native_payload: false,
            model_visible_bytes: Some(
                request
                    .model_visible_bytes
                    .unwrap_or(request.model_visible_text.len()),
            ),
            expected_raw_fingerprint: Some(plan.expected_raw_fingerprint),
        };
        let capture = self.capture_file_observation(path, provider, options)?;
        Ok(ReadCaptureOutcome::Captured(Box::new(capture)))
    }

    pub fn reveal_observation(
        &self,
        observation_id: u64,
    ) -> Result<RevealedObservation, WorkspaceError> {
        let projection = self.project()?;
        let observation = projection
            .observations
            .get(&observation_id)
            .ok_or(WorkspaceError::ObservationNotFound(observation_id))?;
        let payload_reference =
            observation
                .native_payload_reference
                .as_deref()
                .ok_or_else(|| {
                    WorkspaceError::InvalidObservation(
                        "native payload was not retained for this legacy observation".to_owned(),
                    )
                })?;
        if !is_sha256_hex(&observation.observed_container_fingerprint) {
            return Err(WorkspaceError::CorruptLog(format!(
                "invalid container fingerprint for observation {observation_id}"
            )));
        }
        let expected_reference =
            PathBuf::from("payloads").join(&observation.observed_container_fingerprint);
        if Path::new(payload_reference) != expected_reference {
            return Err(WorkspaceError::CorruptLog(format!(
                "invalid native payload reference for observation {observation_id}"
            )));
        }
        let absolute_payload = self.workspace_root.join(expected_reference);
        if fs::symlink_metadata(&absolute_payload)?
            .file_type()
            .is_symlink()
        {
            return Err(WorkspaceError::CorruptLog(format!(
                "native payload is a symlink for observation {observation_id}"
            )));
        }
        let payload = fs::read(absolute_payload)?;
        if hex_digest(&payload) != observation.observed_container_fingerprint {
            return Err(WorkspaceError::CorruptLog(format!(
                "native payload fingerprint mismatch for observation {observation_id}"
            )));
        }
        let content = String::from_utf8(payload).map_err(|_| {
            WorkspaceError::InvalidObservation("retained source is not valid UTF-8".to_owned())
        })?;
        Ok(RevealedObservation {
            observation_id,
            path: observation.path.clone(),
            provider: observation.provider.clone(),
            observed_revision: observation.observed_revision.clone(),
            observed_container_fingerprint: observation.observed_container_fingerprint.clone(),
            ingested_bytes: content.len(),
            content,
        })
    }

    /// Record a provider-reported finding bound to a repository location. The
    /// file at `path` is read to fingerprint the location for freshness (an edit
    /// under the finding will later stale it); the provider's own raw output, if
    /// supplied, is retained in the CAS so the original result stays retrievable
    /// (S8). The finding enters the queue `Open` and `current`.
    pub fn record_finding(
        &self,
        provider: impl Into<String>,
        severity: FindingSeverity,
        rule: Option<String>,
        message: impl Into<String>,
        path: impl AsRef<Path>,
        options: FindingCaptureOptions,
    ) -> Result<Finding, WorkspaceError> {
        let message = message.into();
        if message.trim().is_empty() {
            return Err(WorkspaceError::InvalidFinding(
                "finding message must not be empty".to_owned(),
            ));
        }
        let FindingCaptureOptions {
            selector,
            normalizer,
            native_payload,
        } = options;
        let path = validate_relative_path(path.as_ref())?;
        let resolved_path = resolve_repository_file(&self.repository_root, &path)?;
        let container = fs::read(resolved_path)?;
        let unit = select_observation_unit(&container, &selector)?;
        let input_fingerprint = hex_digest(&normalize_unit(unit, normalizer));
        let raw_fingerprint = (normalizer != Normalizer::None).then(|| hex_digest(unit));
        let container_fingerprint = hex_digest(&container);

        // Retain the provider's raw output — not the source file — keyed by its
        // own digest, so provenance survives independently of the location.
        let (native_payload_reference, native_payload_fingerprint) = match &native_payload {
            Some(payload) => {
                let bytes = payload.as_bytes();
                if bytes.len() > MAX_RETAINED_PAYLOAD_BYTES {
                    return Err(WorkspaceError::InvalidFinding(format!(
                        "{}-byte native payload exceeds the {}-byte retention limit",
                        bytes.len(),
                        MAX_RETAINED_PAYLOAD_BYTES
                    )));
                }
                let fingerprint = hex_digest(bytes);
                let reference = self.persist_native_payload(&fingerprint, bytes)?;
                (Some(reference), Some(fingerprint))
            }
            None => (None, None),
        };

        let git_revision = git_output(&self.repository_root, &["rev-parse", "HEAD"])?;
        let reconciliation_fingerprint = observation_reconciliation_fingerprint(
            &self.repository_root,
            &path,
            &selector,
            Some(&input_fingerprint),
            Some(&container_fingerprint),
        )?;
        let finding_id = self.project()?.next_finding_id;
        self.append(Event::FindingRecorded {
            finding_id,
            provider: provider.into(),
            severity,
            rule,
            message,
            path,
            selector,
            normalizer,
            git_revision,
            input_fingerprint,
            raw_fingerprint,
            container_fingerprint,
            native_payload_reference,
            native_payload_fingerprint,
            freshness: FreshnessWithinScope::Current,
            reason: "finding recorded".to_owned(),
            reconciliation_fingerprint,
        })?;
        self.project()?
            .findings
            .remove(&finding_id)
            .ok_or(WorkspaceError::FindingNotFound(finding_id))
    }

    /// Retrieve a finding's retained native payload, verifying it against its
    /// stored digest (S8). Fails closed on a missing payload or any CAS
    /// tampering rather than returning unverified bytes.
    pub fn reveal_finding(&self, finding_id: u64) -> Result<RevealedFinding, WorkspaceError> {
        let projection = self.project()?;
        let finding = projection
            .findings
            .get(&finding_id)
            .ok_or(WorkspaceError::FindingNotFound(finding_id))?;
        let fingerprint = finding
            .native_payload_fingerprint
            .as_deref()
            .ok_or_else(|| {
                WorkspaceError::InvalidFinding(
                    "no native payload was retained for this finding".to_owned(),
                )
            })?;
        let reference = finding.native_payload_reference.as_deref().ok_or_else(|| {
            WorkspaceError::InvalidFinding(
                "no native payload was retained for this finding".to_owned(),
            )
        })?;
        if !is_sha256_hex(fingerprint) {
            return Err(WorkspaceError::CorruptLog(format!(
                "invalid native payload fingerprint for finding {finding_id}"
            )));
        }
        let expected_reference = PathBuf::from("payloads").join(fingerprint);
        if Path::new(reference) != expected_reference {
            return Err(WorkspaceError::CorruptLog(format!(
                "invalid native payload reference for finding {finding_id}"
            )));
        }
        let absolute_payload = self.workspace_root.join(expected_reference);
        if fs::symlink_metadata(&absolute_payload)?
            .file_type()
            .is_symlink()
        {
            return Err(WorkspaceError::CorruptLog(format!(
                "native payload is a symlink for finding {finding_id}"
            )));
        }
        let payload = fs::read(absolute_payload)?;
        if hex_digest(&payload) != fingerprint {
            return Err(WorkspaceError::CorruptLog(format!(
                "native payload fingerprint mismatch for finding {finding_id}"
            )));
        }
        let content = String::from_utf8(payload).map_err(|_| {
            WorkspaceError::InvalidFinding("retained native payload is not valid UTF-8".to_owned())
        })?;
        Ok(RevealedFinding {
            finding_id,
            provider: finding.provider.clone(),
            path: finding.path.clone(),
            observed_revision: finding.observed_revision.clone(),
            native_payload_fingerprint: fingerprint.to_owned(),
            content,
        })
    }

    pub fn reconcile_finding(&self, finding_id: u64) -> Result<Finding, WorkspaceError> {
        let projection = self.project()?;
        let finding = projection
            .findings
            .get(&finding_id)
            .ok_or(WorkspaceError::FindingNotFound(finding_id))?;
        match self.finding_reconcile_event(finding)? {
            None => Ok(finding.clone()),
            Some(event) => {
                self.append(event)?;
                self.project()?
                    .findings
                    .remove(&finding_id)
                    .ok_or(WorkspaceError::FindingNotFound(finding_id))
            }
        }
    }

    /// Disposition a finding — resolve, defer, suppress, or mark it a false
    /// positive. Every disposition names its actor and rationale (invariant 8),
    /// and is orthogonal to freshness: disposing a finding never touches whether
    /// its bound input is still current, and a stale finding can still be open.
    pub fn dispose_finding(
        &self,
        finding_id: u64,
        disposition: FindingDisposition,
    ) -> Result<Finding, WorkspaceError> {
        let (actor, rationale) = match &disposition {
            FindingDisposition::Open => {
                return Err(WorkspaceError::InvalidFinding(
                    "dispose-finding cannot set a finding back to open".to_owned(),
                ));
            }
            FindingDisposition::Resolved { actor, rationale }
            | FindingDisposition::Deferred { actor, rationale }
            | FindingDisposition::Suppressed { actor, rationale }
            | FindingDisposition::FalsePositive { actor, rationale } => (actor, rationale),
        };
        if actor.trim().is_empty() || rationale.trim().is_empty() {
            return Err(WorkspaceError::InvalidFinding(
                "finding disposition requires a non-empty actor and rationale".to_owned(),
            ));
        }
        if !self.project()?.findings.contains_key(&finding_id) {
            return Err(WorkspaceError::FindingNotFound(finding_id));
        }
        self.append(Event::FindingDispositionChanged {
            finding_id,
            disposition,
        })?;
        self.project()?
            .findings
            .remove(&finding_id)
            .ok_or(WorkspaceError::FindingNotFound(finding_id))
    }

    pub fn reconcile_observation(
        &self,
        observation_id: u64,
    ) -> Result<Observation, WorkspaceError> {
        let projection = self.project()?;
        let observation = projection
            .observations
            .get(&observation_id)
            .ok_or(WorkspaceError::ObservationNotFound(observation_id))?;
        // No-op suppression lives in the shared helper: it recomputes the
        // verdict from current inputs (F9 guard) and returns an event only when
        // it differs from the last persisted one. On the unchanged path the
        // stored observation already is what re-projection would return.
        match self.observation_reconcile_event(observation)? {
            None => Ok(observation.clone()),
            Some(event) => {
                self.append(event)?;
                self.project()?
                    .observations
                    .remove(&observation_id)
                    .ok_or(WorkspaceError::ObservationNotFound(observation_id))
            }
        }
    }

    pub fn record_claim(
        &self,
        statement: impl Into<String>,
        supporting_observation_ids: &[u64],
        declared_dependencies: &[PathBuf],
    ) -> Result<Claim, WorkspaceError> {
        self.record_claim_with_scope(
            statement,
            supporting_observation_ids,
            declared_dependencies,
            ClaimScopeStrategy::Declared,
        )
    }

    pub fn record_claim_with_scope(
        &self,
        statement: impl Into<String>,
        supporting_observation_ids: &[u64],
        declared_dependencies: &[PathBuf],
        scope_strategy: ClaimScopeStrategy,
    ) -> Result<Claim, WorkspaceError> {
        let statement = statement.into();
        if statement.trim().is_empty() {
            return Err(WorkspaceError::InvalidClaim(
                "statement must not be empty".to_owned(),
            ));
        }
        if supporting_observation_ids.is_empty() {
            return Err(WorkspaceError::InvalidClaim(
                "at least one supporting observation is required".to_owned(),
            ));
        }

        let projection = self.project()?;
        let mut inputs = BTreeMap::new();
        let mut supporting_paths = Vec::new();
        for observation_id in supporting_observation_ids {
            let observation = projection
                .observations
                .get(observation_id)
                .ok_or(WorkspaceError::ObservationNotFound(*observation_id))?;
            supporting_paths.push(observation.path.clone());
            inputs.insert(
                (observation.path.clone(), observation.selector.clone()),
                ClaimInput {
                    path: observation.path.clone(),
                    selector: observation.selector.clone(),
                    normalizer: observation.normalizer,
                    recorded_input_fingerprint: observation.observed_input_fingerprint.clone(),
                    recorded_raw_fingerprint: observation.observed_raw_fingerprint.clone(),
                    source: ClaimInputSource::SupportingObservation,
                },
            );
        }
        for dependency in declared_dependencies {
            let path = validate_relative_path(dependency)?;
            let (normalizer, input_fingerprint, raw_fingerprint) =
                fingerprint_dependency(&self.repository_root, &path)?;
            inputs
                .entry((path.clone(), ObservationSelector::WholeFile))
                .or_insert(ClaimInput {
                    path,
                    selector: ObservationSelector::WholeFile,
                    normalizer,
                    recorded_input_fingerprint: input_fingerprint,
                    recorded_raw_fingerprint: raw_fingerprint,
                    source: ClaimInputSource::DeclaredDependency,
                });
        }
        if scope_strategy == ClaimScopeStrategy::ConservativeSiblingFiles {
            for path in conservative_sibling_dependencies(&self.repository_root, &supporting_paths)?
            {
                let (normalizer, input_fingerprint, raw_fingerprint) =
                    fingerprint_dependency(&self.repository_root, &path)?;
                inputs
                    .entry((path.clone(), ObservationSelector::WholeFile))
                    .or_insert(ClaimInput {
                        path,
                        selector: ObservationSelector::WholeFile,
                        normalizer,
                        recorded_input_fingerprint: input_fingerprint,
                        recorded_raw_fingerprint: raw_fingerprint,
                        source: ClaimInputSource::ConservativeDependency,
                    });
            }
        }
        let inputs: Vec<_> = inputs.into_values().collect();
        let (freshness, reason, fingerprint_inputs) =
            assess_claim_inputs(&self.repository_root, &inputs);
        let reconciliation_fingerprint =
            scoped_reconciliation_fingerprint(&self.repository_root, &fingerprint_inputs)?;
        let claim_id = projection.next_claim_id;

        self.append(Event::ClaimRecorded {
            claim_id,
            statement,
            supporting_observation_ids: supporting_observation_ids.to_vec(),
            scope_strategy,
            inputs,
            freshness,
            reason,
            reconciliation_fingerprint,
        })?;

        self.project()?
            .claims
            .remove(&claim_id)
            .ok_or(WorkspaceError::ClaimNotFound(claim_id))
    }

    pub fn reconcile_claim(&self, claim_id: u64) -> Result<Claim, WorkspaceError> {
        let projection = self.project()?;
        let claim = projection
            .claims
            .get(&claim_id)
            .ok_or(WorkspaceError::ClaimNotFound(claim_id))?;
        match self.claim_reconcile_event(claim)? {
            None => Ok(claim.clone()),
            Some(event) => {
                self.append(event)?;
                self.project()?
                    .claims
                    .remove(&claim_id)
                    .ok_or(WorkspaceError::ClaimNotFound(claim_id))
            }
        }
    }

    pub fn supersede_claim(
        &self,
        claim_id: u64,
        replacement_claim_id: u64,
        reason: impl Into<String>,
    ) -> Result<Claim, WorkspaceError> {
        let reason = reason.into();
        if reason.trim().is_empty() {
            return Err(WorkspaceError::InvalidClaim(
                "supersession reason must not be empty".to_owned(),
            ));
        }
        if claim_id == replacement_claim_id {
            return Err(WorkspaceError::InvalidClaim(
                "a claim cannot supersede itself".to_owned(),
            ));
        }
        let projection = self.project()?;
        let claim = projection
            .claims
            .get(&claim_id)
            .ok_or(WorkspaceError::ClaimNotFound(claim_id))?;
        if !claim.lifecycle.is_active() {
            return Err(WorkspaceError::InvalidClaim(format!(
                "claim {claim_id} is already superseded"
            )));
        }
        let replacement = projection
            .claims
            .get(&replacement_claim_id)
            .ok_or(WorkspaceError::ClaimNotFound(replacement_claim_id))?;
        if !replacement.lifecycle.is_active() {
            return Err(WorkspaceError::InvalidClaim(format!(
                "replacement claim {replacement_claim_id} is superseded"
            )));
        }
        if projection.transactions.values().any(|transaction| {
            transaction.state == TransactionState::Open
                && transaction.acceptance_claim_ids.contains(&claim_id)
        }) {
            return Err(WorkspaceError::InvalidClaim(format!(
                "claim {claim_id} belongs to an open transaction"
            )));
        }

        self.reconcile_claim(claim_id)?;
        self.append(Event::ClaimSuperseded {
            claim_id,
            replacement_claim_id,
            reason,
        })?;
        self.project()?
            .claims
            .remove(&claim_id)
            .ok_or(WorkspaceError::ClaimNotFound(claim_id))
    }

    pub fn begin_transaction(
        &self,
        intent: impl Into<String>,
        acceptance_claim_ids: &[u64],
    ) -> Result<Transaction, WorkspaceError> {
        let intent = intent.into();
        if intent.trim().is_empty() {
            return Err(WorkspaceError::InvalidTransaction(
                "a transaction intent is required".to_owned(),
            ));
        }
        if acceptance_claim_ids.is_empty() {
            return Err(WorkspaceError::InvalidTransaction(
                "at least one acceptance claim is required".to_owned(),
            ));
        }
        let projection = self.project()?;
        for claim_id in acceptance_claim_ids {
            let claim = projection
                .claims
                .get(claim_id)
                .ok_or(WorkspaceError::ClaimNotFound(*claim_id))?;
            if !claim.lifecycle.is_active() {
                return Err(WorkspaceError::InvalidTransaction(format!(
                    "acceptance claim {claim_id} is superseded"
                )));
            }
        }
        let transaction_id = projection.next_transaction_id;
        self.append(Event::TransactionBegan {
            transaction_id,
            intent: Some(intent),
            base_revision: git_output(&self.repository_root, &["rev-parse", "HEAD"])?,
            initial_worktree_fingerprint: worktree_fingerprint(&self.repository_root)?,
            acceptance_claim_ids: acceptance_claim_ids.to_vec(),
        })?;
        self.project()?
            .transactions
            .remove(&transaction_id)
            .ok_or(WorkspaceError::TransactionNotFound(transaction_id))
    }

    /// Associate a finding this transaction addresses. Requires an open
    /// transaction and an existing finding; idempotent on repeat. Association is
    /// a link only — it does not dispose the finding, which stays a separate
    /// explicit act.
    pub fn associate_finding(
        &self,
        transaction_id: u64,
        finding_id: u64,
    ) -> Result<Transaction, WorkspaceError> {
        let projection = self.project()?;
        let transaction = projection
            .transactions
            .get(&transaction_id)
            .ok_or(WorkspaceError::TransactionNotFound(transaction_id))?;
        if transaction.state != TransactionState::Open {
            return Err(WorkspaceError::InvalidTransaction(
                "findings can only be associated with an open transaction".to_owned(),
            ));
        }
        if !projection.findings.contains_key(&finding_id) {
            return Err(WorkspaceError::FindingNotFound(finding_id));
        }
        self.append(Event::TransactionFindingAssociated {
            transaction_id,
            finding_id,
        })?;
        self.project()?
            .transactions
            .remove(&transaction_id)
            .ok_or(WorkspaceError::TransactionNotFound(transaction_id))
    }

    /// Record a residual risk the author is knowingly accepting on an open
    /// transaction.
    pub fn record_residual_risk(
        &self,
        transaction_id: u64,
        risk: impl Into<String>,
    ) -> Result<Transaction, WorkspaceError> {
        let risk = risk.into();
        if risk.trim().is_empty() {
            return Err(WorkspaceError::InvalidTransaction(
                "a residual risk must not be empty".to_owned(),
            ));
        }
        let projection = self.project()?;
        let transaction = projection
            .transactions
            .get(&transaction_id)
            .ok_or(WorkspaceError::TransactionNotFound(transaction_id))?;
        if transaction.state != TransactionState::Open {
            return Err(WorkspaceError::InvalidTransaction(
                "residual risks can only be recorded on an open transaction".to_owned(),
            ));
        }
        self.append(Event::TransactionResidualRiskRecorded {
            transaction_id,
            risk,
        })?;
        self.project()?
            .transactions
            .remove(&transaction_id)
            .ok_or(WorkspaceError::TransactionNotFound(transaction_id))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_evidence(
        &self,
        transaction_id: u64,
        claim_id: u64,
        check_name: impl Into<String>,
        invocation: impl Into<String>,
        provider: impl Into<String>,
        outcome: EvidenceOutcome,
    ) -> Result<Evidence, WorkspaceError> {
        let projection = self.project()?;
        let transaction = projection
            .transactions
            .get(&transaction_id)
            .ok_or(WorkspaceError::TransactionNotFound(transaction_id))?;
        if transaction.state != TransactionState::Open
            || !transaction.acceptance_claim_ids.contains(&claim_id)
        {
            return Err(WorkspaceError::InvalidEvidence(
                "evidence must support an open transaction acceptance claim".to_owned(),
            ));
        }
        let claim = projection
            .claims
            .get(&claim_id)
            .ok_or(WorkspaceError::ClaimNotFound(claim_id))?;
        if !claim.lifecycle.is_active() {
            return Err(WorkspaceError::InvalidEvidence(format!(
                "claim {claim_id} is superseded"
            )));
        }
        let inputs = claim.inputs.clone();
        let (freshness, reason, fingerprint_inputs) =
            assess_claim_inputs(&self.repository_root, &inputs);
        if freshness != FreshnessWithinScope::Current {
            return Err(WorkspaceError::InvalidEvidence(
                "claim inputs are not current".to_owned(),
            ));
        }
        // Materialization gate: bind this evidence to the candidate the checks
        // actually ran against. That is only honest if the candidate is on disk
        // *now* — every owned path hashing to its `after_fingerprint`. If a path
        // has drifted before the evidence is even recorded, the check could not
        // have consumed this candidate, so we refuse to stamp it.
        if let Some(drift) = self.candidate_drift(transaction) {
            return Err(WorkspaceError::InvalidEvidence(format!(
                "candidate not materialized: {drift}"
            )));
        }
        let candidate_fingerprint = transaction.candidate_fingerprint();
        let evidence_id = projection.next_evidence_id;
        self.append(Event::EvidenceRecorded {
            evidence_id,
            transaction_id,
            claim_id,
            check_name: check_name.into(),
            invocation: invocation.into(),
            provider: provider.into(),
            outcome,
            inputs,
            candidate_fingerprint,
            freshness,
            reason,
            reconciliation_fingerprint: scoped_reconciliation_fingerprint(
                &self.repository_root,
                &fingerprint_inputs,
            )?,
        })?;
        self.project()?
            .evidence
            .remove(&evidence_id)
            .ok_or(WorkspaceError::EvidenceNotFound(evidence_id))
    }

    pub fn reconcile_evidence(&self, evidence_id: u64) -> Result<Evidence, WorkspaceError> {
        let projection = self.project()?;
        let evidence = projection
            .evidence
            .get(&evidence_id)
            .ok_or(WorkspaceError::EvidenceNotFound(evidence_id))?;
        match self.evidence_reconcile_event(evidence)? {
            None => Ok(evidence.clone()),
            Some(event) => {
                self.append(event)?;
                self.project()?
                    .evidence
                    .remove(&evidence_id)
                    .ok_or(WorkspaceError::EvidenceNotFound(evidence_id))
            }
        }
    }

    pub fn apply_file_mutation(
        &self,
        transaction_id: u64,
        path: impl AsRef<Path>,
        new_contents: &[u8],
    ) -> Result<Transaction, WorkspaceError> {
        let path = validate_relative_path(path.as_ref())?;
        let projection = self.project()?;
        let transaction = projection
            .transactions
            .get(&transaction_id)
            .ok_or(WorkspaceError::TransactionNotFound(transaction_id))?;
        if transaction.state != TransactionState::Open
            || transaction
                .mutations
                .iter()
                .any(|mutation| mutation.path == path)
        {
            return Err(WorkspaceError::InvalidTransaction(
                "transaction is not open or already owns this path".to_owned(),
            ));
        }
        let before =
            git_file_at_revision(&self.repository_root, &transaction.base_revision, &path)?;
        let absolute_path = self.repository_root.join(&path);
        let current = fs::read(&absolute_path)?;
        if current != before {
            return Err(WorkspaceError::InvalidTransaction(
                "S6 clean-base mutation requires the path to match the base revision".to_owned(),
            ));
        }
        write_file_atomically(&absolute_path, new_contents)?;
        let event = Event::MutationApplied {
            transaction_id,
            mutation: Mutation {
                path,
                before_fingerprint: hex_digest(&before),
                after_fingerprint: hex_digest(new_contents),
            },
        };
        if let Err(error) = self.append(event) {
            let _ = write_file_atomically(&absolute_path, &before);
            return Err(error);
        }
        self.resume_status()?
            .transactions
            .into_iter()
            .find(|transaction| transaction.id == transaction_id)
            .ok_or(WorkspaceError::TransactionNotFound(transaction_id))
    }

    pub fn revert_transaction(&self, transaction_id: u64) -> Result<Transaction, WorkspaceError> {
        let projection = self.project()?;
        let transaction = projection
            .transactions
            .get(&transaction_id)
            .ok_or(WorkspaceError::TransactionNotFound(transaction_id))?;
        if transaction.state != TransactionState::Open || transaction.mutations.is_empty() {
            return Err(WorkspaceError::InvalidTransaction(
                "transaction is not open or has no mutations".to_owned(),
            ));
        }

        let mut validated = Vec::with_capacity(transaction.mutations.len());
        for mutation in transaction.mutations.iter().rev() {
            let absolute_path = self.repository_root.join(&mutation.path);
            let owned_contents = fs::read(&absolute_path)?;
            if hex_digest(&owned_contents) != mutation.after_fingerprint {
                return Err(WorkspaceError::InvalidTransaction(format!(
                    "revert conflict on {}",
                    mutation.path.display()
                )));
            }
            let original = git_file_at_revision(
                &self.repository_root,
                &transaction.base_revision,
                &mutation.path,
            )?;
            if hex_digest(&original) != mutation.before_fingerprint {
                return Err(WorkspaceError::CorruptLog(format!(
                    "base fingerprint mismatch for {}",
                    mutation.path.display()
                )));
            }
            validated.push((absolute_path, original, owned_contents));
        }

        for (index, (path, original, _)) in validated.iter().enumerate() {
            if let Err(error) = write_file_atomically(path, original) {
                for (restored_path, _, owned_contents) in validated[..index].iter().rev() {
                    let _ = write_file_atomically(restored_path, owned_contents);
                }
                return Err(error);
            }
        }
        if let Err(error) = self.append(Event::TransactionReverted { transaction_id }) {
            for (path, _, owned_contents) in validated.iter().rev() {
                let _ = write_file_atomically(path, owned_contents);
            }
            return Err(error);
        }
        self.resume_status()?
            .transactions
            .into_iter()
            .find(|transaction| transaction.id == transaction_id)
            .ok_or(WorkspaceError::TransactionNotFound(transaction_id))
    }

    /// Re-verify that the transaction's owned bytes still sit on disk exactly as
    /// they were applied: each mutated path must hash to its `after_fingerprint`.
    /// Returns `None` when every path matches, or `Some(reason)` naming the first
    /// path that has drifted (a formatter reflow, a hand-edit) or can no longer be
    /// read. This is the fail-closed gate that keeps acceptance from committing —
    /// and calling "fresh" — bytes that differ from what the checks consumed. Pure
    /// read; it never writes.
    fn candidate_drift(&self, transaction: &Transaction) -> Option<String> {
        for mutation in &transaction.mutations {
            let absolute_path = self.repository_root.join(&mutation.path);
            match fs::read(&absolute_path) {
                Ok(bytes) if hex_digest(&bytes) == mutation.after_fingerprint => {}
                Ok(_) => {
                    return Some(format!(
                        "owned bytes at {} drifted since apply (post-apply edit or formatter)",
                        mutation.path.display()
                    ));
                }
                Err(error) => {
                    return Some(format!(
                        "cannot re-verify owned bytes at {}: {error}",
                        mutation.path.display()
                    ));
                }
            }
        }
        None
    }

    pub fn accept_transaction(&self, transaction_id: u64) -> Result<Transaction, WorkspaceError> {
        let projection = self.project()?;
        let transaction = projection
            .transactions
            .get(&transaction_id)
            .ok_or(WorkspaceError::TransactionNotFound(transaction_id))?;
        if transaction.state != TransactionState::Open {
            return Err(WorkspaceError::InvalidTransaction(
                "transaction is not open".to_owned(),
            ));
        }
        let claim_ids = transaction.acceptance_claim_ids.clone();
        let evidence_ids = transaction.evidence_ids.clone();
        for claim_id in &claim_ids {
            self.reconcile_claim(*claim_id)?;
        }
        for evidence_id in &evidence_ids {
            self.reconcile_evidence(*evidence_id)?;
        }

        // One rule, evaluated the same way `transaction_preview` shows it: the
        // owned bytes must still be on disk exactly as applied (disk gate), and
        // every acceptance claim must be current with passing evidence bound to
        // the current candidate (pure gate). Both fail closed.
        let status = Self::status_from_projection(self.project()?);
        let transaction = status
            .transactions
            .iter()
            .find(|transaction| transaction.id == transaction_id)
            .ok_or(WorkspaceError::TransactionNotFound(transaction_id))?;
        let (validated, reason) = match self.candidate_drift(transaction) {
            Some(drift) => (false, drift),
            None => status.acceptance_readiness(transaction),
        };
        if validated {
            self.append(Event::TransactionAccepted { transaction_id })?;
        } else {
            self.append(Event::TransactionAcceptanceRejected {
                transaction_id,
                reason,
            })?;
        }
        self.project()?
            .transactions
            .remove(&transaction_id)
            .ok_or(WorkspaceError::TransactionNotFound(transaction_id))
    }

    fn persist_native_payload(
        &self,
        fingerprint: &str,
        contents: &[u8],
    ) -> Result<String, WorkspaceError> {
        if !is_sha256_hex(fingerprint) {
            return Err(WorkspaceError::CorruptLog(
                "invalid native payload fingerprint".to_owned(),
            ));
        }
        let payload_directory = self.workspace_root.join("payloads");
        match fs::symlink_metadata(&payload_directory) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(WorkspaceError::InvalidObservation(
                    "payload storage is not a regular directory".to_owned(),
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&payload_directory)?;
            }
            Err(error) => return Err(error.into()),
        }
        if !payload_directory
            .canonicalize()?
            .starts_with(&self.workspace_root)
        {
            return Err(WorkspaceError::InvalidObservation(
                "payload storage escapes the workspace".to_owned(),
            ));
        }
        let relative = PathBuf::from("payloads").join(fingerprint);
        let absolute = payload_directory.join(fingerprint);
        match fs::symlink_metadata(&absolute) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                return Err(WorkspaceError::CorruptLog(format!(
                    "native payload is not a regular file for {fingerprint}"
                )));
            }
            Ok(_) => {
                if fingerprint_file(&absolute)? != fingerprint {
                    return Err(WorkspaceError::CorruptLog(format!(
                        "native payload collision for {fingerprint}"
                    )));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                write_file_atomically(&absolute, contents)?;
            }
            Err(error) => return Err(error.into()),
        }
        Ok(relative.to_string_lossy().into_owned())
    }

    pub fn event_log_path(&self) -> PathBuf {
        self.workspace_root.join(EVENT_LOG_NAME)
    }

    /// Acquire the exclusive advisory lock that serializes all operations on
    /// this workspace, blocking until it is available. Hold the returned guard
    /// for the full duration of a command's read-modify-write: every mutating
    /// op reads a projection to compute the next entity id and sequence, then
    /// appends, and nothing may interleave between those steps or two processes
    /// would collide on a sequence (corrupt log) or an entity id (silent
    /// overwrite). The lock releases when the guard is dropped or the process
    /// exits. Callers driving the library directly (rather than via the CLI)
    /// must hold this around any sequence of mutations.
    pub fn lock_exclusive(&self) -> Result<WorkspaceLock, WorkspaceError> {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(self.workspace_root.join(LOCK_FILE_NAME))?;
        file.lock()?;
        Ok(WorkspaceLock { _file: file })
    }

    fn append(&self, event: Event) -> Result<(), WorkspaceError> {
        let sequence = self.project()?.next_sequence;
        let record = EventRecord {
            schema_version: EVENT_SCHEMA_VERSION,
            sequence,
            event,
        };
        let mut encoded = serde_json::to_vec(&record)?;
        encoded.push(b'\n');

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.event_log_path())?;
        file.write_all(&encoded)?;
        file.sync_data()?;
        Ok(())
    }

    fn project(&self) -> Result<Projection, WorkspaceError> {
        self.project_upto(None)
    }

    /// Replay the log, optionally stopping after `max_sequence`. `None` projects
    /// the whole log (current state); `Some(s)` reconstructs the state the log
    /// described up to and including sequence `s` — the baseline a delta diffs
    /// against. This is a pure read: it never appends reconciliation events, so
    /// the freshness it reports is the freshness *recorded at that time*.
    fn project_upto(&self, max_sequence: Option<u64>) -> Result<Projection, WorkspaceError> {
        let path = self.event_log_path();
        if !path.exists() {
            return Ok(Projection::default());
        }

        let reader = BufReader::new(File::open(path)?);
        self.event_log_reads
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut projection = Projection::default();
        for (index, line) in reader.lines().enumerate() {
            let line = line?;
            let record: EventRecord = serde_json::from_str(&line).map_err(|error| {
                WorkspaceError::CorruptLog(format!("line {}: {error}", index + 1))
            })?;
            if max_sequence.is_some_and(|max_sequence| record.sequence > max_sequence) {
                break;
            }
            projection.apply(record)?;
        }
        Ok(projection)
    }
}

#[derive(Default)]
struct Projection {
    objective: Option<Objective>,
    working_set: BTreeMap<u64, WorkingSetEntry>,
    /// Ordered focus history — one entry per `ObservationFocused` event,
    /// revisits included. The deduped `working_set` map answers "what am I
    /// attending to"; this Vec answers "in what order did I get here", which the
    /// map's key ordering cannot express. Both are replayed from the same event
    /// stream, so both recover on restart for free.
    navigation_trail: Vec<WorkingSetEntry>,
    next_focus_ordinal: u64,
    observations: BTreeMap<u64, Observation>,
    claims: BTreeMap<u64, Claim>,
    evidence: BTreeMap<u64, Evidence>,
    findings: BTreeMap<u64, Finding>,
    transactions: BTreeMap<u64, Transaction>,
    checkpoints: Vec<CheckpointMarker>,
    next_observation_id: u64,
    next_claim_id: u64,
    next_evidence_id: u64,
    next_finding_id: u64,
    next_transaction_id: u64,
    next_sequence: u64,
}

impl Projection {
    fn apply(&mut self, record: EventRecord) -> Result<(), WorkspaceError> {
        if !(MINIMUM_EVENT_SCHEMA_VERSION..=EVENT_SCHEMA_VERSION).contains(&record.schema_version) {
            return Err(WorkspaceError::CorruptLog(format!(
                "unsupported schema version {}",
                record.schema_version
            )));
        }
        if record.sequence != self.next_sequence {
            return Err(WorkspaceError::CorruptLog(format!(
                "expected sequence {}, found {}",
                self.next_sequence, record.sequence
            )));
        }
        self.next_sequence += 1;

        match record.event {
            Event::ObjectiveBound {
                intent,
                external_reference,
            } => {
                self.objective = Some(Objective {
                    intent,
                    external_reference,
                });
            }
            Event::ObservationFocused {
                observation_id,
                reason,
            } => {
                if !self.observations.contains_key(&observation_id) {
                    return Err(WorkspaceError::ObservationNotFound(observation_id));
                }
                let focus_sequence = self.next_focus_ordinal;
                self.next_focus_ordinal += 1;
                let entry = WorkingSetEntry {
                    observation_id,
                    reason,
                    focus_sequence,
                };
                // The trail keeps every visit in order; the working set keeps the
                // latest visit per observation, so a re-focus moves that location
                // to the front of recency rather than accumulating duplicates.
                self.navigation_trail.push(entry.clone());
                self.working_set.insert(observation_id, entry);
            }
            Event::ObservationRecorded {
                observation_id,
                path,
                provider,
                git_revision,
                selector,
                normalizer,
                input_fingerprint,
                raw_fingerprint,
                container_fingerprint,
                native_payload_reference,
                ingested_bytes,
                model_visible_bytes,
                reconciliation_fingerprint,
            } => {
                if self.observations.contains_key(&observation_id) {
                    return Err(WorkspaceError::CorruptLog(format!(
                        "duplicate observation {observation_id}"
                    )));
                }
                self.next_observation_id = self.next_observation_id.max(observation_id + 1);
                self.observations.insert(
                    observation_id,
                    Observation {
                        id: observation_id,
                        path: path.clone(),
                        provider,
                        observed_revision: git_revision,
                        selector: selector.clone(),
                        normalizer,
                        observed_raw_fingerprint: raw_fingerprint,
                        observed_container_fingerprint: container_fingerprint
                            .unwrap_or_else(|| input_fingerprint.clone()),
                        observed_input_fingerprint: input_fingerprint,
                        native_payload_reference,
                        ingested_bytes,
                        model_visible_bytes,
                        report: FreshnessReport {
                            freshness_within_scope: FreshnessWithinScope::Current,
                            scope_assurance: ScopeAssurance {
                                source: ScopeSource::Declared,
                                completeness: if selector == ObservationSelector::WholeFile {
                                    ScopeCompleteness::AssertedComplete
                                } else {
                                    ScopeCompleteness::NotAsserted
                                },
                            },
                            operational_coverage: OperationalCoverage {
                                mediated_paths: vec![path.clone()],
                                mediated_units: vec![MediatedUnit { path, selector }],
                                reconciliation_fingerprint,
                            },
                            reason: "supporting input recorded".to_owned(),
                        },
                    },
                );
            }
            Event::ObservationReconciled {
                observation_id,
                freshness,
                reason,
                reconciliation_fingerprint,
            } => {
                let observation = self
                    .observations
                    .get_mut(&observation_id)
                    .ok_or(WorkspaceError::ObservationNotFound(observation_id))?;
                observation.report.freshness_within_scope = freshness;
                observation.report.reason = reason;
                observation
                    .report
                    .operational_coverage
                    .reconciliation_fingerprint = reconciliation_fingerprint;
            }
            Event::ClaimRecorded {
                claim_id,
                statement,
                supporting_observation_ids,
                scope_strategy,
                inputs,
                freshness,
                reason,
                reconciliation_fingerprint,
            } => {
                if self.claims.contains_key(&claim_id) {
                    return Err(WorkspaceError::CorruptLog(format!(
                        "duplicate claim {claim_id}"
                    )));
                }
                if let Some(missing) = supporting_observation_ids
                    .iter()
                    .find(|id| !self.observations.contains_key(id))
                {
                    return Err(WorkspaceError::CorruptLog(format!(
                        "claim {claim_id} references missing observation {missing}"
                    )));
                }
                self.next_claim_id = self.next_claim_id.max(claim_id + 1);
                let mediated_paths = inputs.iter().map(|input| input.path.clone()).collect();
                let mediated_units = inputs
                    .iter()
                    .map(|input| MediatedUnit {
                        path: input.path.clone(),
                        selector: input.selector.clone(),
                    })
                    .collect();
                let assurance_source = scope_strategy.assurance_source();
                self.claims.insert(
                    claim_id,
                    Claim {
                        id: claim_id,
                        statement,
                        supporting_observation_ids,
                        scope_strategy,
                        inputs,
                        lifecycle: ClaimLifecycle::Active,
                        report: FreshnessReport {
                            freshness_within_scope: freshness,
                            scope_assurance: ScopeAssurance {
                                source: assurance_source,
                                completeness: ScopeCompleteness::NotAsserted,
                            },
                            operational_coverage: OperationalCoverage {
                                mediated_paths,
                                mediated_units,
                                reconciliation_fingerprint,
                            },
                            reason,
                        },
                    },
                );
            }
            Event::ClaimReconciled {
                claim_id,
                freshness,
                reason,
                reconciliation_fingerprint,
            } => {
                let claim = self
                    .claims
                    .get_mut(&claim_id)
                    .ok_or(WorkspaceError::ClaimNotFound(claim_id))?;
                claim.report.freshness_within_scope = freshness;
                claim.report.reason = reason;
                claim.report.operational_coverage.reconciliation_fingerprint =
                    reconciliation_fingerprint;
            }
            Event::ClaimSuperseded {
                claim_id,
                replacement_claim_id,
                reason,
            } => {
                if reason.trim().is_empty() {
                    return Err(WorkspaceError::CorruptLog(format!(
                        "claim {claim_id} has an empty supersession reason"
                    )));
                }
                if claim_id == replacement_claim_id {
                    return Err(WorkspaceError::CorruptLog(format!(
                        "claim {claim_id} supersedes itself"
                    )));
                }
                if self.transactions.values().any(|transaction| {
                    transaction.state == TransactionState::Open
                        && transaction.acceptance_claim_ids.contains(&claim_id)
                }) {
                    return Err(WorkspaceError::CorruptLog(format!(
                        "superseded claim {claim_id} belongs to an open transaction"
                    )));
                }
                let replacement = self
                    .claims
                    .get(&replacement_claim_id)
                    .ok_or(WorkspaceError::ClaimNotFound(replacement_claim_id))?;
                if !replacement.lifecycle.is_active() {
                    return Err(WorkspaceError::CorruptLog(format!(
                        "replacement claim {replacement_claim_id} is superseded"
                    )));
                }
                let claim = self
                    .claims
                    .get_mut(&claim_id)
                    .ok_or(WorkspaceError::ClaimNotFound(claim_id))?;
                if !claim.lifecycle.is_active() {
                    return Err(WorkspaceError::CorruptLog(format!(
                        "claim {claim_id} is already superseded"
                    )));
                }
                claim.lifecycle = ClaimLifecycle::Superseded {
                    replacement_claim_id,
                    reason,
                };
            }
            Event::TransactionBegan {
                transaction_id,
                intent,
                base_revision,
                initial_worktree_fingerprint,
                acceptance_claim_ids,
            } => {
                if self.transactions.contains_key(&transaction_id) {
                    return Err(WorkspaceError::CorruptLog(format!(
                        "duplicate transaction {transaction_id}"
                    )));
                }
                if acceptance_claim_ids.is_empty() {
                    return Err(WorkspaceError::CorruptLog(format!(
                        "transaction {transaction_id} has no acceptance claims"
                    )));
                }
                for claim_id in &acceptance_claim_ids {
                    let claim = self
                        .claims
                        .get(claim_id)
                        .ok_or(WorkspaceError::ClaimNotFound(*claim_id))?;
                    if !claim.lifecycle.is_active() {
                        return Err(WorkspaceError::CorruptLog(format!(
                            "transaction {transaction_id} references superseded claim {claim_id}"
                        )));
                    }
                }
                self.next_transaction_id = self.next_transaction_id.max(transaction_id + 1);
                self.transactions.insert(
                    transaction_id,
                    Transaction {
                        id: transaction_id,
                        intent,
                        base_revision,
                        initial_worktree_fingerprint,
                        acceptance_claim_ids,
                        evidence_ids: Vec::new(),
                        finding_ids: Vec::new(),
                        residual_risks: Vec::new(),
                        mutations: Vec::new(),
                        state: TransactionState::Open,
                        last_rejection: None,
                    },
                );
            }
            Event::TransactionFindingAssociated {
                transaction_id,
                finding_id,
            } => {
                if !self.findings.contains_key(&finding_id) {
                    return Err(WorkspaceError::FindingNotFound(finding_id));
                }
                let transaction = self
                    .transactions
                    .get_mut(&transaction_id)
                    .ok_or(WorkspaceError::TransactionNotFound(transaction_id))?;
                if !transaction.finding_ids.contains(&finding_id) {
                    transaction.finding_ids.push(finding_id);
                }
            }
            Event::TransactionResidualRiskRecorded {
                transaction_id,
                risk,
            } => {
                self.transactions
                    .get_mut(&transaction_id)
                    .ok_or(WorkspaceError::TransactionNotFound(transaction_id))?
                    .residual_risks
                    .push(risk);
            }
            Event::EvidenceRecorded {
                evidence_id,
                transaction_id,
                claim_id,
                check_name,
                invocation,
                provider,
                outcome,
                inputs,
                candidate_fingerprint,
                freshness,
                reason,
                reconciliation_fingerprint,
            } => {
                if self.evidence.contains_key(&evidence_id) {
                    return Err(WorkspaceError::CorruptLog(format!(
                        "duplicate evidence {evidence_id}"
                    )));
                }
                let transaction = self
                    .transactions
                    .get(&transaction_id)
                    .ok_or(WorkspaceError::TransactionNotFound(transaction_id))?;
                if transaction.state != TransactionState::Open
                    || !transaction.acceptance_claim_ids.contains(&claim_id)
                {
                    return Err(WorkspaceError::CorruptLog(format!(
                        "evidence {evidence_id} does not support an open transaction acceptance claim"
                    )));
                }
                let claim = self
                    .claims
                    .get(&claim_id)
                    .ok_or(WorkspaceError::ClaimNotFound(claim_id))?;
                if !claim.lifecycle.is_active() {
                    return Err(WorkspaceError::CorruptLog(format!(
                        "evidence {evidence_id} references superseded claim {claim_id}"
                    )));
                }
                if inputs != claim.inputs || freshness != FreshnessWithinScope::Current {
                    return Err(WorkspaceError::CorruptLog(format!(
                        "evidence {evidence_id} does not match its current claim inputs"
                    )));
                }
                let assurance_source = claim.report.scope_assurance.source.clone();
                let mediated_paths = claim
                    .inputs
                    .iter()
                    .map(|input| input.path.clone())
                    .collect();
                let mediated_units = claim
                    .inputs
                    .iter()
                    .map(|input| MediatedUnit {
                        path: input.path.clone(),
                        selector: input.selector.clone(),
                    })
                    .collect();
                self.next_evidence_id = self.next_evidence_id.max(evidence_id + 1);
                self.transactions
                    .get_mut(&transaction_id)
                    .ok_or(WorkspaceError::TransactionNotFound(transaction_id))?
                    .evidence_ids
                    .push(evidence_id);
                self.evidence.insert(
                    evidence_id,
                    Evidence {
                        id: evidence_id,
                        transaction_id,
                        claim_id,
                        check_name,
                        invocation,
                        provider,
                        outcome,
                        inputs,
                        candidate_fingerprint,
                        report: FreshnessReport {
                            freshness_within_scope: freshness,
                            scope_assurance: ScopeAssurance {
                                source: assurance_source,
                                completeness: ScopeCompleteness::NotAsserted,
                            },
                            operational_coverage: OperationalCoverage {
                                mediated_paths,
                                mediated_units,
                                reconciliation_fingerprint,
                            },
                            reason,
                        },
                    },
                );
            }
            Event::EvidenceReconciled {
                evidence_id,
                freshness,
                reason,
                reconciliation_fingerprint,
            } => {
                let evidence = self
                    .evidence
                    .get_mut(&evidence_id)
                    .ok_or(WorkspaceError::EvidenceNotFound(evidence_id))?;
                evidence.report.freshness_within_scope = freshness;
                evidence.report.reason = reason;
                evidence
                    .report
                    .operational_coverage
                    .reconciliation_fingerprint = reconciliation_fingerprint;
            }
            Event::FindingRecorded {
                finding_id,
                provider,
                severity,
                rule,
                message,
                path,
                selector,
                normalizer,
                git_revision,
                input_fingerprint,
                raw_fingerprint,
                container_fingerprint,
                native_payload_reference,
                native_payload_fingerprint,
                freshness,
                reason,
                reconciliation_fingerprint,
            } => {
                if self.findings.contains_key(&finding_id) {
                    return Err(WorkspaceError::CorruptLog(format!(
                        "duplicate finding {finding_id}"
                    )));
                }
                self.next_finding_id = self.next_finding_id.max(finding_id + 1);
                self.findings.insert(
                    finding_id,
                    Finding {
                        id: finding_id,
                        provider,
                        severity,
                        rule,
                        message,
                        path: path.clone(),
                        selector: selector.clone(),
                        normalizer,
                        observed_revision: git_revision,
                        observed_input_fingerprint: input_fingerprint,
                        observed_raw_fingerprint: raw_fingerprint,
                        observed_container_fingerprint: container_fingerprint,
                        native_payload_reference,
                        native_payload_fingerprint,
                        disposition: FindingDisposition::Open,
                        report: FreshnessReport {
                            freshness_within_scope: freshness,
                            scope_assurance: ScopeAssurance {
                                source: ScopeSource::Declared,
                                completeness: if selector == ObservationSelector::WholeFile {
                                    ScopeCompleteness::AssertedComplete
                                } else {
                                    ScopeCompleteness::NotAsserted
                                },
                            },
                            operational_coverage: OperationalCoverage {
                                mediated_paths: vec![path.clone()],
                                mediated_units: vec![MediatedUnit { path, selector }],
                                reconciliation_fingerprint,
                            },
                            reason,
                        },
                    },
                );
            }
            Event::FindingReconciled {
                finding_id,
                freshness,
                reason,
                reconciliation_fingerprint,
            } => {
                let finding = self
                    .findings
                    .get_mut(&finding_id)
                    .ok_or(WorkspaceError::FindingNotFound(finding_id))?;
                finding.report.freshness_within_scope = freshness;
                finding.report.reason = reason;
                finding
                    .report
                    .operational_coverage
                    .reconciliation_fingerprint = reconciliation_fingerprint;
            }
            Event::FindingDispositionChanged {
                finding_id,
                disposition,
            } => {
                self.findings
                    .get_mut(&finding_id)
                    .ok_or(WorkspaceError::FindingNotFound(finding_id))?
                    .disposition = disposition;
            }
            Event::MutationApplied {
                transaction_id,
                mutation,
            } => {
                let transaction = self
                    .transactions
                    .get_mut(&transaction_id)
                    .ok_or(WorkspaceError::TransactionNotFound(transaction_id))?;
                transaction.mutations.push(mutation);
            }
            Event::TransactionAccepted { transaction_id } => {
                let transaction = self
                    .transactions
                    .get_mut(&transaction_id)
                    .ok_or(WorkspaceError::TransactionNotFound(transaction_id))?;
                transaction.state = TransactionState::Accepted;
                transaction.last_rejection = None;
            }
            Event::TransactionReverted { transaction_id } => {
                let transaction = self
                    .transactions
                    .get_mut(&transaction_id)
                    .ok_or(WorkspaceError::TransactionNotFound(transaction_id))?;
                transaction.state = TransactionState::Reverted;
            }
            Event::TransactionAcceptanceRejected {
                transaction_id,
                reason,
            } => {
                let transaction = self
                    .transactions
                    .get_mut(&transaction_id)
                    .ok_or(WorkspaceError::TransactionNotFound(transaction_id))?;
                transaction.last_rejection = Some(reason);
            }
            Event::Checkpointed {
                label,
                note,
                git_revision,
            } => {
                self.checkpoints.push(CheckpointMarker {
                    label,
                    note,
                    git_revision,
                    objective: self.objective.clone(),
                    // `next_sequence` was advanced above; this event's own
                    // sequence is therefore one less.
                    sequence: self.next_sequence - 1,
                });
            }
        }
        Ok(())
    }
}

fn validate_relative_path(path: &Path) -> Result<PathBuf, WorkspaceError> {
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

fn fingerprint_file(path: &Path) -> Result<String, WorkspaceError> {
    let bytes = fs::read(path)?;
    Ok(hex_digest(&bytes))
}

fn conservative_sibling_dependencies(
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
fn verdict_unchanged(
    report: &FreshnessReport,
    freshness: &FreshnessWithinScope,
    reason: &str,
    reconciliation_fingerprint: &str,
) -> bool {
    report.freshness_within_scope == *freshness
        && report.reason == reason
        && report.operational_coverage.reconciliation_fingerprint == reconciliation_fingerprint
}

fn assess_claim_inputs(repository_root: &Path, inputs: &[ClaimInput]) -> ClaimAssessment {
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

fn resolve_repository_file(
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
fn fingerprint_dependency(
    repository_root: &Path,
    relative_path: &Path,
) -> Result<(Normalizer, String, Option<String>), WorkspaceError> {
    let bytes = fs::read(resolve_repository_file(repository_root, relative_path)?)?;
    let normalizer = Normalizer::detect_for_path(relative_path);
    let input_fingerprint = hex_digest(&normalize_unit(&bytes, normalizer));
    let raw_fingerprint = (normalizer != Normalizer::None).then(|| hex_digest(&bytes));
    Ok((normalizer, input_fingerprint, raw_fingerprint))
}

fn select_observation_unit<'a>(
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
fn location_freshness_verdict(
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

fn read_observation_fingerprints(
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
fn normalize_unit(unit: &[u8], normalizer: Normalizer) -> Vec<u8> {
    match normalizer {
        Normalizer::None => unit.to_vec(),
        Normalizer::Rustfmt => rustfmt_canonical(unit).unwrap_or_else(|| unit.to_vec()),
    }
}

fn rustfmt_canonical(unit: &[u8]) -> Option<Vec<u8>> {
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

fn observation_reconciliation_fingerprint(
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

fn append_selector_fingerprint(material: &mut Vec<u8>, selector: &ObservationSelector) {
    match selector {
        ObservationSelector::WholeFile => material.extend(b"whole_file"),
        ObservationSelector::ByteRange { start, end } => {
            material.extend(b"byte_range");
            material.extend(start.to_le_bytes());
            material.extend(end.to_le_bytes());
        }
    }
}

fn scoped_reconciliation_fingerprint(
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

fn git_file_at_revision(
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

fn write_file_atomically(path: &Path, contents: &[u8]) -> Result<(), WorkspaceError> {
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

fn worktree_fingerprint(repository_root: &Path) -> Result<String, WorkspaceError> {
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

fn git_output(repository_root: &Path, arguments: &[&str]) -> Result<String, WorkspaceError> {
    let output = git_bytes(repository_root, arguments)?;
    String::from_utf8(output)
        .map(|value| value.trim().to_owned())
        .map_err(|error| WorkspaceError::Git(error.to_string()))
}

fn git_bytes(repository_root: &Path, arguments: &[&str]) -> Result<Vec<u8>, WorkspaceError> {
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
struct ReadSelectionPlan {
    selector: ObservationSelector,
    expected_raw_fingerprint: String,
}

/// Concise `ReadCaptureOutcome::Skipped` constructor for the capture guards.
fn skip(reason: impl Into<String>) -> ReadCaptureOutcome {
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
fn plan_read_selection(
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
fn is_sensitive_repository_path(path: &Path) -> bool {
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

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn hex_digest(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}
