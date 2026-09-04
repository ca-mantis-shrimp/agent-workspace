//! The workspace domain model: the durable entities (observations, claims,
//! evidence, findings, transactions) and the small value types and enums they
//! are built from. Plain data with only intrinsic logic — extracted verbatim
//! from `lib.rs`.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::reconcile::hex_digest;

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
/// [`crate::normalizer_config`] and persist the *concrete* normalizer, so
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
    /// Map a registered tool name to its variant, or `None` for a name the
    /// kernel does not know how to drive. This is the *registry*: the closed set
    /// of formatters whose invocation is defined here, which is what lets config
    /// (`.agent-workspace/normalizers.toml`) select a normalizer by name without
    /// ever naming an arbitrary — and thus non-deterministic — shell command.
    /// Shared by config resolution ([`crate::normalizer_config`]) and the CLI's
    /// `--normalize` flag so the accepted names never drift between them.
    pub fn from_tool_name(name: &str) -> Option<Self> {
        match name {
            "none" => Some(Self::None),
            "rustfmt" => Some(Self::Rustfmt),
            _ => None,
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
    pub(crate) fn assurance_source(&self) -> ScopeSource {
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
    pub(crate) fn is_active(&self) -> bool {
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

/// One `rests-on` path of a recorded belief: which observation carries it, and
/// whether that observation already existed in a fresh state (typically ambient
/// adapter capture) and was reused, or had to be captured now. Reuse is the
/// point of the fused verb — it joins the ambient sense ledger to the
/// deliberate belief ledger instead of forking them.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BeliefSupport {
    pub path: PathBuf,
    pub observation_id: u64,
    pub reused: bool,
}

/// The result of the fused belief operation: the claim the agent asserted,
/// plus the per-path reuse accounting that makes the join visible.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Belief {
    pub claim: Claim,
    pub supports: Vec<BeliefSupport>,
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
    pub(crate) fn is_open(&self) -> bool {
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
