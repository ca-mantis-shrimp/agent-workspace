//! Read/view projections over a reconciled workspace: the status, delta,
//! working-set, findings-queue, and transaction-preview surfaces, plus their
//! bounded (`Brief*`) variants. Pure over already-reconciled state — extracted
//! verbatim from `lib.rs`.

use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use crate::model::*;

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

    pub(crate) fn working_set_view_with_uncited_candidates(
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
pub(crate) const WORKING_SET_UNCITED_CANDIDATE_LIMIT: usize = 24;
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
    pub(crate) fn acceptance_readiness(&self, transaction: &Transaction) -> (bool, String) {
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
pub(crate) fn claim_headline(statement: &str, max_chars: usize) -> String {
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
    pub(crate) fn from_ids(ids: impl IntoIterator<Item = u64>) -> Self {
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
pub(crate) const BRIEF_OBJECTIVE_MAX_CHARS: usize = 120;
