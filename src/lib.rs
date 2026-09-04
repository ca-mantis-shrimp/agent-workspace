use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

mod locate;
mod model;
mod projection;
mod reconcile;
pub use locate::resolve_state_root;
pub use model::*;
pub use projection::*;
use projection::{BRIEF_OBJECTIVE_MAX_CHARS, WORKING_SET_UNCITED_CANDIDATE_LIMIT, claim_headline};
use reconcile::*;

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
            observations_since_last_claim: projection.observations_since_last_claim as usize,
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

    /// Record a belief: the agent's cognitive act of "I now believe X, and it
    /// rests on files Y, Z" as one operation instead of the two-step
    /// observe-then-claim that starved claims (observations captured ambiently
    /// by adapters, claims left as raw-CLI friction).
    ///
    /// Each `rests-on` path is **required** — a belief you cannot cite is a
    /// belief you cannot record, and mandatory citation is the schema-level
    /// guard against reflexive, uncited bookkeeping. For every path the freshest
    /// existing observation is reconciled and **reused when current** (joining
    /// the ambient capture ledger to this belief); only when it is stale or
    /// absent is a whole-file observation captured now. Every supporting
    /// observation is focused with the statement itself as the reason, so the
    /// belief's provenance lands in the working set and navigation trail.
    pub fn record_belief(
        &self,
        statement: impl Into<String>,
        rests_on: &[PathBuf],
        scope_strategy: ClaimScopeStrategy,
    ) -> Result<Belief, WorkspaceError> {
        let statement = statement.into();
        if statement.trim().is_empty() {
            return Err(WorkspaceError::InvalidClaim(
                "statement must not be empty".to_owned(),
            ));
        }
        if rests_on.is_empty() {
            return Err(WorkspaceError::InvalidClaim(
                "a belief must rest on at least one cited path".to_owned(),
            ));
        }

        // Duplicate paths collapse onto one support: citing the same file twice
        // is one citation, not two observations.
        let mut seen = BTreeSet::new();
        let mut supports = Vec::new();
        let mut supporting_observation_ids = Vec::new();
        for path in rests_on {
            let path = validate_relative_path(path)?;
            if !seen.insert(path.clone()) {
                continue;
            }
            let (observation_id, reused) = match self.freshest_observation(&path)? {
                Some(candidate) => {
                    let reconciled = self.reconcile_observation(candidate.id)?;
                    if reconciled.report.freshness_within_scope == FreshnessWithinScope::Current {
                        (reconciled.id, true)
                    } else {
                        let capture = self.capture_belief_observation(&path)?;
                        (capture.observation.id, false)
                    }
                }
                None => {
                    let capture = self.capture_belief_observation(&path)?;
                    (capture.observation.id, false)
                }
            };
            self.focus_observation(observation_id, statement.clone())?;
            supporting_observation_ids.push(observation_id);
            supports.push(BeliefSupport {
                path,
                observation_id,
                reused,
            });
        }

        let claim = self.record_claim_with_scope(
            statement,
            &supporting_observation_ids,
            &[],
            scope_strategy,
        )?;
        Ok(Belief { claim, supports })
    }

    /// The newest observation recorded for `path`, if any. Reconcile decides
    /// freshness, so recency alone never promotes a stale record.
    fn freshest_observation(&self, path: &Path) -> Result<Option<Observation>, WorkspaceError> {
        Ok(self
            .project()?
            .observations
            .values()
            .filter(|observation| observation.path == path)
            .max_by_key(|observation| observation.id)
            .cloned())
    }

    fn capture_belief_observation(
        &self,
        path: &Path,
    ) -> Result<ObservationCapture, WorkspaceError> {
        self.capture_file_observation(
            path,
            "agent.belief",
            ObservationCaptureOptions {
                normalizer: Normalizer::detect_for_path(path),
                ..ObservationCaptureOptions::default()
            },
        )
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
    /// How many observations have been recorded since the most recent claim —
    /// the write-back lag. Incremented on every `ObservationRecorded`, reset to
    /// zero on every `ClaimRecorded`. The kernel reports this raw count as a
    /// proprioceptive fact (like freshness); whether the lag is a debt is the
    /// agent's judgment, so no threshold or verdict lives here.
    observations_since_last_claim: u64,
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
                self.observations_since_last_claim += 1;
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
                self.observations_since_last_claim = 0;
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
