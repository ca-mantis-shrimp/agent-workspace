use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Component, Path, PathBuf};
use std::process::Command;

const EVENT_SCHEMA_VERSION: u32 = 2;
const MINIMUM_EVENT_SCHEMA_VERSION: u32 = 1;
const EVENT_LOG_NAME: &str = "events.jsonl";
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
    TransactionNotFound(u64),
    InvalidTransaction(String),
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
            Self::TransactionNotFound(id) => write!(formatter, "transaction {id} not found"),
            Self::InvalidTransaction(message) => {
                write!(formatter, "invalid transaction: {message}")
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Observation {
    pub id: u64,
    pub path: PathBuf,
    pub provider: String,
    pub observed_revision: String,
    #[serde(default)]
    pub selector: ObservationSelector,
    pub observed_input_fingerprint: String,
    #[serde(default)]
    pub observed_container_fingerprint: String,
    #[serde(default)]
    pub native_payload_reference: Option<String>,
    #[serde(default)]
    pub ingested_bytes: usize,
    pub report: FreshnessReport,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ObservationCapture {
    #[serde(flatten)]
    pub observation: Observation,
    pub content: String,
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
    pub recorded_input_fingerprint: String,
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
    pub base_revision: String,
    pub initial_worktree_fingerprint: String,
    pub acceptance_claim_ids: Vec<u64>,
    pub evidence_ids: Vec<u64>,
    pub mutations: Vec<Mutation>,
    pub state: TransactionState,
    pub last_rejection: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WorkspaceStatus {
    pub objective: Option<Objective>,
    pub working_set: Vec<WorkingSetEntry>,
    pub observations: Vec<Observation>,
    pub claims: Vec<Claim>,
    #[serde(default)]
    pub superseded_claims: Vec<Claim>,
    pub evidence: Vec<Evidence>,
    pub transactions: Vec<Transaction>,
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
        input_fingerprint: String,
        #[serde(default)]
        container_fingerprint: Option<String>,
        #[serde(default)]
        native_payload_reference: Option<String>,
        #[serde(default)]
        ingested_bytes: usize,
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
        base_revision: String,
        initial_worktree_fingerprint: String,
        acceptance_claim_ids: Vec<u64>,
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
}

#[derive(Debug)]
pub struct Workspace {
    repository_root: PathBuf,
    workspace_root: PathBuf,
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
        })
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
        let projection = self.project()?;
        let observation_ids: Vec<_> = projection.observations.keys().copied().collect();
        let claim_ids: Vec<_> = projection.claims.keys().copied().collect();
        let evidence_ids: Vec<_> = projection.evidence.keys().copied().collect();
        for observation_id in observation_ids {
            self.reconcile_observation(observation_id)?;
        }
        for claim_id in claim_ids {
            self.reconcile_claim(claim_id)?;
        }
        for evidence_id in evidence_ids {
            self.reconcile_evidence(evidence_id)?;
        }
        let projection = self.project()?;
        let (claims, superseded_claims) = projection
            .claims
            .into_values()
            .partition(|claim| claim.lifecycle.is_active());
        Ok(WorkspaceStatus {
            objective: projection.objective,
            working_set: projection.working_set.into_values().collect(),
            observations: projection.observations.into_values().collect(),
            claims,
            superseded_claims,
            evidence: projection.evidence.into_values().collect(),
            transactions: projection.transactions.into_values().collect(),
        })
    }

    pub fn capture_file_observation(
        &self,
        path: impl AsRef<Path>,
        provider: impl Into<String>,
        selector: ObservationSelector,
        retain_native_payload: bool,
    ) -> Result<ObservationCapture, WorkspaceError> {
        let path = validate_relative_path(path.as_ref())?;
        let resolved_path = resolve_repository_file(&self.repository_root, &path)?;
        let container = fs::read(resolved_path)?;
        let unit = select_observation_unit(&container, &selector)?;
        let content = String::from_utf8(unit.to_vec()).map_err(|_| {
            WorkspaceError::InvalidObservation("selected source is not valid UTF-8".to_owned())
        })?;
        let input_fingerprint = hex_digest(unit);
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
            input_fingerprint,
            container_fingerprint: Some(container_fingerprint),
            native_payload_reference,
            ingested_bytes,
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

    pub fn reconcile_observation(
        &self,
        observation_id: u64,
    ) -> Result<Observation, WorkspaceError> {
        let projection = self.project()?;
        let observation = projection
            .observations
            .get(&observation_id)
            .ok_or(WorkspaceError::ObservationNotFound(observation_id))?;
        let current = read_observation_fingerprints(
            &self.repository_root,
            &observation.path,
            &observation.selector,
        );
        let (current_unit, current_container) = current
            .as_ref()
            .map(|(unit, container)| (Some(unit.as_str()), Some(container.as_str())))
            .unwrap_or((None, None));
        let reconciliation_fingerprint = observation_reconciliation_fingerprint(
            &self.repository_root,
            &observation.path,
            &observation.selector,
            current_unit,
            current_container,
        )?;

        let (freshness, reason) = match &current {
            Ok((unit, container)) if unit == &observation.observed_input_fingerprint => {
                let reason = if container == &observation.observed_container_fingerprint {
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

        self.append(Event::ObservationReconciled {
            observation_id,
            freshness,
            reason,
            reconciliation_fingerprint,
        })?;

        self.project()?
            .observations
            .remove(&observation_id)
            .ok_or(WorkspaceError::ObservationNotFound(observation_id))
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
                    recorded_input_fingerprint: observation.observed_input_fingerprint.clone(),
                    source: ClaimInputSource::SupportingObservation,
                },
            );
        }
        for dependency in declared_dependencies {
            let path = validate_relative_path(dependency)?;
            let input_fingerprint = fingerprint_repository_file(&self.repository_root, &path)?;
            inputs
                .entry((path.clone(), ObservationSelector::WholeFile))
                .or_insert(ClaimInput {
                    path,
                    selector: ObservationSelector::WholeFile,
                    recorded_input_fingerprint: input_fingerprint,
                    source: ClaimInputSource::DeclaredDependency,
                });
        }
        if scope_strategy == ClaimScopeStrategy::ConservativeSiblingFiles {
            for path in conservative_sibling_dependencies(&self.repository_root, &supporting_paths)?
            {
                let input_fingerprint = fingerprint_repository_file(&self.repository_root, &path)?;
                inputs
                    .entry((path.clone(), ObservationSelector::WholeFile))
                    .or_insert(ClaimInput {
                        path,
                        selector: ObservationSelector::WholeFile,
                        recorded_input_fingerprint: input_fingerprint,
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
        let (freshness, reason, fingerprint_inputs) =
            assess_claim_inputs(&self.repository_root, &claim.inputs);
        let reconciliation_fingerprint =
            scoped_reconciliation_fingerprint(&self.repository_root, &fingerprint_inputs)?;
        self.append(Event::ClaimReconciled {
            claim_id,
            freshness,
            reason,
            reconciliation_fingerprint,
        })?;

        self.project()?
            .claims
            .remove(&claim_id)
            .ok_or(WorkspaceError::ClaimNotFound(claim_id))
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
        acceptance_claim_ids: &[u64],
    ) -> Result<Transaction, WorkspaceError> {
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
            base_revision: git_output(&self.repository_root, &["rev-parse", "HEAD"])?,
            initial_worktree_fingerprint: worktree_fingerprint(&self.repository_root)?,
            acceptance_claim_ids: acceptance_claim_ids.to_vec(),
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
        let (freshness, reason, fingerprint_inputs) =
            assess_claim_inputs(&self.repository_root, &evidence.inputs);
        self.append(Event::EvidenceReconciled {
            evidence_id,
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

        let projection = self.project()?;
        let validated = claim_ids.iter().all(|claim_id| {
            projection.claims.get(claim_id).is_some_and(|claim| {
                claim.report.freshness_within_scope == FreshnessWithinScope::Current
                    && evidence_ids.iter().any(|evidence_id| {
                        projection
                            .evidence
                            .get(evidence_id)
                            .is_some_and(|evidence| {
                                evidence.claim_id == *claim_id
                                    && evidence.outcome == EvidenceOutcome::Passed
                                    && evidence.report.freshness_within_scope
                                        == FreshnessWithinScope::Current
                            })
                    })
            })
        });
        if validated {
            self.append(Event::TransactionAccepted { transaction_id })?;
        } else {
            self.append(Event::TransactionAcceptanceRejected {
                transaction_id,
                reason: "acceptance claims lack current passing evidence".to_owned(),
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
        let path = self.event_log_path();
        if !path.exists() {
            return Ok(Projection::default());
        }

        let reader = BufReader::new(File::open(path)?);
        let mut projection = Projection::default();
        for (index, line) in reader.lines().enumerate() {
            let line = line?;
            let record: EventRecord = serde_json::from_str(&line).map_err(|error| {
                WorkspaceError::CorruptLog(format!("line {}: {error}", index + 1))
            })?;
            projection.apply(record)?;
        }
        Ok(projection)
    }
}

#[derive(Default)]
struct Projection {
    objective: Option<Objective>,
    working_set: BTreeMap<u64, WorkingSetEntry>,
    observations: BTreeMap<u64, Observation>,
    claims: BTreeMap<u64, Claim>,
    evidence: BTreeMap<u64, Evidence>,
    transactions: BTreeMap<u64, Transaction>,
    next_observation_id: u64,
    next_claim_id: u64,
    next_evidence_id: u64,
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
                self.working_set.insert(
                    observation_id,
                    WorkingSetEntry {
                        observation_id,
                        reason,
                    },
                );
            }
            Event::ObservationRecorded {
                observation_id,
                path,
                provider,
                git_revision,
                selector,
                input_fingerprint,
                container_fingerprint,
                native_payload_reference,
                ingested_bytes,
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
                        observed_container_fingerprint: container_fingerprint
                            .unwrap_or_else(|| input_fingerprint.clone()),
                        observed_input_fingerprint: input_fingerprint,
                        native_payload_reference,
                        ingested_bytes,
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
                        base_revision,
                        initial_worktree_fingerprint,
                        acceptance_claim_ids,
                        evidence_ids: Vec::new(),
                        mutations: Vec::new(),
                        state: TransactionState::Open,
                        last_rejection: None,
                    },
                );
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

fn assess_claim_inputs(repository_root: &Path, inputs: &[ClaimInput]) -> ClaimAssessment {
    let mut freshness = FreshnessWithinScope::Current;
    let mut reason = "recorded claim inputs unchanged".to_owned();
    let mut fingerprint_inputs = Vec::with_capacity(inputs.len());

    for input in inputs {
        let current = read_observation_fingerprints(repository_root, &input.path, &input.selector)
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

fn fingerprint_repository_file(
    repository_root: &Path,
    relative_path: &Path,
) -> Result<String, WorkspaceError> {
    fingerprint_file(&resolve_repository_file(repository_root, relative_path)?)
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

fn read_observation_fingerprints(
    repository_root: &Path,
    path: &Path,
    selector: &ObservationSelector,
) -> Result<(String, String), WorkspaceError> {
    let container = fs::read(resolve_repository_file(repository_root, path)?)?;
    let unit = select_observation_unit(&container, selector)?;
    Ok((hex_digest(unit), hex_digest(&container)))
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
        match fs::read(repository_root.join(&path)) {
            Ok(bytes) => material.extend(bytes),
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

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn hex_digest(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}
