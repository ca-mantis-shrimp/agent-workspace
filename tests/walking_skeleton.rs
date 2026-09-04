use agent_workspace::{
    Belief, BeliefSupport, Claim, ClaimInputSource, ClaimLifecycle, DeltaStatus, Evidence, Finding,
    FindingDisposition, FindingSeverity, FreshnessWithinScope, Normalizer, Objective, Observation,
    ObservationCapture, ObservationCaptureOptions, ObservationSelector, RevealedFinding,
    RevealedObservation, ScopeCompleteness, ScopeSource, Transaction, TransactionState, Workspace,
    WorkspaceStatus,
};
use serde_json::Value;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

#[test]
fn s1_observation_becomes_stale_after_an_out_of_band_edit() {
    let fixture = GitFixture::new();
    let workspace = fixture.root.path().join("workspace-state");

    let recorded = invoke(&[
        "observe",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--path",
        "src/lib.rs",
        "--provider",
        "fixture-source",
    ]);
    let recorded: Observation = serde_json::from_slice(&recorded.stdout).unwrap();
    assert_eq!(
        recorded.report.freshness_within_scope,
        FreshnessWithinScope::Current
    );
    assert_eq!(recorded.report.reason, "supporting input recorded");
    assert_eq!(
        recorded.report.scope_assurance.source,
        ScopeSource::Declared
    );
    assert_eq!(
        recorded.report.scope_assurance.completeness,
        ScopeCompleteness::AssertedComplete
    );

    fs::write(
        fixture.repository.join("src/lib.rs"),
        "pub fn foo() -> i32 { 2 }\n",
    )
    .unwrap();

    let reconciled = invoke(&[
        "reconcile",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--id",
        &recorded.id.to_string(),
    ]);
    let reconciled: Observation = serde_json::from_slice(&reconciled.stdout).unwrap();
    assert_eq!(
        reconciled.report.freshness_within_scope,
        FreshnessWithinScope::Stale
    );
    assert_eq!(reconciled.report.reason, "supporting input changed");
    assert_ne!(
        recorded
            .report
            .operational_coverage
            .reconciliation_fingerprint,
        reconciled
            .report
            .operational_coverage
            .reconciliation_fingerprint
    );

    let event_log = fs::read_to_string(workspace.join("events.jsonl")).unwrap();
    let records: Vec<Value> = event_log
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0]["sequence"], 0);
    assert_eq!(records[1]["sequence"], 1);
    assert_eq!(records[0]["schema_version"], 2);
    assert_eq!(records[1]["event"]["type"], "observation_reconciled");
}

#[test]
fn an_unverifiable_input_is_unknown_rather_than_stale() {
    let fixture = GitFixture::new();
    let workspace = fixture.root.path().join("workspace-state");
    let recorded = invoke(&[
        "observe",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--path",
        "src/lib.rs",
    ]);
    let recorded: Observation = serde_json::from_slice(&recorded.stdout).unwrap();

    fs::remove_file(fixture.repository.join("src/lib.rs")).unwrap();
    fs::create_dir(fixture.repository.join("src/lib.rs")).unwrap();

    let reconciled = invoke(&[
        "reconcile",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--id",
        &recorded.id.to_string(),
    ]);
    let reconciled: Observation = serde_json::from_slice(&reconciled.stdout).unwrap();
    assert_eq!(
        reconciled.report.freshness_within_scope,
        FreshnessWithinScope::Unknown
    );
    assert_eq!(
        reconciled.report.reason,
        "supporting input could not be verified"
    );
}

#[test]
fn schema_one_fingerprint_records_still_replay() {
    let fixture = GitFixture::new();
    let workspace = fixture.root.path().join("workspace-state");
    let recorded = invoke(&[
        "observe",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--path",
        "src/lib.rs",
    ]);
    let recorded: Observation = serde_json::from_slice(&recorded.stdout).unwrap();
    let event_log_path = workspace.join("events.jsonl");
    let mut legacy_record: Value =
        serde_json::from_str(fs::read_to_string(&event_log_path).unwrap().trim()).unwrap();
    legacy_record["schema_version"] = Value::from(1);
    let event = legacy_record["event"].as_object_mut().unwrap();
    let reconciliation = event.remove("reconciliation_fingerprint").unwrap();
    event.insert("repository_fingerprint".to_owned(), reconciliation);
    event.remove("selector");
    event.remove("container_fingerprint");
    event.remove("native_payload_reference");
    event.remove("ingested_bytes");
    event.remove("model_visible_bytes");
    fs::write(
        &event_log_path,
        format!("{}\n", serde_json::to_string(&legacy_record).unwrap()),
    )
    .unwrap();
    fs::write(
        fixture.repository.join("src/lib.rs"),
        "pub fn foo() -> i32 { 2 }\n",
    )
    .unwrap();

    let reconciled = invoke(&[
        "reconcile",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--id",
        &recorded.id.to_string(),
    ]);
    let reconciled: Observation = serde_json::from_slice(&reconciled.stdout).unwrap();
    assert_eq!(
        reconciled.report.freshness_within_scope,
        FreshnessWithinScope::Stale
    );
    assert_eq!(reconciled.selector, ObservationSelector::WholeFile);
    assert_eq!(
        reconciled.observed_container_fingerprint,
        reconciled.observed_input_fingerprint
    );
    assert_eq!(reconciled.ingested_bytes, 0);
    assert_eq!(reconciled.model_visible_bytes, None);
    assert_eq!(reconciled.native_payload_reference, None);
    let reveal = invoke_failure(&[
        "reveal",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--observation",
        &recorded.id.to_string(),
    ]);
    assert!(String::from_utf8_lossy(&reveal.stderr).contains("legacy observation"));
    let records: Vec<Value> = fs::read_to_string(event_log_path)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(records[0]["schema_version"], 1);
    assert_eq!(records[1]["schema_version"], 2);
}

#[test]
fn s2_declared_dependency_change_makes_claim_stale() {
    let fixture = GitFixture::new();
    let workspace = fixture.root.path().join("workspace-state");
    let observation = invoke(&[
        "observe",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--path",
        "src/lib.rs",
    ]);
    let observation: Observation = serde_json::from_slice(&observation.stdout).unwrap();
    let claim = invoke(&[
        "claim",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--statement",
        "foo delegates to the helper",
        "--observation",
        &observation.id.to_string(),
        "--dependency",
        "src/helper.rs",
    ]);
    let claim: Claim = serde_json::from_slice(&claim.stdout).unwrap();
    assert_eq!(
        claim.report.scope_assurance.completeness,
        ScopeCompleteness::NotAsserted
    );
    assert_eq!(claim.report.operational_coverage.mediated_paths.len(), 2);

    fs::write(
        fixture.repository.join("src/helper.rs"),
        "pub fn helper() -> i32 { 2 }\n",
    )
    .unwrap();
    let reconciled = invoke(&[
        "reconcile-claim",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--id",
        &claim.id.to_string(),
    ]);
    let reconciled: Claim = serde_json::from_slice(&reconciled.stdout).unwrap();
    assert_eq!(
        reconciled.report.freshness_within_scope,
        FreshnessWithinScope::Stale
    );
    assert_eq!(reconciled.report.reason, "recorded claim input changed");
}

#[test]
fn s2_out_of_scope_change_keeps_scoped_claim_current_and_visible() {
    let fixture = GitFixture::new();
    let workspace = fixture.root.path().join("workspace-state");
    let observation = invoke(&[
        "observe",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--path",
        "src/lib.rs",
    ]);
    let observation: Observation = serde_json::from_slice(&observation.stdout).unwrap();
    let claim = invoke(&[
        "claim",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--statement",
        "foo returns one",
        "--observation",
        &observation.id.to_string(),
    ]);
    let claim: Claim = serde_json::from_slice(&claim.stdout).unwrap();

    fs::write(
        fixture.repository.join("src/helper.rs"),
        "pub fn helper() -> i32 { 2 }\n",
    )
    .unwrap();
    let reconciled = invoke(&[
        "reconcile-claim",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--id",
        &claim.id.to_string(),
    ]);
    let reconciled: Claim = serde_json::from_slice(&reconciled.stdout).unwrap();
    assert_eq!(
        reconciled.report.freshness_within_scope,
        FreshnessWithinScope::Current
    );
    assert_eq!(
        reconciled.report.operational_coverage.mediated_paths,
        vec![std::path::PathBuf::from("src/lib.rs")]
    );
    assert_eq!(
        claim.report.operational_coverage.reconciliation_fingerprint,
        reconciled
            .report
            .operational_coverage
            .reconciliation_fingerprint
    );
}

#[test]
fn pre_s3_claim_events_replay_with_declared_scope() {
    let fixture = GitFixture::new();
    let workspace = fixture.root.path().join("workspace-state");
    let observation = invoke(&[
        "observe",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--path",
        "src/lib.rs",
    ]);
    let observation: Observation = serde_json::from_slice(&observation.stdout).unwrap();
    let claim = invoke(&[
        "claim",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--statement",
        "foo returns one",
        "--observation",
        &observation.id.to_string(),
    ]);
    let claim: Claim = serde_json::from_slice(&claim.stdout).unwrap();
    let event_log_path = workspace.join("events.jsonl");
    let old_log = fs::read_to_string(&event_log_path)
        .unwrap()
        .replace("\"scope_strategy\":\"declared\",", "");
    fs::write(&event_log_path, old_log).unwrap();

    let reconciled = invoke(&[
        "reconcile-claim",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--id",
        &claim.id.to_string(),
    ]);
    let reconciled: Claim = serde_json::from_slice(&reconciled.stdout).unwrap();
    assert_eq!(
        reconciled.report.scope_assurance.source,
        ScopeSource::Declared
    );
    assert_eq!(
        reconciled.report.freshness_within_scope,
        FreshnessWithinScope::Current
    );
}

#[test]
fn s3_conservative_sibling_scope_invalidates_on_helper_change() {
    let fixture = GitFixture::new();
    let workspace = fixture.root.path().join("workspace-state");
    let observation = invoke(&[
        "observe",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--path",
        "src/lib.rs",
    ]);
    let observation: Observation = serde_json::from_slice(&observation.stdout).unwrap();
    let claim = invoke(&[
        "claim",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--statement",
        "foo depends on helper behavior",
        "--observation",
        &observation.id.to_string(),
        "--scope",
        "conservative-siblings",
    ]);
    let claim: Claim = serde_json::from_slice(&claim.stdout).unwrap();
    assert_eq!(
        claim.report.scope_assurance.source,
        ScopeSource::Conservative
    );
    assert_eq!(
        claim.report.scope_assurance.completeness,
        ScopeCompleteness::NotAsserted
    );
    assert!(claim.inputs.iter().any(|input| {
        input.path == Path::new("src/helper.rs")
            && input.source == ClaimInputSource::ConservativeDependency
    }));

    fs::write(
        fixture.repository.join("src/helper.rs"),
        "pub fn helper() -> i32 { 2 }\n",
    )
    .unwrap();
    let reconciled = invoke(&[
        "reconcile-claim",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--id",
        &claim.id.to_string(),
    ]);
    let reconciled: Claim = serde_json::from_slice(&reconciled.stdout).unwrap();
    assert_eq!(
        reconciled.report.freshness_within_scope,
        FreshnessWithinScope::Stale
    );
    assert_eq!(reconciled.report.reason, "recorded claim input changed");
}

#[test]
fn claim_creation_reconciles_supporting_observations_before_reporting_current() {
    let fixture = GitFixture::new();
    let workspace = fixture.root.path().join("workspace-state");
    let observation = invoke(&[
        "observe",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--path",
        "src/lib.rs",
    ]);
    let observation: Observation = serde_json::from_slice(&observation.stdout).unwrap();
    fs::write(
        fixture.repository.join("src/lib.rs"),
        "pub fn foo() -> i32 { 2 }\n",
    )
    .unwrap();

    let claim = invoke(&[
        "claim",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--statement",
        "foo returns one",
        "--observation",
        &observation.id.to_string(),
    ]);
    let claim: Claim = serde_json::from_slice(&claim.stdout).unwrap();
    assert_eq!(
        claim.report.freshness_within_scope,
        FreshnessWithinScope::Stale
    );
}

#[test]
fn s4_stale_evidence_cannot_accept_transaction() {
    let fixture = GitFixture::new();
    let workspace = fixture.root.path().join("workspace-state");
    let observation = invoke(&[
        "observe",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--path",
        "src/lib.rs",
    ]);
    let observation: Observation = serde_json::from_slice(&observation.stdout).unwrap();
    let claim = invoke(&[
        "claim",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--statement",
        "foo returns one",
        "--observation",
        &observation.id.to_string(),
    ]);
    let claim: Claim = serde_json::from_slice(&claim.stdout).unwrap();
    let transaction = invoke(&[
        "begin-transaction",
        "--intent",
        "fixture transaction intent",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--claim",
        &claim.id.to_string(),
    ]);
    let transaction: Transaction = serde_json::from_slice(&transaction.stdout).unwrap();
    let evidence = invoke(&[
        "evidence",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--transaction",
        &transaction.id.to_string(),
        "--claim",
        &claim.id.to_string(),
        "--check",
        "fixture-check",
        "--invocation",
        "fixture check",
        "--provider",
        "fixture-runner",
        "--result",
        "passed",
    ]);
    let evidence: Evidence = serde_json::from_slice(&evidence.stdout).unwrap();
    assert_eq!(
        evidence.report.freshness_within_scope,
        FreshnessWithinScope::Current
    );

    fs::write(
        fixture.repository.join("src/lib.rs"),
        "pub fn foo() -> i32 { 2 }\n",
    )
    .unwrap();
    let rejected = invoke(&[
        "accept-transaction",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--id",
        &transaction.id.to_string(),
    ]);
    let rejected: Transaction = serde_json::from_slice(&rejected.stdout).unwrap();
    assert_eq!(rejected.state, TransactionState::Open);
    assert_eq!(
        rejected.last_rejection.as_deref(),
        Some(format!("acceptance claim {} is not current", claim.id).as_str())
    );

    let event_log = fs::read_to_string(workspace.join("events.jsonl")).unwrap();
    assert!(event_log.contains("\"type\":\"evidence_reconciled\""));
    assert!(event_log.contains("\"freshness\":\"stale\""));
    assert!(event_log.contains("\"type\":\"transaction_acceptance_rejected\""));
}

#[test]
fn current_passing_evidence_accepts_transaction() {
    let fixture = GitFixture::new();
    let workspace = fixture.root.path().join("workspace-state");
    let observation = invoke(&[
        "observe",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--path",
        "src/lib.rs",
    ]);
    let observation: Observation = serde_json::from_slice(&observation.stdout).unwrap();
    let claim = invoke(&[
        "claim",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--statement",
        "foo returns one",
        "--observation",
        &observation.id.to_string(),
    ]);
    let claim: Claim = serde_json::from_slice(&claim.stdout).unwrap();
    let transaction = invoke(&[
        "begin-transaction",
        "--intent",
        "fixture transaction intent",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--claim",
        &claim.id.to_string(),
    ]);
    let transaction: Transaction = serde_json::from_slice(&transaction.stdout).unwrap();
    invoke(&[
        "evidence",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--transaction",
        &transaction.id.to_string(),
        "--claim",
        &claim.id.to_string(),
        "--check",
        "fixture-check",
        "--invocation",
        "fixture check",
        "--result",
        "passed",
    ]);
    let accepted = invoke(&[
        "accept-transaction",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--id",
        &transaction.id.to_string(),
    ]);
    let accepted: Transaction = serde_json::from_slice(&accepted.stdout).unwrap();
    assert_eq!(accepted.state, TransactionState::Accepted);
}

#[test]
fn s5_restart_recovers_objective_working_set_and_open_work() {
    let fixture = GitFixture::new();
    let workspace = fixture.root.path().join("workspace-state");
    invoke(&[
        "bind-objective",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--intent",
        "prove restart recovery",
        "--reference",
        "clearhead:kernel",
    ]);
    let observation = invoke(&[
        "observe",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--path",
        "src/lib.rs",
    ]);
    let observation: Observation = serde_json::from_slice(&observation.stdout).unwrap();
    invoke(&[
        "focus",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--observation",
        &observation.id.to_string(),
        "--reason",
        "acceptance target",
    ]);
    let claim = invoke(&[
        "claim",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--statement",
        "foo returns one",
        "--observation",
        &observation.id.to_string(),
    ]);
    let claim: Claim = serde_json::from_slice(&claim.stdout).unwrap();
    let transaction = invoke(&[
        "begin-transaction",
        "--intent",
        "fixture transaction intent",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--claim",
        &claim.id.to_string(),
    ]);
    let transaction: Transaction = serde_json::from_slice(&transaction.stdout).unwrap();
    invoke(&[
        "evidence",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--transaction",
        &transaction.id.to_string(),
        "--claim",
        &claim.id.to_string(),
        "--check",
        "fixture-check",
        "--invocation",
        "fixture check",
        "--result",
        "passed",
    ]);

    let resumed = invoke(&[
        "status",
        "--full",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
    ]);
    let resumed: WorkspaceStatus = serde_json::from_slice(&resumed.stdout).unwrap();
    assert_eq!(
        resumed
            .objective
            .as_ref()
            .map(|objective| objective.intent.as_str()),
        Some("prove restart recovery")
    );
    assert_eq!(resumed.working_set[0].observation_id, observation.id);
    assert_eq!(resumed.observations.len(), 1);
    assert_eq!(resumed.claims.len(), 1);
    assert_eq!(resumed.evidence.len(), 1);
    assert_eq!(resumed.transactions[0].state, TransactionState::Open);
    assert_eq!(
        resumed.evidence[0].report.freshness_within_scope,
        FreshnessWithinScope::Current
    );

    let resumed_again = invoke(&[
        "status",
        "--full",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
    ]);
    let resumed_again: WorkspaceStatus = serde_json::from_slice(&resumed_again.stdout).unwrap();
    assert_eq!(resumed, resumed_again);
}

#[test]
fn s6_clean_transaction_revert_restores_repository_and_freshness() {
    let fixture = GitFixture::new();
    let workspace = fixture.root.path().join("workspace-state");
    let original = fs::read(fixture.repository.join("src/lib.rs")).unwrap();
    let observation = invoke(&[
        "observe",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--path",
        "src/lib.rs",
    ]);
    let observation: Observation = serde_json::from_slice(&observation.stdout).unwrap();
    let claim = invoke(&[
        "claim",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--statement",
        "foo returns one",
        "--observation",
        &observation.id.to_string(),
    ]);
    let claim: Claim = serde_json::from_slice(&claim.stdout).unwrap();
    let transaction = invoke(&[
        "begin-transaction",
        "--intent",
        "fixture transaction intent",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--claim",
        &claim.id.to_string(),
    ]);
    let transaction: Transaction = serde_json::from_slice(&transaction.stdout).unwrap();
    invoke(&[
        "evidence",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--transaction",
        &transaction.id.to_string(),
        "--claim",
        &claim.id.to_string(),
        "--check",
        "fixture-check",
        "--invocation",
        "fixture check",
        "--result",
        "passed",
    ]);

    let applied = invoke(&[
        "apply",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--id",
        &transaction.id.to_string(),
        "--path",
        "src/lib.rs",
        "--content",
        "pub fn foo() -> i32 { 2 }\n",
    ]);
    let applied: Transaction = serde_json::from_slice(&applied.stdout).unwrap();
    assert_eq!(applied.mutations.len(), 1);
    let changed = invoke(&[
        "status",
        "--full",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
    ]);
    let changed: WorkspaceStatus = serde_json::from_slice(&changed.stdout).unwrap();
    assert_eq!(
        changed.claims[0].report.freshness_within_scope,
        FreshnessWithinScope::Stale
    );
    assert_eq!(
        changed.evidence[0].report.freshness_within_scope,
        FreshnessWithinScope::Stale
    );

    let reverted = invoke(&[
        "revert-transaction",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--id",
        &transaction.id.to_string(),
    ]);
    let reverted: Transaction = serde_json::from_slice(&reverted.stdout).unwrap();
    assert_eq!(reverted.state, TransactionState::Reverted);
    assert_eq!(
        fs::read(fixture.repository.join("src/lib.rs")).unwrap(),
        original
    );
    let restored = invoke(&[
        "status",
        "--full",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
    ]);
    let restored: WorkspaceStatus = serde_json::from_slice(&restored.stdout).unwrap();
    assert_eq!(
        restored.claims[0].report.freshness_within_scope,
        FreshnessWithinScope::Current
    );
    assert_eq!(
        restored.evidence[0].report.freshness_within_scope,
        FreshnessWithinScope::Current
    );
}

#[test]
fn multi_path_revert_conflict_changes_nothing() {
    let fixture = GitFixture::new();
    let workspace = fixture.root.path().join("workspace-state");
    let observation = invoke(&[
        "observe",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--path",
        "src/lib.rs",
    ]);
    let observation: Observation = serde_json::from_slice(&observation.stdout).unwrap();
    let claim = invoke(&[
        "claim",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--statement",
        "transaction target",
        "--observation",
        &observation.id.to_string(),
    ]);
    let claim: Claim = serde_json::from_slice(&claim.stdout).unwrap();
    let transaction = invoke(&[
        "begin-transaction",
        "--intent",
        "fixture transaction intent",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--claim",
        &claim.id.to_string(),
    ]);
    let transaction: Transaction = serde_json::from_slice(&transaction.stdout).unwrap();
    let owned_lib = "pub fn foo() -> i32 { 2 }\n";
    let owned_helper = "pub fn helper() -> i32 { 2 }\n";
    for (path, contents) in [("src/lib.rs", owned_lib), ("src/helper.rs", owned_helper)] {
        invoke(&[
            "apply",
            "--repository",
            fixture.repository.to_str().unwrap(),
            "--workspace",
            workspace.to_str().unwrap(),
            "--id",
            &transaction.id.to_string(),
            "--path",
            path,
            "--content",
            contents,
        ]);
    }
    let external_lib = "pub fn foo() -> i32 { 99 }\n";
    fs::write(fixture.repository.join("src/lib.rs"), external_lib).unwrap();

    let failure = invoke_failure(&[
        "revert-transaction",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--id",
        &transaction.id.to_string(),
    ]);
    assert!(String::from_utf8_lossy(&failure.stderr).contains("revert conflict"));
    assert_eq!(
        fs::read_to_string(fixture.repository.join("src/lib.rs")).unwrap(),
        external_lib
    );
    assert_eq!(
        fs::read_to_string(fixture.repository.join("src/helper.rs")).unwrap(),
        owned_helper
    );
    let status = invoke(&[
        "status",
        "--full",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
    ]);
    let status: WorkspaceStatus = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(status.transactions[0].state, TransactionState::Open);
}

#[test]
fn pi_read_capture_persists_the_model_visible_accounting_boundary() {
    let fixture = GitFixture::new();
    let workspace = fixture.root.path().join("workspace-state");
    let captured: ObservationCapture = serde_json::from_slice(
        &invoke(&[
            "observe",
            "--repository",
            fixture.repository.to_str().unwrap(),
            "--workspace",
            workspace.to_str().unwrap(),
            "--path",
            "src/lib.rs",
            "--provider",
            "pi.read",
            "--range",
            "0:3",
            "--model-visible-bytes",
            "57",
        ])
        .stdout,
    )
    .unwrap();

    assert_eq!(captured.observation.provider, "pi.read");
    assert_eq!(captured.observation.ingested_bytes, 3);
    assert_eq!(captured.observation.model_visible_bytes, Some(57));
    assert_eq!(captured.observation.native_payload_reference, None);

    let status: WorkspaceStatus = serde_json::from_slice(
        &invoke(&[
            "status",
            "--repository",
            fixture.repository.to_str().unwrap(),
            "--workspace",
            workspace.to_str().unwrap(),
            "--full",
        ])
        .stdout,
    )
    .unwrap();
    assert_eq!(status.observations[0].model_visible_bytes, Some(57));

    let raced = invoke_failure(&[
        "observe",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--path",
        "src/lib.rs",
        "--range",
        "0:3",
        "--expected-raw-fingerprint",
        &"0".repeat(64),
    ]);
    assert!(
        String::from_utf8_lossy(&raced.stderr)
            .contains("selected input changed after the provider result was finalized")
    );
}

#[test]
fn s7_bounded_perception_reduces_ingestion_with_equal_outcome_and_reveal() {
    let source = bounded_task_source();
    let raw = GitFixture::with_task_source(&source);
    let assisted = GitFixture::with_task_source(&source);
    let raw_workspace = raw.root.path().join("raw-workspace");
    let assisted_workspace = assisted.root.path().join("assisted-workspace");

    let raw_output = invoke(&[
        "observe",
        "--repository",
        raw.repository.to_str().unwrap(),
        "--workspace",
        raw_workspace.to_str().unwrap(),
        "--path",
        "src/task.rs",
    ]);
    let raw_capture: ObservationCapture = serde_json::from_slice(&raw_output.stdout).unwrap();
    assert_eq!(raw_capture.content, source);
    assert_eq!(raw_capture.observation.ingested_bytes, source.len());
    assert_eq!(raw_capture.observation.native_payload_reference, None);
    assert!(!raw_workspace.join("payloads").exists());
    assert_eq!(
        raw_capture.observation.selector,
        ObservationSelector::WholeFile
    );

    let signature = "fn foo(value: i32) -> i32";
    let start = source.find(signature).unwrap();
    let end = start + signature.len();
    let range = format!("{start}:{end}");
    let assisted_output = invoke(&[
        "observe",
        "--repository",
        assisted.repository.to_str().unwrap(),
        "--workspace",
        assisted_workspace.to_str().unwrap(),
        "--path",
        "src/task.rs",
        "--range",
        &range,
        "--retain-payload",
        "true",
    ]);
    let assisted_capture: ObservationCapture =
        serde_json::from_slice(&assisted_output.stdout).unwrap();
    assert_eq!(assisted_capture.content, signature);
    assert_eq!(assisted_capture.observation.ingested_bytes, signature.len());
    assert!(assisted_capture.observation.ingested_bytes < raw_capture.observation.ingested_bytes);
    assert_eq!(
        assisted_capture
            .observation
            .report
            .scope_assurance
            .completeness,
        ScopeCompleteness::NotAsserted
    );

    complete_bounded_task(&raw.repository, &source, &raw_capture.content);
    complete_bounded_task(&assisted.repository, &source, &assisted_capture.content);

    let revealed_after_edit = invoke(&[
        "reveal",
        "--repository",
        assisted.repository.to_str().unwrap(),
        "--workspace",
        assisted_workspace.to_str().unwrap(),
        "--observation",
        &assisted_capture.observation.id.to_string(),
    ]);
    let revealed_after_edit: RevealedObservation =
        serde_json::from_slice(&revealed_after_edit.stdout).unwrap();
    assert_eq!(revealed_after_edit.content, source);
    assert_eq!(revealed_after_edit.ingested_bytes, source.len());
    assert_eq!(
        revealed_after_edit.observed_container_fingerprint,
        assisted_capture.observation.observed_container_fingerprint
    );

    let reconciled = invoke(&[
        "reconcile",
        "--repository",
        assisted.repository.to_str().unwrap(),
        "--workspace",
        assisted_workspace.to_str().unwrap(),
        "--id",
        &assisted_capture.observation.id.to_string(),
    ]);
    let reconciled: Observation = serde_json::from_slice(&reconciled.stdout).unwrap();
    assert_eq!(
        reconciled.report.freshness_within_scope,
        FreshnessWithinScope::Current
    );
    assert_eq!(
        reconciled.report.reason,
        "observed unit unchanged; container changed outside mediated unit"
    );
    assert_ne!(
        reconciled
            .report
            .operational_coverage
            .reconciliation_fingerprint,
        assisted_capture
            .observation
            .report
            .operational_coverage
            .reconciliation_fingerprint
    );

    let claim = invoke(&[
        "claim",
        "--repository",
        assisted.repository.to_str().unwrap(),
        "--workspace",
        assisted_workspace.to_str().unwrap(),
        "--statement",
        "foo accepts and returns i32",
        "--observation",
        &assisted_capture.observation.id.to_string(),
    ]);
    let claim: Claim = serde_json::from_slice(&claim.stdout).unwrap();
    assert_eq!(
        claim.report.freshness_within_scope,
        FreshnessWithinScope::Current
    );
    assert_eq!(
        claim.inputs[0].selector,
        assisted_capture.observation.selector
    );

    let changed_signature = fs::read_to_string(assisted.repository.join("src/task.rs"))
        .unwrap()
        .replace("fn foo(value: i32) -> i32", "fn foo(value: i64) -> i64");
    fs::write(assisted.repository.join("src/task.rs"), changed_signature).unwrap();
    let stale = invoke(&[
        "reconcile-claim",
        "--repository",
        assisted.repository.to_str().unwrap(),
        "--workspace",
        assisted_workspace.to_str().unwrap(),
        "--id",
        &claim.id.to_string(),
    ]);
    let stale: Claim = serde_json::from_slice(&stale.stdout).unwrap();
    assert_eq!(
        stale.report.freshness_within_scope,
        FreshnessWithinScope::Stale
    );
}

#[test]
fn claim_supersession_distinguishes_retired_beliefs_from_active_drift() {
    let fixture = GitFixture::new();
    let workspace = fixture.root.path().join("workspace-state");
    let observation = invoke(&[
        "observe",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--path",
        "src/lib.rs",
    ]);
    let observation: Observation = serde_json::from_slice(&observation.stdout).unwrap();

    let record_claim = |statement: &str, observation_id: u64| {
        let observation_id = observation_id.to_string();
        let output = invoke(&[
            "claim",
            "--repository",
            fixture.repository.to_str().unwrap(),
            "--workspace",
            workspace.to_str().unwrap(),
            "--statement",
            statement,
            "--observation",
            &observation_id,
        ]);
        serde_json::from_slice::<Claim>(&output.stdout).unwrap()
    };
    let retired = record_claim("foo returns one", observation.id);
    let drifted = record_claim("foo has no arguments", observation.id);

    fs::write(
        fixture.repository.join("src/lib.rs"),
        "pub fn foo() -> i32 { 2 }\n",
    )
    .unwrap();
    let replacement_observation = invoke(&[
        "observe",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--path",
        "src/lib.rs",
    ]);
    let replacement_observation: Observation =
        serde_json::from_slice(&replacement_observation.stdout).unwrap();
    let replacement = record_claim("foo returns two", replacement_observation.id);
    let superseded = invoke(&[
        "supersede-claim",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--id",
        &retired.id.to_string(),
        "--claim",
        &replacement.id.to_string(),
        "--reason",
        "the implementation and supported return-value belief changed",
    ]);
    let superseded: Claim = serde_json::from_slice(&superseded.stdout).unwrap();
    assert_eq!(
        superseded.lifecycle,
        ClaimLifecycle::Superseded {
            replacement_claim_id: replacement.id,
            reason: "the implementation and supported return-value belief changed".to_owned(),
        }
    );
    assert_eq!(
        superseded.report.freshness_within_scope,
        FreshnessWithinScope::Stale
    );

    let status = invoke(&[
        "status",
        "--full",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
    ]);
    let status: WorkspaceStatus = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(
        status
            .claims
            .iter()
            .map(|claim| claim.id)
            .collect::<Vec<_>>(),
        vec![drifted.id, replacement.id]
    );
    assert_eq!(
        status.claims[0].report.freshness_within_scope,
        FreshnessWithinScope::Stale
    );
    assert_eq!(status.claims[0].lifecycle, ClaimLifecycle::Active);
    assert_eq!(status.superseded_claims, vec![superseded]);
    let resumed = invoke(&[
        "status",
        "--full",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
    ]);
    let resumed: WorkspaceStatus = serde_json::from_slice(&resumed.stdout).unwrap();
    assert_eq!(resumed, status);

    let reconciled_history = invoke(&[
        "reconcile-claim",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--id",
        &retired.id.to_string(),
    ]);
    let reconciled_history: Claim = serde_json::from_slice(&reconciled_history.stdout).unwrap();
    assert_eq!(reconciled_history, status.superseded_claims[0]);
}

#[test]
fn claim_supersession_rejects_unsafe_lifecycle_transitions_and_replay() {
    let fixture = GitFixture::new();
    let workspace = fixture.root.path().join("workspace-state");
    let observation = invoke(&[
        "observe",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--path",
        "src/lib.rs",
    ]);
    let observation: Observation = serde_json::from_slice(&observation.stdout).unwrap();
    let mut claims = Vec::new();
    for statement in ["claim a", "claim b", "claim c", "claim d"] {
        let output = invoke(&[
            "claim",
            "--repository",
            fixture.repository.to_str().unwrap(),
            "--workspace",
            workspace.to_str().unwrap(),
            "--statement",
            statement,
            "--observation",
            &observation.id.to_string(),
        ]);
        claims.push(serde_json::from_slice::<Claim>(&output.stdout).unwrap());
    }

    for (claim_id, replacement_id, reason, expected) in [
        (claims[0].id, claims[1].id, "", "must not be empty"),
        (
            claims[0].id,
            claims[0].id,
            "self replacement",
            "cannot supersede itself",
        ),
    ] {
        let failure = invoke_failure(&[
            "supersede-claim",
            "--repository",
            fixture.repository.to_str().unwrap(),
            "--workspace",
            workspace.to_str().unwrap(),
            "--id",
            &claim_id.to_string(),
            "--claim",
            &replacement_id.to_string(),
            "--reason",
            reason,
        ]);
        assert!(String::from_utf8_lossy(&failure.stderr).contains(expected));
    }

    invoke(&[
        "supersede-claim",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--id",
        &claims[0].id.to_string(),
        "--claim",
        &claims[1].id.to_string(),
        "--reason",
        "b replaces a",
    ]);
    let superseded_replacement = invoke_failure(&[
        "supersede-claim",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--id",
        &claims[2].id.to_string(),
        "--claim",
        &claims[0].id.to_string(),
        "--reason",
        "invalid replacement",
    ]);
    assert!(
        String::from_utf8_lossy(&superseded_replacement.stderr)
            .contains("replacement claim 0 is superseded")
    );
    invoke(&[
        "supersede-claim",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--id",
        &claims[1].id.to_string(),
        "--claim",
        &claims[2].id.to_string(),
        "--reason",
        "c replaces b",
    ]);

    let historical_transaction = invoke_failure(&[
        "begin-transaction",
        "--intent",
        "fixture transaction intent",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--claim",
        &claims[0].id.to_string(),
    ]);
    assert!(
        String::from_utf8_lossy(&historical_transaction.stderr)
            .contains("acceptance claim 0 is superseded")
    );

    invoke(&[
        "begin-transaction",
        "--intent",
        "fixture transaction intent",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--claim",
        &claims[3].id.to_string(),
    ]);
    let open_transaction = invoke_failure(&[
        "supersede-claim",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--id",
        &claims[3].id.to_string(),
        "--claim",
        &claims[2].id.to_string(),
        "--reason",
        "cannot retire open acceptance",
    ]);
    assert!(
        String::from_utf8_lossy(&open_transaction.stderr)
            .contains("belongs to an open transaction")
    );

    append_raw_event(
        &workspace,
        serde_json::json!({
            "type": "claim_superseded",
            "claim_id": claims[3].id,
            "replacement_claim_id": claims[2].id,
            "reason": "invalid concurrent retirement"
        }),
    );
    let corrupt = invoke_failure(&[
        "status",
        "--full",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
    ]);
    assert!(String::from_utf8_lossy(&corrupt.stderr).contains("belongs to an open transaction"));
}

#[cfg(unix)]
#[test]
fn bounded_observation_rejects_repository_and_payload_symlink_escapes() {
    use std::os::unix::fs::symlink;

    let fixture = GitFixture::new();
    let outside_source = fixture.root.path().join("outside-secret.rs");
    fs::write(&outside_source, "const SECRET: &str = \"do-not-retain\";\n").unwrap();
    symlink(&outside_source, fixture.repository.join("src/leak.rs")).unwrap();
    let workspace = fixture.root.path().join("workspace-state");

    let escaped_source = invoke_failure(&[
        "observe",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--path",
        "src/leak.rs",
        "--retain-payload",
        "true",
    ]);
    assert!(
        String::from_utf8_lossy(&escaped_source.stderr)
            .contains("path must be repository-relative")
    );

    fs::create_dir_all(&workspace).unwrap();
    let outside_payloads = fixture.root.path().join("outside-payloads");
    fs::create_dir(&outside_payloads).unwrap();
    symlink(&outside_payloads, workspace.join("payloads")).unwrap();
    let escaped_payload = invoke_failure(&[
        "observe",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--path",
        "src/lib.rs",
        "--retain-payload",
        "true",
    ]);
    assert!(
        String::from_utf8_lossy(&escaped_payload.stderr)
            .contains("payload storage is not a regular directory")
    );
    assert_eq!(fs::read_dir(outside_payloads).unwrap().count(), 0);
}

#[test]
fn equal_bytes_at_different_ranges_have_distinct_coverage() {
    let source = "fn foo(value: i32) -> i32 { value }\nfn bar(value: i32) -> i32 { value }\n";
    let fixture = GitFixture::with_task_source(source);
    let workspace = fixture.root.path().join("workspace-state");
    let first = source.find("i32").unwrap();
    let second = source[first + 3..].find("i32").unwrap() + first + 3;

    let mut captures = Vec::new();
    for start in [first, second] {
        let range = format!("{start}:{}", start + 3);
        let output = invoke(&[
            "observe",
            "--repository",
            fixture.repository.to_str().unwrap(),
            "--workspace",
            workspace.to_str().unwrap(),
            "--path",
            "src/task.rs",
            "--range",
            &range,
        ]);
        captures.push(serde_json::from_slice::<ObservationCapture>(&output.stdout).unwrap());
    }

    assert_eq!(
        captures[0].observation.observed_input_fingerprint,
        captures[1].observation.observed_input_fingerprint
    );
    assert_ne!(
        captures[0]
            .observation
            .report
            .operational_coverage
            .mediated_units,
        captures[1]
            .observation
            .report
            .operational_coverage
            .mediated_units
    );
    assert_ne!(
        captures[0]
            .observation
            .report
            .operational_coverage
            .reconciliation_fingerprint,
        captures[1]
            .observation
            .report
            .operational_coverage
            .reconciliation_fingerprint
    );

    let claims: Vec<Claim> = captures
        .iter()
        .map(|capture| {
            let observation_id = capture.observation.id.to_string();
            let output = invoke(&[
                "claim",
                "--repository",
                fixture.repository.to_str().unwrap(),
                "--workspace",
                workspace.to_str().unwrap(),
                "--statement",
                "selected type is i32",
                "--observation",
                &observation_id,
            ]);
            serde_json::from_slice(&output.stdout).unwrap()
        })
        .collect();
    assert_ne!(
        claims[0]
            .report
            .operational_coverage
            .reconciliation_fingerprint,
        claims[1]
            .report
            .operational_coverage
            .reconciliation_fingerprint
    );
}

fn bounded_task_source() -> String {
    let mut source = "fn foo(value: i32) -> i32 { value + 1 }\n".to_owned();
    for index in 0..100 {
        source.push_str(&format!(
            "fn padding_{index}(value: i32) -> i32 {{ value + {index} }}\n"
        ));
    }
    source.push_str("fn main() {}\n");
    source
}

fn complete_bounded_task(repository: &Path, source: &str, perceived_source: &str) {
    let declaration = perceived_source
        .split_once("fn ")
        .map(|(_, declaration)| declaration)
        .unwrap();
    let function_name = declaration.split_once('(').map(|(name, _)| name).unwrap();
    let return_type = declaration
        .split_once("->")
        .map(|(_, return_type)| return_type.trim())
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap();
    let task_main = format!("fn main() {{ let _: {return_type} = {function_name}(41); }}");
    let completed = source.replace("fn main() {}", &task_main);
    let source_path = repository.join("src/task.rs");
    fs::write(&source_path, completed).unwrap();
    let binary = repository.join("task-bin");
    let compile = Command::new("rustc")
        .arg(&source_path)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "task did not compile: {}",
        String::from_utf8_lossy(&compile.stderr)
    );
    assert!(Command::new(binary).status().unwrap().success());
}

#[test]
fn checkpoint_delta_reports_recorded_superseded_and_staled_since_a_line() {
    let fixture = GitFixture::new();
    let workspace = fixture.root.path().join("workspace-state");
    let repo = fixture.repository.to_str().unwrap().to_owned();
    let ws = workspace.to_str().unwrap().to_owned();

    let observe = |path: &str| -> Observation {
        let out = invoke(&[
            "observe",
            "--repository",
            &repo,
            "--workspace",
            &ws,
            "--path",
            path,
        ]);
        serde_json::from_slice(&out.stdout).unwrap()
    };
    let claim = |statement: &str, observation_id: u64| -> Claim {
        let observation_id = observation_id.to_string();
        let out = invoke(&[
            "claim",
            "--repository",
            &repo,
            "--workspace",
            &ws,
            "--statement",
            statement,
            "--observation",
            &observation_id,
        ]);
        serde_json::from_slice(&out.stdout).unwrap()
    };

    let helper_observation = observe("src/helper.rs");
    let lib_observation = observe("src/lib.rs");
    let will_stale = claim("helper returns one", helper_observation.id);
    let will_retire = claim("foo returns one", lib_observation.id);

    // Draw the line at a clean, all-current base.
    invoke(&[
        "checkpoint",
        "--repository",
        &repo,
        "--workspace",
        &ws,
        "--label",
        "baseline",
        "--note",
        "before the changes",
    ]);

    // (a) stale a claim by editing its input out of band,
    fs::write(
        fixture.repository.join("src/helper.rs"),
        "pub fn helper() -> i32 { 2 }\n",
    )
    .unwrap();
    // (b) record a new belief, and (c) supersede an old one with it.
    let successor = claim("foo still returns one after review", lib_observation.id);
    invoke(&[
        "supersede-claim",
        "--repository",
        &repo,
        "--workspace",
        &ws,
        "--id",
        &will_retire.id.to_string(),
        "--claim",
        &successor.id.to_string(),
        "--reason",
        "consumed by review",
    ]);

    let delta = invoke(&[
        "delta",
        "--full",
        "--repository",
        &repo,
        "--workspace",
        &ws,
        "--since",
        "baseline",
    ]);
    let delta: DeltaStatus = serde_json::from_slice(&delta.stdout).unwrap();

    assert_eq!(delta.checkpoint.label, "baseline");
    assert_eq!(delta.checkpoint.note.as_deref(), Some("before the changes"));
    assert_eq!(claim_ids(&delta.claims_recorded), vec![successor.id]);
    assert_eq!(claim_ids(&delta.claims_superseded), vec![will_retire.id]);
    assert_eq!(claim_ids(&delta.claims_staled), vec![will_stale.id]);
    assert_eq!(
        delta.claims_staled[0].report.freshness_within_scope,
        FreshnessWithinScope::Stale
    );
    // A reused observation, an untouched objective, and no transactions must not
    // masquerade as changes.
    assert!(delta.observations_recorded.is_empty());
    assert!(delta.objective_change.is_none());
    assert!(delta.transactions_opened.is_empty());
    assert!(delta.transactions_closed.is_empty());
}

#[test]
fn delta_without_a_label_uses_the_latest_checkpoint() {
    let fixture = GitFixture::new();
    let workspace = fixture.root.path().join("workspace-state");
    let repo = fixture.repository.to_str().unwrap().to_owned();
    let ws = workspace.to_str().unwrap().to_owned();

    let lib_observation: Observation = serde_json::from_slice(
        &invoke(&[
            "observe",
            "--repository",
            &repo,
            "--workspace",
            &ws,
            "--path",
            "src/lib.rs",
        ])
        .stdout,
    )
    .unwrap();
    let claim = |statement: &str| -> Claim {
        let observation_id = lib_observation.id.to_string();
        serde_json::from_slice(
            &invoke(&[
                "claim",
                "--repository",
                &repo,
                "--workspace",
                &ws,
                "--statement",
                statement,
                "--observation",
                &observation_id,
            ])
            .stdout,
        )
        .unwrap()
    };
    let checkpoint = |label: &str| {
        invoke(&[
            "checkpoint",
            "--repository",
            &repo,
            "--workspace",
            &ws,
            "--label",
            label,
        ]);
    };

    checkpoint("first");
    let middle = claim("foo returns one");
    checkpoint("second");
    let late = claim("foo is a function");

    let latest: DeltaStatus = serde_json::from_slice(
        &invoke(&["delta", "--full", "--repository", &repo, "--workspace", &ws]).stdout,
    )
    .unwrap();
    assert_eq!(latest.checkpoint.label, "second");
    assert_eq!(claim_ids(&latest.claims_recorded), vec![late.id]);

    let from_first: DeltaStatus = serde_json::from_slice(
        &invoke(&[
            "delta",
            "--full",
            "--repository",
            &repo,
            "--workspace",
            &ws,
            "--since",
            "first",
        ])
        .stdout,
    )
    .unwrap();
    assert_eq!(
        claim_ids(&from_first.claims_recorded),
        vec![middle.id, late.id]
    );

    let brief_out = invoke(&[
        "delta",
        "--compact",
        "--repository",
        &repo,
        "--workspace",
        &ws,
    ]);
    let brief: Value = serde_json::from_slice(&brief_out.stdout).unwrap();
    assert_eq!(brief["checkpoint"]["label"], "second");
    assert_eq!(brief["claims_recorded"]["total"], 1);
    assert_eq!(brief["claims_recorded"]["recent_ids"][0], late.id);
    assert!(brief["claims_recorded"].get("statement").is_none());
    assert!(brief_out.stdout.len() < 1_800);
}

#[test]
fn checkpoint_rejects_duplicate_labels_and_records_objective_change() {
    let fixture = GitFixture::new();
    let workspace = fixture.root.path().join("workspace-state");
    let repo = fixture.repository.to_str().unwrap().to_owned();
    let ws = workspace.to_str().unwrap().to_owned();

    invoke(&[
        "bind-objective",
        "--repository",
        &repo,
        "--workspace",
        &ws,
        "--intent",
        "ship the delta view",
    ]);
    invoke(&[
        "checkpoint",
        "--repository",
        &repo,
        "--workspace",
        &ws,
        "--label",
        "start",
    ]);

    let duplicate = invoke_failure(&[
        "checkpoint",
        "--repository",
        &repo,
        "--workspace",
        &ws,
        "--label",
        "start",
    ]);
    assert!(String::from_utf8_lossy(&duplicate.stderr).contains("already used"));

    invoke(&[
        "bind-objective",
        "--repository",
        &repo,
        "--workspace",
        &ws,
        "--intent",
        "ship the read hook",
    ]);

    let delta: DeltaStatus = serde_json::from_slice(
        &invoke(&["delta", "--full", "--repository", &repo, "--workspace", &ws]).stdout,
    )
    .unwrap();
    assert_eq!(
        delta.checkpoint.objective,
        Some(Objective {
            intent: "ship the delta view".to_owned(),
            external_reference: None,
        })
    );
    let change = delta.objective_change.expect("objective changed");
    assert_eq!(change.before.unwrap().intent, "ship the delta view");
    assert_eq!(change.after.unwrap().intent, "ship the read hook");
}

#[test]
fn delta_without_any_checkpoint_fails_clearly() {
    let fixture = GitFixture::new();
    let workspace = fixture.root.path().join("workspace-state");
    let failure = invoke_failure(&[
        "delta",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
    ]);
    assert!(String::from_utf8_lossy(&failure.stderr).contains("not found"));
}

/// The default `status` is the brief orientation surface, not the full audit
/// dump. It must carry the objective, every active claim's belief with its
/// freshness, and counts — while dropping the heavy per-entity coverage that
/// only `--full` returns — and it must be dramatically smaller than `--full`
/// over the same workspace (the whole reason it exists).
#[test]
fn default_status_is_the_brief_orientation_surface() {
    let fixture = GitFixture::new();
    let workspace = fixture.root.path().join("workspace-state");
    let repo = fixture.repository.to_str().unwrap().to_owned();
    let ws = workspace.to_str().unwrap().to_owned();

    invoke(&[
        "bind-objective",
        "--repository",
        &repo,
        "--workspace",
        &ws,
        "--intent",
        "make status cheap to consult",
    ]);
    let out = invoke(&[
        "observe",
        "--repository",
        &repo,
        "--workspace",
        &ws,
        "--path",
        "src/lib.rs",
    ]);
    let observation: Observation = serde_json::from_slice(&out.stdout).unwrap();
    // A short claim (stays whole in brief)...
    invoke(&[
        "claim",
        "--repository",
        &repo,
        "--workspace",
        &ws,
        "--statement",
        "lib.rs defines the kernel",
        "--observation",
        &observation.id.to_string(),
    ]);
    // ...and a long thesis-first claim (truncated to a headline in brief).
    let long_statement = "Truncation headline check: this deliberately long claim leads \
        with its thesis and then continues well past the brief headline budget with a \
        great deal of additional implementation detail that a scan does not need up front, \
        only on --full.";
    invoke(&[
        "claim",
        "--repository",
        &repo,
        "--workspace",
        &ws,
        "--statement",
        long_statement,
        "--observation",
        &observation.id.to_string(),
    ]);

    // Default output: brief. Assert on the actual wire JSON an agent consumes
    // (BriefStatus is a Serialize-only output projection). It carries the
    // orientation surface...
    let brief_out = invoke(&["status", "--repository", &repo, "--workspace", &ws]);
    let brief: Value = serde_json::from_slice(&brief_out.stdout).unwrap();
    assert_eq!(brief["objective"]["intent"], "make status cheap to consult");
    assert_eq!(brief["claims"].as_array().unwrap().len(), 2);
    // Short claim: headline is the whole statement, no ellipsis.
    assert_eq!(brief["claims"][0]["headline"], "lib.rs defines the kernel");
    assert_eq!(
        brief["claims"][0]["freshness"], "current",
        "a just-recorded claim over an unchanged input reads current"
    );
    // Long claim: headline is truncated to a scannable thesis with a trailing ….
    let headline = brief["claims"][1]["headline"].as_str().unwrap();
    assert!(headline.ends_with('…'), "truncated headline marks the cut");
    assert!(
        headline.chars().count() <= 161,
        "headline stays within the budget (+ ellipsis): {headline:?}"
    );
    assert!(
        headline.starts_with("Truncation headline check:"),
        "thesis-first claims read cleanly in the headline: {headline:?}"
    );
    assert!(
        (headline.chars().count() as usize) < long_statement.chars().count(),
        "the headline is shorter than the full statement it stands in for"
    );
    assert_eq!(brief["counts"]["active_claims"], 2);
    assert_eq!(brief["counts"]["observations"], 1);
    assert_eq!(brief["counts"]["freshness"]["current"], 2);
    assert_eq!(brief["counts"]["freshness"]["stale"], 0);
    // The heavy per-claim coverage that only --full returns is absent.
    assert!(brief["claims"][0].get("inputs").is_none());
    assert!(brief.get("observations").is_none());

    // ...and the full audit dump deserializes as the heavy surface brief omits,
    // carrying the untruncated statement the headline stands in for.
    let full_out = invoke(&[
        "status",
        "--full",
        "--repository",
        &repo,
        "--workspace",
        &ws,
    ]);
    let full: WorkspaceStatus = serde_json::from_slice(&full_out.stdout).unwrap();
    assert_eq!(full.observations.len(), 1);
    assert_eq!(full.claims.len(), 2);
    assert_eq!(full.claims[1].statement, long_statement);

    // Brief deserializing as WorkspaceStatus would fail (it lacks the heavy
    // fields), and it is a fraction of the full dump's size — the point of it.
    assert!(serde_json::from_slice::<WorkspaceStatus>(&brief_out.stdout).is_err());
    assert!(
        brief_out.stdout.len() * 3 < full_out.stdout.len(),
        "brief ({} bytes) must be far smaller than full ({} bytes)",
        brief_out.stdout.len(),
        full_out.stdout.len()
    );
}

/// Model-entry status remains hard-bounded as the standing belief set grows.
/// Truncation is explicit rather than silently pretending all active claims fit.
#[test]
fn brief_status_caps_claims_and_compact_transport_fits_hook_preview() {
    let fixture = GitFixture::new();
    let workspace = fixture.root.path().join("workspace-state");
    let repo = fixture.repository.to_str().unwrap().to_owned();
    let ws = workspace.to_str().unwrap().to_owned();
    let observation: Observation = serde_json::from_slice(
        &invoke(&[
            "observe",
            "--repository",
            &repo,
            "--workspace",
            &ws,
            "--path",
            "src/lib.rs",
        ])
        .stdout,
    )
    .unwrap();

    for index in 0..12 {
        invoke(&[
            "claim",
            "--repository",
            &repo,
            "--workspace",
            &ws,
            "--statement",
            &format!(
                "claim {index}: a deliberately long thesis that should be bounded before model entry while the full belief remains available in audit status"
            ),
            "--observation",
            &observation.id.to_string(),
        ]);
    }

    let output = invoke(&[
        "status",
        "--compact",
        "--repository",
        &repo,
        "--workspace",
        &ws,
    ]);
    let brief: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(brief["claims"].as_array().unwrap().len(), 8);
    assert_eq!(brief["claims_omitted"], 4);
    assert_eq!(brief["counts"]["active_claims"], 12);
    assert!(
        output.stdout.len() < 1_800,
        "compact bounded status must fit Claude's inline hook preview: {} bytes",
        output.stdout.len()
    );
}

/// Teeth for the single-pass I/O guarantee itself: a settled `status` reads the
/// event log a small constant number of times, NOT once per entity. Built with
/// many observations + claims so the old per-entity-`project()` design would
/// read the log ~2×(entities) times; single-pass reads it once. Uses an
/// in-process `Workspace` to read the diagnostic `event_log_reads` counter that
/// a subprocess could not expose. This is the assertion that fails on a
/// regression to multi-pass — the strace-measured 214→1 win, made permanent.
#[test]
fn single_pass_status_reads_the_event_log_a_constant_number_of_times() {
    let files: Vec<(String, String)> = (0..8)
        .map(|index| (format!("src/f{index}.txt"), format!("content {index}\n")))
        .collect();
    let file_refs: Vec<(&str, &str)> = files
        .iter()
        .map(|(path, contents)| (path.as_str(), contents.as_str()))
        .collect();
    let fixture = GitFixture::with_files(&file_refs);
    let workspace = fixture.root.path().join("workspace-state");
    let repo = fixture.repository.to_str().unwrap().to_owned();
    let ws = workspace.to_str().unwrap().to_owned();

    // 8 observations, each with a claim — 16 reconcilable entities.
    for (path, _) in &files {
        let observation: Observation = serde_json::from_slice(
            &invoke(&[
                "observe",
                "--repository",
                &repo,
                "--workspace",
                &ws,
                "--path",
                path,
            ])
            .stdout,
        )
        .unwrap();
        invoke(&[
            "claim",
            "--repository",
            &repo,
            "--workspace",
            &ws,
            "--statement",
            &format!("claim about {path}"),
            "--observation",
            &observation.id.to_string(),
        ]);
    }
    // Settle with the exhaustive audit path so the measured status is a pure
    // no-op (nothing left to append). The default bounded status intentionally
    // reconciles only the claims it serves.
    invoke(&[
        "status",
        "--full",
        "--repository",
        &repo,
        "--workspace",
        &ws,
    ]);

    // Drive resume_status in-process over the settled, unchanged workspace and
    // read the diagnostic counter. Single-pass reads the log exactly once
    // (project the snapshot; nothing to append, so the snapshot is reused).
    let handle = Workspace::open(&fixture.repository, &workspace).unwrap();
    assert_eq!(handle.event_log_reads(), 0, "counter starts at zero");
    handle.resume_status().unwrap();
    let reads = handle.event_log_reads();
    assert!(
        reads <= 2,
        "a settled status must read the log a small constant number of times, \
         got {reads} over 16 entities (multi-pass would be ~17+)"
    );
}

/// Single-pass `status` projects the log once and reconciles every entity
/// against that one snapshot. This guards the two properties that could break:
/// (1) suppression must hold across *many* entities at once — a settled repeat
/// status appends nothing even with several observations and claims; and
/// (2) entities stay independent — an out-of-band edit to one file stales only
/// the entity that observed it, never its neighbours (the shared snapshot must
/// not smear one verdict across the batch). Detection surviving the reused
/// snapshot is exactly the F9 guard under the single-pass optimization.
#[test]
fn single_pass_status_suppresses_at_scale_and_isolates_edits() {
    let fixture = GitFixture::with_files(&[
        ("src/a.txt", "alpha\n"),
        ("src/b.txt", "bravo\n"),
        ("src/c.txt", "charlie\n"),
    ]);
    let workspace = fixture.root.path().join("workspace-state");
    let repo = fixture.repository.to_str().unwrap().to_owned();
    let ws = workspace.to_str().unwrap().to_owned();
    let log_path = workspace.join("events.jsonl");
    let log_len = || {
        fs::read_to_string(&log_path)
            .unwrap()
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count()
    };

    // Observe all three files and stake a claim on each.
    let mut observation_ids = Vec::new();
    for path in ["src/a.txt", "src/b.txt", "src/c.txt"] {
        let observation: Observation = serde_json::from_slice(
            &invoke(&[
                "observe",
                "--repository",
                &repo,
                "--workspace",
                &ws,
                "--path",
                path,
            ])
            .stdout,
        )
        .unwrap();
        invoke(&[
            "claim",
            "--repository",
            &repo,
            "--workspace",
            &ws,
            "--statement",
            &format!("claim about {path}"),
            "--observation",
            &observation.id.to_string(),
        ]);
        observation_ids.push(observation.id);
    }

    // Settle, then prove a repeat status over the unchanged workspace appends
    // nothing — suppression holds across all six entities in one pass.
    invoke(&["status", "--repository", &repo, "--workspace", &ws]);
    let settled = log_len();
    invoke(&["status", "--repository", &repo, "--workspace", &ws]);
    assert_eq!(
        log_len(),
        settled,
        "a settled repeat status must append nothing across many entities"
    );

    // Edit exactly one observed file out of band.
    fs::write(fixture.repository.join("src/b.txt"), "bravo edited\n").unwrap();

    let full: WorkspaceStatus = serde_json::from_slice(
        &invoke(&[
            "status",
            "--full",
            "--repository",
            &repo,
            "--workspace",
            &ws,
        ])
        .stdout,
    )
    .unwrap();
    let freshness_of = |observation_id: u64| {
        full.observations
            .iter()
            .find(|observation| observation.id == observation_id)
            .map(|observation| observation.report.freshness_within_scope.clone())
            .unwrap_or_else(|| panic!("observation {observation_id} missing from status"))
    };
    // The edited file's observation is detected as stale...
    assert_eq!(
        freshness_of(observation_ids[1]),
        FreshnessWithinScope::Stale
    );
    // ...and its untouched neighbours stay current — no cross-contamination.
    assert_eq!(
        freshness_of(observation_ids[0]),
        FreshnessWithinScope::Current
    );
    assert_eq!(
        freshness_of(observation_ids[2]),
        FreshnessWithinScope::Current
    );
    // Claims mirror their observed files: only b's claim stales.
    let claim_freshness: Vec<_> = full
        .claims
        .iter()
        .map(|claim| {
            (
                claim.statement.clone(),
                claim.report.freshness_within_scope.clone(),
            )
        })
        .collect();
    let fresh_for = |path: &str| {
        claim_freshness
            .iter()
            .find(|(statement, _)| statement == &format!("claim about {path}"))
            .map(|(_, freshness)| freshness.clone())
            .unwrap()
    };
    assert_eq!(fresh_for("src/b.txt"), FreshnessWithinScope::Stale);
    assert_eq!(fresh_for("src/a.txt"), FreshnessWithinScope::Current);
    assert_eq!(fresh_for("src/c.txt"), FreshnessWithinScope::Current);
}

#[test]
fn status_suppresses_redundant_reconcile_events() {
    let fixture = GitFixture::new();
    let workspace = fixture.root.path().join("workspace-state");
    let repo = fixture.repository.to_str().unwrap().to_owned();
    let ws = workspace.to_str().unwrap().to_owned();
    let log_path = workspace.join("events.jsonl");
    let log_len = || {
        fs::read_to_string(&log_path)
            .unwrap()
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count()
    };

    let out = invoke(&[
        "observe",
        "--repository",
        &repo,
        "--workspace",
        &ws,
        "--path",
        "src/lib.rs",
    ]);
    let observation: Observation = serde_json::from_slice(&out.stdout).unwrap();
    let out = invoke(&[
        "claim",
        "--repository",
        &repo,
        "--workspace",
        &ws,
        "--statement",
        "foo returns one",
        "--observation",
        &observation.id.to_string(),
    ]);
    let claim: Claim = serde_json::from_slice(&out.stdout).unwrap();
    let out = invoke(&[
        "begin-transaction",
        "--intent",
        "fixture transaction intent",
        "--repository",
        &repo,
        "--workspace",
        &ws,
        "--claim",
        &claim.id.to_string(),
    ]);
    let transaction: Transaction = serde_json::from_slice(&out.stdout).unwrap();
    let out = invoke(&[
        "evidence",
        "--repository",
        &repo,
        "--workspace",
        &ws,
        "--transaction",
        &transaction.id.to_string(),
        "--claim",
        &claim.id.to_string(),
        "--check",
        "fixture-check",
        "--invocation",
        "fixture check",
        "--result",
        "passed",
    ]);
    let _: Evidence = serde_json::from_slice(&out.stdout).unwrap();
    let setup_len = log_len();

    // First status: only the observation normalizes (record-time reason
    // "supporting input recorded" -> "supporting input unchanged"). Claims and
    // evidence are recorded with the same assessment reconciliation uses, so
    // their reconciles are already no-ops.
    let out = invoke(&[
        "status",
        "--full",
        "--repository",
        &repo,
        "--workspace",
        &ws,
    ]);
    let first: WorkspaceStatus = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        first.observations[0].report.reason,
        "supporting input unchanged"
    );
    assert_eq!(log_len(), setup_len + 1);

    // Second status over unchanged inputs: nothing left to persist.
    let out = invoke(&[
        "status",
        "--full",
        "--repository",
        &repo,
        "--workspace",
        &ws,
    ]);
    let second: WorkspaceStatus = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(second, first);
    assert_eq!(log_len(), setup_len + 1);

    // A third status stays silent too: suppression is stable, not one-shot.
    let out = invoke(&[
        "status",
        "--full",
        "--repository",
        &repo,
        "--workspace",
        &ws,
    ]);
    let third: WorkspaceStatus = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(third, second);
    assert_eq!(log_len(), setup_len + 1);
}

#[test]
fn suppressed_status_still_recomputes_and_emits_changed_verdicts() {
    let fixture = GitFixture::new();
    let workspace = fixture.root.path().join("workspace-state");
    let repo = fixture.repository.to_str().unwrap().to_owned();
    let ws = workspace.to_str().unwrap().to_owned();
    let log_path = workspace.join("events.jsonl");
    let log_len = || {
        fs::read_to_string(&log_path)
            .unwrap()
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count()
    };

    let out = invoke(&[
        "observe",
        "--repository",
        &repo,
        "--workspace",
        &ws,
        "--path",
        "src/lib.rs",
    ]);
    let observation: Observation = serde_json::from_slice(&out.stdout).unwrap();
    let out = invoke(&[
        "claim",
        "--repository",
        &repo,
        "--workspace",
        &ws,
        "--statement",
        "foo returns one",
        "--observation",
        &observation.id.to_string(),
    ]);
    let _: Claim = serde_json::from_slice(&out.stdout).unwrap();
    // Silence first: prove the baseline is suppressed.
    let out = invoke(&[
        "status",
        "--full",
        "--repository",
        &repo,
        "--workspace",
        &ws,
    ]);
    let _: WorkspaceStatus = serde_json::from_slice(&out.stdout).unwrap();
    let suppressed_len = log_len();
    let out = invoke(&[
        "status",
        "--full",
        "--repository",
        &repo,
        "--workspace",
        &ws,
    ]);
    let _: WorkspaceStatus = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(log_len(), suppressed_len);

    // Suppression must not degrade into replay (F9): an out-of-band edit is
    // still detected and persisted on the next status.
    fs::write(
        fixture.repository.join("src/lib.rs"),
        "pub fn foo() -> i32 { 2 }\n",
    )
    .unwrap();
    let out = invoke(&[
        "status",
        "--full",
        "--repository",
        &repo,
        "--workspace",
        &ws,
    ]);
    let changed: WorkspaceStatus = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        changed.observations[0].report.freshness_within_scope,
        FreshnessWithinScope::Stale
    );
    assert_eq!(
        changed.claims[0].report.freshness_within_scope,
        FreshnessWithinScope::Stale
    );
    assert_eq!(log_len(), suppressed_len + 2);
    let records: Vec<Value> = fs::read_to_string(&log_path)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(
        records[records.len() - 1]["event"]["type"],
        "claim_reconciled"
    );
    assert_eq!(
        records[records.len() - 2]["event"]["type"],
        "observation_reconciled"
    );

    // The changed verdicts are persisted, so the following status is silent
    // again — and reports the stale verdicts it just recomputed.
    let out = invoke(&[
        "status",
        "--full",
        "--repository",
        &repo,
        "--workspace",
        &ws,
    ]);
    let resettle: WorkspaceStatus = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(resettle, changed);
    assert_eq!(log_len(), suppressed_len + 2);
}

/// Writer locking: many CLI processes hitting one workspace at once must
/// serialize, never interleave appends into a corrupt log. Without the
/// boundary lock this races two ways — a duplicate sequence (CorruptLog on
/// replay) or a duplicate entity id (silent overwrite) — and both are caught
/// below: the sequences must be exactly 0..=N contiguous and unique, and every
/// observe must land a distinct observation id.
#[test]
fn concurrent_writers_serialize_without_corrupting_the_log() {
    const WRITERS: usize = 16;

    let fixture = GitFixture::new();
    let workspace = fixture.root.path().join("workspace-state");
    let repo = fixture.repository.to_str().unwrap().to_owned();
    let ws = workspace.to_str().unwrap().to_owned();

    // One sequential event establishes the workspace (sequence 0).
    invoke(&[
        "bind-objective",
        "--repository",
        &repo,
        "--workspace",
        &ws,
        "--intent",
        "concurrency probe",
    ]);

    // Fire WRITERS observe commands at the same workspace simultaneously.
    let outputs: Vec<Output> = (0..WRITERS)
        .map(|_| {
            let repo = repo.clone();
            let ws = ws.clone();
            std::thread::spawn(move || {
                Command::new(env!("CARGO_BIN_EXE_agent-workspace"))
                    .args([
                        "observe",
                        "--repository",
                        &repo,
                        "--workspace",
                        &ws,
                        "--path",
                        "src/lib.rs",
                    ])
                    .output()
                    .unwrap()
            })
        })
        .collect::<Vec<_>>()
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect();
    for output in &outputs {
        assert!(
            output.status.success(),
            "a concurrent observe failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // Sequences must be exactly {0, 1, .., WRITERS}: contiguous, no collisions.
    let log = fs::read_to_string(workspace.join("events.jsonl")).unwrap();
    let mut sequences: Vec<u64> = log
        .lines()
        .map(|line| {
            serde_json::from_str::<Value>(line).unwrap()["sequence"]
                .as_u64()
                .unwrap()
        })
        .collect();
    let total = 1 + WRITERS as u64;
    assert_eq!(sequences.len(), total as usize, "unexpected event count");
    sequences.sort_unstable();
    assert_eq!(
        sequences,
        (0..total).collect::<Vec<u64>>(),
        "sequences must be contiguous and unique — a collision means a lost append"
    );

    // Every observe must land a distinct observation id (no silent overwrite).
    let observation_ids: std::collections::BTreeSet<u64> = log
        .lines()
        .filter_map(|line| {
            let record: Value = serde_json::from_str(line).unwrap();
            let event = &record["event"];
            (event["type"] == "observation_recorded")
                .then(|| event["observation_id"].as_u64().unwrap())
        })
        .collect();
    assert_eq!(
        observation_ids.len(),
        WRITERS,
        "each concurrent observe must receive a distinct observation id"
    );

    // And the whole log still replays cleanly (no CorruptLog).
    invoke(&["status", "--repository", &repo, "--workspace", &ws]);
}

/// Normalized fingerprinting: a `.rs` observation captured with `--normalize
/// rustfmt` fingerprints the formatter-canonical form, so a pure reformat
/// (same meaning, different bytes) stays `current` while a real semantic edit
/// still stales. The explicit `--normalize none` escape hatch keeps byte-exact
/// behavior — a reformat stales it, as any edit does.
#[test]
fn rustfmt_normalized_observation_ignores_reformat_but_catches_semantics() {
    let fixture =
        GitFixture::with_files(&[("src/task.rs", "pub fn f() -> i32 { let x = 1; x }\n")]);
    let workspace = fixture.root.path().join("workspace-state");
    let repo = fixture.repository.to_str().unwrap().to_owned();
    let ws = workspace.to_str().unwrap().to_owned();

    // Normalized observation, plus a default (byte) observation for contrast.
    let normalized: Observation = serde_json::from_slice(
        &invoke(&[
            "observe",
            "--repository",
            &repo,
            "--workspace",
            &ws,
            "--path",
            "src/task.rs",
            "--normalize",
            "rustfmt",
        ])
        .stdout,
    )
    .unwrap();
    // Byte-exact observation via the escape hatch, for contrast.
    let byte: Observation = serde_json::from_slice(
        &invoke(&[
            "observe",
            "--repository",
            &repo,
            "--workspace",
            &ws,
            "--path",
            "src/task.rs",
            "--normalize",
            "none",
        ])
        .stdout,
    )
    .unwrap();
    let normalized_id = normalized.id.to_string();
    let byte_id = byte.id.to_string();

    // A pure reformat: rustfmt-equivalent to the original, different bytes.
    fs::write(
        fixture.repository.join("src/task.rs"),
        "pub fn f() -> i32 {\n    let x = 1;\n    x\n}\n",
    )
    .unwrap();

    let reconcile = |id: &str| -> FreshnessWithinScope {
        let observation: Observation = serde_json::from_slice(
            &invoke(&[
                "reconcile",
                "--repository",
                &repo,
                "--workspace",
                &ws,
                "--id",
                id,
            ])
            .stdout,
        )
        .unwrap();
        observation.report.freshness_within_scope
    };

    assert_eq!(
        reconcile(&normalized_id),
        FreshnessWithinScope::Current,
        "a pure reformat must not stale a rustfmt-normalized observation"
    );
    assert_eq!(
        reconcile(&byte_id),
        FreshnessWithinScope::Stale,
        "the default byte observation still stales on any byte change, reformat included"
    );

    // A semantic edit must stale even the normalized observation.
    fs::write(
        fixture.repository.join("src/task.rs"),
        "pub fn f() -> i32 {\n    let x = 2;\n    x\n}\n",
    )
    .unwrap();
    assert_eq!(
        reconcile(&normalized_id),
        FreshnessWithinScope::Stale,
        "a semantic edit must still stale a normalized observation"
    );
}

/// The TypeScript half of normalized fingerprinting: a `.ts` observation
/// captured with `--normalize prettier` fingerprints prettier's canonical form,
/// so a format-on-save reflow (same meaning, different bytes) stays `current`
/// while a real edit still stales. This is the concrete fix for the pain that
/// drove the whole formatter-vs-structural investigation: an agent's TS belief
/// must not go stale — nor tempt a revert — just because prettier reflowed the
/// file. Skips when prettier is unavailable, since the fallback is raw bytes and
/// a reflow would then (correctly) stale, which is not what this asserts.
#[test]
fn prettier_normalized_observation_ignores_reflow_but_catches_semantics() {
    if !Command::new("prettier")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
    {
        eprintln!("SKIP: prettier unavailable; normalization falls back to raw bytes");
        return;
    }

    let fixture = GitFixture::with_files(&[(
        "src/task.ts",
        "export function f(x: number): number {\n  return x + 1;\n}\n",
    )]);
    let repo = fixture.repository.to_str().unwrap().to_owned();
    let ws = fixture
        .root
        .path()
        .join("workspace-state")
        .to_str()
        .unwrap()
        .to_owned();

    let observe = |normalize: &str| -> Observation {
        serde_json::from_slice(
            &invoke(&[
                "observe",
                "--repository",
                &repo,
                "--workspace",
                &ws,
                "--path",
                "src/task.ts",
                "--normalize",
                normalize,
            ])
            .stdout,
        )
        .unwrap()
    };
    let normalized_id = observe("prettier").id.to_string();
    let byte_id = observe("none").id.to_string();

    let reconcile = |id: &str| -> FreshnessWithinScope {
        let observation: Observation = serde_json::from_slice(
            &invoke(&[
                "reconcile",
                "--repository",
                &repo,
                "--workspace",
                &ws,
                "--id",
                id,
            ])
            .stdout,
        )
        .unwrap();
        observation.report.freshness_within_scope
    };

    // Format-on-save reflow: prettier-equivalent to the original, different bytes.
    fs::write(
        fixture.repository.join("src/task.ts"),
        "export  function f(x:number):number{\n      return x+1;\n}\n",
    )
    .unwrap();
    assert_eq!(
        reconcile(&normalized_id),
        FreshnessWithinScope::Current,
        "a prettier reflow must not stale a prettier-normalized TS observation"
    );
    assert_eq!(
        reconcile(&byte_id),
        FreshnessWithinScope::Stale,
        "the byte observation still stales on any byte change, reflow included"
    );

    // A real edit must stale even the normalized observation.
    fs::write(
        fixture.repository.join("src/task.ts"),
        "export function f(x: number): number {\n  return x + 2;\n}\n",
    )
    .unwrap();
    assert_eq!(
        reconcile(&normalized_id),
        FreshnessWithinScope::Stale,
        "a real edit must still stale a normalized observation"
    );
}

/// The Python half: a `.py` observation captured with `--normalize ruff`
/// fingerprints ruff's canonical form, so a reflow of an adapter hook stays
/// `current` while a real edit stales. Same idempotence guarantee as prettier;
/// this repo's own Claude hooks are Python, so the case is not hypothetical.
/// Skips when ruff is unavailable.
#[test]
fn ruff_normalized_observation_ignores_reflow_but_catches_semantics() {
    if !Command::new("ruff")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
    {
        eprintln!("SKIP: ruff unavailable; normalization falls back to raw bytes");
        return;
    }

    let fixture = GitFixture::with_files(&[("hooks/orient.py", "def f(x):\n    return x + 1\n")]);
    let repo = fixture.repository.to_str().unwrap().to_owned();
    let ws = fixture
        .root
        .path()
        .join("workspace-state")
        .to_str()
        .unwrap()
        .to_owned();

    let observe = |normalize: &str| -> Observation {
        serde_json::from_slice(
            &invoke(&[
                "observe",
                "--repository",
                &repo,
                "--workspace",
                &ws,
                "--path",
                "hooks/orient.py",
                "--normalize",
                normalize,
            ])
            .stdout,
        )
        .unwrap()
    };
    let normalized_id = observe("ruff").id.to_string();

    let reconcile = |id: &str| -> FreshnessWithinScope {
        let observation: Observation = serde_json::from_slice(
            &invoke(&[
                "reconcile",
                "--repository",
                &repo,
                "--workspace",
                &ws,
                "--id",
                id,
            ])
            .stdout,
        )
        .unwrap();
        observation.report.freshness_within_scope
    };

    // Reflow: ruff-equivalent to the original, different bytes.
    fs::write(
        fixture.repository.join("hooks/orient.py"),
        "def  f( x ):\n        return    x+1\n",
    )
    .unwrap();
    assert_eq!(
        reconcile(&normalized_id),
        FreshnessWithinScope::Current,
        "a ruff reflow must not stale a ruff-normalized Python observation"
    );

    // A real edit must stale even the normalized observation.
    fs::write(
        fixture.repository.join("hooks/orient.py"),
        "def f(x):\n    return x + 2\n",
    )
    .unwrap();
    assert_eq!(
        reconcile(&normalized_id),
        FreshnessWithinScope::Stale,
        "a real edit must still stale a normalized observation"
    );
}

/// The `auto` default: `observe` with no `--normalize` flag resolves the
/// normalizer from the path extension — rustfmt for recognized Rust source,
/// raw bytes for unrecognized types — and persists the *resolved* scheme on
/// the record, so reconcile semantics never depend on resolution order.
#[test]
fn auto_normalizer_detects_rustfmt_for_rust_and_none_otherwise() {
    let fixture = GitFixture::with_files(&[
        ("src/task.rs", "pub fn f() -> i32 { 1 }\n"),
        ("docs/notes.md", "# notes\n"),
    ]);
    let workspace = fixture.root.path().join("workspace-state");
    let repo = fixture.repository.to_str().unwrap().to_owned();
    let ws = workspace.to_str().unwrap().to_owned();

    let observe = |path: &str| -> Observation {
        serde_json::from_slice(
            &invoke(&[
                "observe",
                "--repository",
                &repo,
                "--workspace",
                &ws,
                "--path",
                path,
            ])
            .stdout,
        )
        .unwrap()
    };

    let rust = observe("src/task.rs");
    assert_eq!(rust.normalizer, Normalizer::Rustfmt);
    assert!(
        rust.observed_raw_fingerprint.is_some(),
        "a normalized record must carry the raw fingerprint for the reconcile fast path"
    );

    let markdown = observe("docs/notes.md");
    assert_eq!(markdown.normalizer, Normalizer::None);
    assert_eq!(
        markdown.observed_raw_fingerprint, None,
        "a byte-mode record needs no separate raw fingerprint"
    );
}

/// The selection seam is configurable, not hard-coded: a committed
/// `.agent-workspace/normalizers.toml` overlays the builtin default, so a real
/// `observe` capture persists the *configured* normalizer. Proven by the case
/// that inverts the default — disabling rustfmt for Rust — so a pass cannot come
/// from the builtin still being in force.
#[test]
fn capture_resolves_the_normalizer_from_committed_repo_config() {
    let fixture = GitFixture::with_files(&[("src/task.rs", "pub fn f() -> i32 { 1 }\n")]);
    let config_dir = fixture.repository.join(".agent-workspace");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(
        config_dir.join("normalizers.toml"),
        "[normalizers]\nrs = { tool = \"none\" }\n",
    )
    .unwrap();

    let workspace = fixture.root.path().join("workspace-state");
    let observed: Observation = serde_json::from_slice(
        &invoke(&[
            "observe",
            "--repository",
            fixture.repository.to_str().unwrap(),
            "--workspace",
            workspace.to_str().unwrap(),
            "--path",
            "src/task.rs",
        ])
        .stdout,
    )
    .unwrap();

    assert_eq!(
        observed.normalizer,
        Normalizer::None,
        "config must override the builtin rs -> rustfmt default at capture time"
    );
}

/// Reconcile fast path: while the raw bytes are unchanged, reconcile must not
/// need the formatter at all. Proven black-box by reconciling under a PATH
/// that has git (which the CLI still needs) but no rustfmt: without the fast
/// path the missing formatter would fall back to raw bytes and false-stale
/// this deliberately non-canonical file.
#[test]
fn reconcile_fast_path_skips_formatter_when_bytes_unchanged() {
    let fixture = GitFixture::with_task_source("pub fn f() -> i32 { let x = 1; x }\n");
    let workspace = fixture.root.path().join("workspace-state");
    let repo = fixture.repository.to_str().unwrap().to_owned();
    let ws = workspace.to_str().unwrap().to_owned();

    let observed: Observation = serde_json::from_slice(
        &invoke(&[
            "observe",
            "--repository",
            &repo,
            "--workspace",
            &ws,
            "--path",
            "src/task.rs",
        ])
        .stdout,
    )
    .unwrap();
    assert_eq!(observed.normalizer, Normalizer::Rustfmt);

    // Build a PATH containing git but not rustfmt.
    let bin_dir = fixture.root.path().join("bin");
    fs::create_dir(&bin_dir).unwrap();
    let git_path = std::env::var_os("PATH")
        .and_then(|paths| {
            std::env::split_paths(&paths).find_map(|dir| {
                let candidate = dir.join("git");
                candidate.is_file().then_some(candidate)
            })
        })
        .expect("git must be on PATH for this test");
    std::os::unix::fs::symlink(git_path, bin_dir.join("git")).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_agent-workspace"))
        .args([
            "reconcile",
            "--repository",
            &repo,
            "--workspace",
            &ws,
            "--id",
            &observed.id.to_string(),
        ])
        .env("PATH", &bin_dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "reconcile failed without rustfmt on PATH: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let reconciled: Observation = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        reconciled.report.freshness_within_scope,
        FreshnessWithinScope::Current,
        "unchanged raw bytes must stay current without invoking the formatter"
    );
}

/// Claim dependencies auto-detect the normalizer kernel-side: a dependency on
/// Rust source fingerprints the canonical form, so reformatting the dependency
/// leaves the claim current while a semantic edit stales it.
#[test]
fn claim_dependency_auto_detects_normalizer() {
    let fixture = GitFixture::with_files(&[
        ("src/lib.rs", "pub fn foo() -> i32 { 1 }\n"),
        ("src/task.rs", "pub fn f() -> i32 { let x = 1; x }\n"),
    ]);
    let workspace = fixture.root.path().join("workspace-state");
    let repo = fixture.repository.to_str().unwrap().to_owned();
    let ws = workspace.to_str().unwrap().to_owned();

    let observed: Observation = serde_json::from_slice(
        &invoke(&[
            "observe",
            "--repository",
            &repo,
            "--workspace",
            &ws,
            "--path",
            "src/lib.rs",
        ])
        .stdout,
    )
    .unwrap();
    let claim: Claim = serde_json::from_slice(
        &invoke(&[
            "claim",
            "--repository",
            &repo,
            "--workspace",
            &ws,
            "--statement",
            "dependency normalization test",
            "--observation",
            &observed.id.to_string(),
            "--dependency",
            "src/task.rs",
        ])
        .stdout,
    )
    .unwrap();
    let dependency = claim
        .inputs
        .iter()
        .find(|input| input.path == Path::new("src/task.rs"))
        .expect("the declared dependency must be a claim input");
    assert_eq!(dependency.normalizer, Normalizer::Rustfmt);
    assert!(dependency.recorded_raw_fingerprint.is_some());

    let reconcile_claim = || -> FreshnessWithinScope {
        let claim: Claim = serde_json::from_slice(
            &invoke(&[
                "reconcile-claim",
                "--repository",
                &repo,
                "--workspace",
                &ws,
                "--id",
                &claim.id.to_string(),
            ])
            .stdout,
        )
        .unwrap();
        claim.report.freshness_within_scope
    };

    // A pure reformat of the dependency leaves the claim current.
    fs::write(
        fixture.repository.join("src/task.rs"),
        "pub fn f() -> i32 {\n    let x = 1;\n    x\n}\n",
    )
    .unwrap();
    assert_eq!(
        reconcile_claim(),
        FreshnessWithinScope::Current,
        "a reformatted dependency must not stale the claim under auto-detection"
    );

    // A semantic edit of the dependency stales the claim.
    fs::write(
        fixture.repository.join("src/task.rs"),
        "pub fn f() -> i32 {\n    let x = 2;\n    x\n}\n",
    )
    .unwrap();
    assert_eq!(
        reconcile_claim(),
        FreshnessWithinScope::Stale,
        "a semantic dependency edit must still stale the claim"
    );
}

#[test]
fn working_set_view_projects_ranked_semantic_locations_uncited_and_trail() {
    let fixture = GitFixture::with_files(&[
        ("src/a.rs", "pub fn a() -> i32 { 1 }\n"),
        ("src/b.rs", "pub fn b() -> i32 { 1 }\n"),
        ("src/c.rs", "pub fn c() -> i32 { 1 }\n"),
        ("src/d.rs", "pub fn d() -> i32 { 1 }\n"),
    ]);
    let workspace = fixture.root.path().join("workspace-state");
    let repo = fixture.repository.to_str().unwrap().to_string();
    let ws = workspace.to_str().unwrap().to_string();

    let observe = |path: &str| -> Observation {
        let out = invoke(&[
            "observe",
            "--repository",
            repo.as_str(),
            "--workspace",
            ws.as_str(),
            "--path",
            path,
        ]);
        serde_json::from_slice(&out.stdout).unwrap()
    };
    let focus = |id: u64, reason: &str| {
        invoke(&[
            "focus",
            "--repository",
            repo.as_str(),
            "--workspace",
            ws.as_str(),
            "--observation",
            &id.to_string(),
            "--reason",
            reason,
        ]);
    };
    let working_set = || -> Value {
        let out = invoke(&[
            "working-set",
            "--repository",
            repo.as_str(),
            "--workspace",
            ws.as_str(),
        ]);
        serde_json::from_slice(&out.stdout).unwrap()
    };

    let a = observe("src/a.rs");
    let b = observe("src/b.rs");
    let c = observe("src/c.rs");
    observe("src/d.rs");

    // a.rs is cited by an active claim; b.rs and c.rs are focused into the
    // working set (b revisited last); d.rs is only observed.
    invoke(&[
        "claim",
        "--repository",
        repo.as_str(),
        "--workspace",
        ws.as_str(),
        "--statement",
        "a returns one",
        "--observation",
        &a.id.to_string(),
    ]);
    focus(b.id, "investigate b");
    focus(c.id, "investigate c");
    focus(b.id, "recheck b");

    let view = working_set();

    // Criterion 3: a focused entry is a *semantic location* carrying the
    // observation's coordinates, not a bare id.
    let locations = view["locations"].as_array().unwrap();
    assert_eq!(
        locations.len(),
        2,
        "b and c are focused; a is cited, d is not"
    );
    let top = &locations[0];
    assert_eq!(top["observation_id"].as_u64().unwrap(), b.id);
    assert_eq!(top["path"], "src/b.rs");
    assert_eq!(top["selector"]["kind"], "whole_file");
    assert_eq!(top["freshness"], "current");
    // Latest focus reason wins, and the revisit makes b the most recent.
    assert_eq!(top["reason"], "recheck b");
    assert!(
        top["focus_sequence"].as_u64().unwrap() > locations[1]["focus_sequence"].as_u64().unwrap(),
        "revisited b outranks c by recency"
    );
    assert!(!top["observed_revision"].as_str().unwrap().is_empty());
    assert!(!top["relocation_fingerprint"].as_str().unwrap().is_empty());

    // Criterion 1: only current observations neither cited nor focused surface as
    // attention candidates — here just d.rs.
    let uncited = view["uncited"].as_array().unwrap();
    assert_eq!(uncited.len(), 1);
    assert_eq!(uncited[0]["path"], "src/d.rs");

    // Criterion 6 (trail): every visit, most recent first, revisits included.
    let trail = view["trail"].as_array().unwrap();
    assert_eq!(trail.len(), 3);
    assert_eq!(trail[0]["reason"], "recheck b");
    assert_eq!(trail[1]["reason"], "investigate c");
    assert_eq!(trail[2]["reason"], "investigate b");

    // Criterion 5 + ranking: an out-of-band edit under a focused location stales
    // it, and stale sorts ahead of current so a cap can never hide it.
    fs::write(
        fixture.repository.join("src/b.rs"),
        "pub fn b() -> i32 { 999 }\n",
    )
    .unwrap();
    let after_edit = working_set();
    let top = &after_edit["locations"][0];
    assert_eq!(top["observation_id"].as_u64().unwrap(), b.id);
    assert_eq!(top["freshness"], "stale", "edited location is stale-first");

    // Criterion 6 (restart): a cold re-invocation recovers the same ordered
    // trail from the event log alone.
    let resumed = working_set();
    let resumed_trail = resumed["trail"].as_array().unwrap();
    assert_eq!(resumed_trail.len(), 3);
    assert_eq!(resumed_trail[0]["reason"], "recheck b");
    assert_eq!(resumed_trail[2]["reason"], "investigate b");
}

#[test]
fn working_set_view_hard_bounds_each_section_with_explicit_omissions() {
    // 13 focused locations, 13 uncited candidates, and 17 total focus visits —
    // each one over its cap by exactly one — so every omission counter is proven.
    let files: Vec<(String, String)> = (0..26)
        .map(|i| (format!("src/f{i:02}.rs"), format!("pub fn f{i}() {{}}\n")))
        .collect();
    let file_refs: Vec<(&str, &str)> = files
        .iter()
        .map(|(path, body)| (path.as_str(), body.as_str()))
        .collect();
    let fixture = GitFixture::with_files(&file_refs);
    let workspace = fixture.root.path().join("workspace-state");
    let repo = fixture.repository.to_str().unwrap().to_string();
    let ws = workspace.to_str().unwrap().to_string();

    let observe = |path: &str| -> Observation {
        let out = invoke(&[
            "observe",
            "--repository",
            repo.as_str(),
            "--workspace",
            ws.as_str(),
            "--path",
            path,
        ]);
        serde_json::from_slice(&out.stdout).unwrap()
    };
    let focus = |id: u64| {
        invoke(&[
            "focus",
            "--repository",
            repo.as_str(),
            "--workspace",
            ws.as_str(),
            "--observation",
            &id.to_string(),
            "--reason",
            "bound fixture",
        ]);
    };

    let ids: Vec<u64> = (0..26)
        .map(|i| observe(&format!("src/f{i:02}.rs")).id)
        .collect();
    // Focus the first 13 (distinct locations); leave the last 13 as uncited
    // candidates; revisit 4 to push the trail to 17 visits.
    for &id in &ids[..13] {
        focus(id);
    }
    for &id in &ids[..4] {
        focus(id);
    }

    let out = invoke(&[
        "working-set",
        "--repository",
        repo.as_str(),
        "--workspace",
        ws.as_str(),
    ]);
    let view: Value = serde_json::from_slice(&out.stdout).unwrap();

    assert_eq!(view["locations"].as_array().unwrap().len(), 12);
    assert_eq!(view["locations_omitted"].as_u64().unwrap(), 1);
    assert_eq!(view["uncited"].as_array().unwrap().len(), 12);
    assert_eq!(view["uncited_omitted"].as_u64().unwrap(), 1);
    assert_eq!(view["trail"].as_array().unwrap().len(), 16);
    assert_eq!(view["trail_omitted"].as_u64().unwrap(), 1);
}

#[test]
fn brief_status_recomputes_active_claims_without_retired_history() {
    let fixture = GitFixture::new();
    let workspace = fixture.root.path().join("workspace-state");
    let repo = fixture.repository.to_str().unwrap();
    let ws = workspace.to_str().unwrap();

    let helper_observation: Observation = serde_json::from_slice(
        &invoke(&[
            "observe",
            "--repository",
            repo,
            "--workspace",
            ws,
            "--path",
            "src/helper.rs",
        ])
        .stdout,
    )
    .unwrap();
    let lib_observation: Observation = serde_json::from_slice(
        &invoke(&[
            "observe",
            "--repository",
            repo,
            "--workspace",
            ws,
            "--path",
            "src/lib.rs",
        ])
        .stdout,
    )
    .unwrap();
    let active: Claim = serde_json::from_slice(
        &invoke(&[
            "claim",
            "--repository",
            repo,
            "--workspace",
            ws,
            "--statement",
            "helper returns one",
            "--observation",
            &helper_observation.id.to_string(),
        ])
        .stdout,
    )
    .unwrap();
    let retire: Claim = serde_json::from_slice(
        &invoke(&[
            "claim",
            "--repository",
            repo,
            "--workspace",
            ws,
            "--statement",
            "foo returns one",
            "--observation",
            &lib_observation.id.to_string(),
        ])
        .stdout,
    )
    .unwrap();
    let successor: Claim = serde_json::from_slice(
        &invoke(&[
            "claim",
            "--repository",
            repo,
            "--workspace",
            ws,
            "--statement",
            "foo returns one after review",
            "--observation",
            &lib_observation.id.to_string(),
        ])
        .stdout,
    )
    .unwrap();
    invoke(&[
        "supersede-claim",
        "--repository",
        repo,
        "--workspace",
        ws,
        "--id",
        &retire.id.to_string(),
        "--claim",
        &successor.id.to_string(),
        "--reason",
        "consumed by review",
    ]);
    let log_before: usize = fs::read_to_string(workspace.join("events.jsonl"))
        .unwrap()
        .lines()
        .count();

    // Edit only the ACTIVE claim's input. The superseded claim's input is
    // untouched, so if the bounded path reconciled retired history it would be
    // visible as a no-op-differing event; if it skipped the active claim, the
    // staleness below would never appear.
    fs::write(
        fixture.repository.join("src/helper.rs"),
        "pub fn helper() -> i32 { 2 }\n",
    )
    .unwrap();

    let status = invoke(&["status", "--repository", repo, "--workspace", ws]);
    let brief: Value = serde_json::from_slice(&status.stdout).unwrap();
    let claims = brief["claims"].as_array().unwrap();
    let freshness_of = |id: u64| {
        claims
            .iter()
            .find(|claim| claim["id"].as_u64() == Some(id))
            .map(|claim| claim["freshness"].clone())
            .unwrap_or_else(|| serde_json::json!("absent"))
    };
    // F9 on the bounded path: the active claim's staleness was recomputed.
    assert_eq!(freshness_of(active.id), "stale");
    assert_eq!(freshness_of(successor.id), "current");
    // Retired history is not part of the served surface.
    assert_eq!(freshness_of(retire.id), "absent");
    assert_eq!(brief["counts"]["freshness"]["stale"], 1);
    assert_eq!(brief["counts"]["freshness"]["current"], 1);

    // Exactly one claim reconcile event was appended: for the active claim.
    // The superseded claim was never reconciled by the bounded status.
    let records: Vec<Value> = fs::read_to_string(workspace.join("events.jsonl"))
        .unwrap()
        .lines()
        .skip(log_before)
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let reconciled: Vec<u64> = records
        .iter()
        .filter(|record| record["event"]["type"] == "claim_reconciled")
        .map(|record| record["event"]["claim_id"].as_u64().unwrap())
        .collect();
    assert_eq!(reconciled, vec![active.id]);
}

#[test]
fn working_set_reserves_newest_current_focus_under_a_stale_cap() {
    let files: Vec<(String, String)> = (0..14)
        .map(|i| (format!("src/g{i:02}.rs"), format!("pub fn g{i}() {{}}\n")))
        .collect();
    let file_refs: Vec<(&str, &str)> = files
        .iter()
        .map(|(path, body)| (path.as_str(), body.as_str()))
        .collect();
    let fixture = GitFixture::with_files(&file_refs);
    let workspace = fixture.root.path().join("workspace-state");
    let repo = fixture.repository.to_str().unwrap();
    let ws = workspace.to_str().unwrap();

    let observe = |path: &str| -> Observation {
        serde_json::from_slice(
            &invoke(&[
                "observe",
                "--repository",
                repo,
                "--workspace",
                ws,
                "--path",
                path,
            ])
            .stdout,
        )
        .unwrap()
    };
    let focus = |id: u64| {
        invoke(&[
            "focus",
            "--repository",
            repo,
            "--workspace",
            ws,
            "--observation",
            &id.to_string(),
            "--reason",
            "stale-cap fixture",
        ]);
    };

    let mut ids: Vec<u64> = (0..13)
        .map(|i| observe(&format!("src/g{i:02}.rs")).id)
        .collect();
    for &id in &ids {
        focus(id);
    }
    // Stale all 13 focused locations out of band.
    for i in 0..13 {
        let path = fixture.repository.join(format!("src/g{i:02}.rs"));
        fs::write(&path, format!("pub fn g{i}() {{ 1 }}\n")).unwrap();
    }
    // Focus one fresh, still-current location — the newest attention.
    let fresh = observe("src/g13.rs");
    focus(fresh.id);
    ids.push(fresh.id);

    let view: Value = serde_json::from_slice(
        &invoke(&["working-set", "--repository", repo, "--workspace", ws]).stdout,
    )
    .unwrap();
    let locations = view["locations"].as_array().unwrap();
    assert_eq!(locations.len(), 12);
    // Two stale entries fall past the cap.
    assert_eq!(view["locations_omitted"].as_u64().unwrap(), 2);
    // The newest current focus survived the cap despite ranking last.
    assert!(
        locations
            .iter()
            .any(|location| location["observation_id"].as_u64() == Some(fresh.id))
    );
    let fresh_row = locations
        .iter()
        .find(|location| location["observation_id"].as_u64() == Some(fresh.id))
        .unwrap();
    assert_eq!(fresh_row["freshness"], "current");
}

#[test]
fn bounded_working_set_omits_unverified_uncited_candidates() {
    let files: Vec<(String, String)> = (0..26)
        .map(|i| (format!("src/h{i:02}.rs"), format!("pub fn h{i}() {{}}\n")))
        .collect();
    let file_refs: Vec<(&str, &str)> = files
        .iter()
        .map(|(path, body)| (path.as_str(), body.as_str()))
        .collect();
    let fixture = GitFixture::with_files(&file_refs);
    let workspace = fixture.root.path().join("workspace-state");
    let repo = fixture.repository.to_str().unwrap();
    let ws = workspace.to_str().unwrap();

    let ids: Vec<u64> = (0..26)
        .map(|i| {
            serde_json::from_slice::<Observation>(
                &invoke(&[
                    "observe",
                    "--repository",
                    repo,
                    "--workspace",
                    ws,
                    "--path",
                    &format!("src/h{i:02}.rs"),
                ])
                .stdout,
            )
            .unwrap()
            .id
        })
        .collect();

    let working_set = || -> Value {
        serde_json::from_slice(
            &invoke(&["working-set", "--repository", repo, "--workspace", ws]).stdout,
        )
        .unwrap()
    };

    // 26 uncited current observations: the 12 newest are served, the rest are
    // outside the bounded candidate window and counted as omitted.
    let view = working_set();
    let uncited = view["uncited"].as_array().unwrap();
    assert_eq!(uncited.len(), 12);
    assert_eq!(view["uncited_omitted"].as_u64().unwrap(), 14);
    let served_ids: Vec<u64> = uncited
        .iter()
        .map(|entry| entry["observation_id"].as_u64().unwrap())
        .collect();
    assert_eq!(
        served_ids,
        ids[14..26].iter().rev().copied().collect::<Vec<_>>()
    );

    // Stale the newest candidate. It becomes a known non-candidate inside the
    // window: excluded from serving but no longer counted as omitted.
    fs::write(
        fixture.repository.join("src/h25.rs"),
        "pub fn h25() { 1 }\n",
    )
    .unwrap();
    let view = working_set();
    let uncited = view["uncited"].as_array().unwrap();
    assert_eq!(uncited.len(), 12);
    assert_eq!(view["uncited_omitted"].as_u64().unwrap(), 13);
    let served_ids: Vec<u64> = uncited
        .iter()
        .map(|entry| entry["observation_id"].as_u64().unwrap())
        .collect();
    assert_eq!(
        served_ids,
        ids[13..25].iter().rev().copied().collect::<Vec<_>>()
    );
    assert!(!served_ids.contains(&ids[25]));
}

#[test]
fn finding_retains_provider_provenance_and_reveals_native_payload() {
    // S8: a normalized finding keeps its provider identity and native payload
    // (or CAS reference) retrievable.
    let fixture = GitFixture::new();
    let workspace = fixture.root.path().join("workspace-state");
    let repo = fixture.repository.to_str().unwrap();
    let ws = workspace.to_str().unwrap();
    let payload = r#"{"rule":"needless_return","level":"warning","spans":[{"line":1}]}"#;

    let recorded = invoke_with_stdin(
        &[
            "record-finding",
            "--repository",
            repo,
            "--workspace",
            ws,
            "--provider",
            "clippy",
            "--severity",
            "warning",
            "--rule",
            "needless_return",
            "--message",
            "unneeded return statement",
            "--path",
            "src/lib.rs",
        ],
        payload,
    );
    let finding: Finding = serde_json::from_slice(&recorded.stdout).unwrap();
    assert_eq!(finding.provider, "clippy");
    assert_eq!(finding.severity, FindingSeverity::Warning);
    assert_eq!(finding.rule.as_deref(), Some("needless_return"));
    assert_eq!(finding.message, "unneeded return statement");
    assert_eq!(finding.path.to_str().unwrap(), "src/lib.rs");
    assert_eq!(
        finding.report.freshness_within_scope,
        FreshnessWithinScope::Current
    );
    assert!(matches!(finding.disposition, FindingDisposition::Open));
    assert!(finding.native_payload_fingerprint.is_some());
    assert!(!finding.observed_revision.is_empty());

    // Native payload is retrievable and byte-exact, with provider identity intact.
    let revealed = invoke(&[
        "reveal-finding",
        "--repository",
        repo,
        "--workspace",
        ws,
        "--id",
        &finding.id.to_string(),
    ]);
    let revealed: RevealedFinding = serde_json::from_slice(&revealed.stdout).unwrap();
    assert_eq!(revealed.provider, "clippy");
    assert_eq!(revealed.content, payload);
}

#[test]
fn finding_stales_when_its_bound_location_changes() {
    // The evidence-invalidation analog: a diagnostic that may no longer apply
    // after an edit must not keep counting as a current issue.
    let fixture = GitFixture::new();
    let workspace = fixture.root.path().join("workspace-state");
    let repo = fixture.repository.to_str().unwrap();
    let ws = workspace.to_str().unwrap();

    // No native payload supplied (empty stdin) — a finding without retained
    // provenance is still a first-class queue entry.
    invoke_with_stdin(
        &[
            "record-finding",
            "--repository",
            repo,
            "--workspace",
            ws,
            "--provider",
            "rustc",
            "--severity",
            "error",
            "--message",
            "mismatched types",
            "--path",
            "src/lib.rs",
        ],
        "",
    );

    let before: WorkspaceStatus = serde_json::from_slice(
        &invoke(&["status", "--full", "--repository", repo, "--workspace", ws]).stdout,
    )
    .unwrap();
    assert_eq!(before.findings.len(), 1);
    assert_eq!(
        before.findings[0].report.freshness_within_scope,
        FreshnessWithinScope::Current
    );

    fs::write(
        fixture.repository.join("src/lib.rs"),
        "pub fn foo() -> i32 { 2 }\n",
    )
    .unwrap();

    let after: WorkspaceStatus = serde_json::from_slice(
        &invoke(&["status", "--full", "--repository", repo, "--workspace", ws]).stdout,
    )
    .unwrap();
    assert_eq!(
        after.findings[0].report.freshness_within_scope,
        FreshnessWithinScope::Stale
    );

    // A finding with no retained payload fails closed on reveal rather than
    // returning unverified bytes.
    let revealed = invoke_failure(&[
        "reveal-finding",
        "--repository",
        repo,
        "--workspace",
        ws,
        "--id",
        "0",
    ]);
    assert!(String::from_utf8_lossy(&revealed.stderr).contains("no native payload was retained"));
}

#[test]
fn findings_queue_ranks_by_severity_and_disposition_leaves_the_queue() {
    let fixture = GitFixture::new();
    let workspace = fixture.root.path().join("workspace-state");
    let repo = fixture.repository.to_str().unwrap();
    let ws = workspace.to_str().unwrap();

    let record = |severity: &str, message: &str| {
        invoke_with_stdin(
            &[
                "record-finding",
                "--repository",
                repo,
                "--workspace",
                ws,
                "--provider",
                "rustc",
                "--severity",
                severity,
                "--message",
                message,
                "--path",
                "src/lib.rs",
            ],
            "",
        );
    };
    // Record out of severity order to prove the queue ranks, not insertion-order.
    record("info", "consider documenting foo"); // id 0
    record("error", "mismatched types"); // id 1
    record("warning", "unused import"); // id 2

    let queue: Value = serde_json::from_slice(
        &invoke(&["findings", "--repository", repo, "--workspace", ws]).stdout,
    )
    .unwrap();
    let open = queue["open"].as_array().unwrap();
    assert_eq!(open.len(), 3);
    assert_eq!(open[0]["severity"], "error");
    assert_eq!(open[1]["severity"], "warning");
    assert_eq!(open[2]["severity"], "info");
    assert_eq!(queue["open_omitted"].as_u64().unwrap(), 0);
    assert_eq!(queue["disposed"].as_u64().unwrap(), 0);

    // Disposition names its actor and rationale (invariant 8) and removes the
    // finding from the open queue while keeping it in the audit record.
    let disposed = invoke(&[
        "dispose-finding",
        "--repository",
        repo,
        "--workspace",
        ws,
        "--id",
        "1",
        "--disposition",
        "false-positive",
        "--actor",
        "darrion",
        "--rationale",
        "the borrow checker is satisfied after the sibling change",
    ]);
    let disposed: Finding = serde_json::from_slice(&disposed.stdout).unwrap();
    match disposed.disposition {
        FindingDisposition::FalsePositive { actor, rationale } => {
            assert_eq!(actor, "darrion");
            assert!(rationale.contains("borrow checker"));
        }
        other => panic!("expected false-positive disposition, got {other:?}"),
    }

    let after: Value = serde_json::from_slice(
        &invoke(&["findings", "--repository", repo, "--workspace", ws]).stdout,
    )
    .unwrap();
    assert_eq!(after["open"].as_array().unwrap().len(), 2);
    assert_eq!(after["disposed"].as_u64().unwrap(), 1);
    // The error left the queue; warning now leads.
    assert_eq!(after["open"][0]["severity"], "warning");

    // Invariant 8 is enforced: a disposition with no actor/rationale is refused.
    let refused = invoke_failure(&[
        "dispose-finding",
        "--repository",
        repo,
        "--workspace",
        ws,
        "--id",
        "0",
        "--disposition",
        "resolved",
        "--actor",
        "   ",
        "--rationale",
        "",
    ]);
    assert!(String::from_utf8_lossy(&refused.stderr).contains("non-empty actor and rationale"));

    // Disposition survives restart: a cold status projection still shows it.
    let resumed: WorkspaceStatus = serde_json::from_slice(
        &invoke(&["status", "--full", "--repository", repo, "--workspace", ws]).stdout,
    )
    .unwrap();
    let error_finding = resumed
        .findings
        .iter()
        .find(|finding| finding.id == 1)
        .unwrap();
    assert!(matches!(
        error_finding.disposition,
        FindingDisposition::FalsePositive { .. }
    ));
}

#[test]
fn transaction_can_begin_with_a_tracked_symlink_to_a_directory() {
    let fixture = GitFixture::new();
    let workspace = fixture.root.path().join("workspace-state");
    let repo = fixture.repository.to_str().unwrap();
    let ws = workspace.to_str().unwrap();

    symlink("src", fixture.repository.join("linked-src")).unwrap();
    git(&fixture.repository, &["add", "linked-src"]);
    git(
        &fixture.repository,
        &["commit", "--quiet", "-m", "track directory symlink"],
    );

    let observation: Observation = serde_json::from_slice(
        &invoke(&[
            "observe",
            "--repository",
            repo,
            "--workspace",
            ws,
            "--path",
            "src/lib.rs",
        ])
        .stdout,
    )
    .unwrap();
    let claim: Claim = serde_json::from_slice(
        &invoke(&[
            "claim",
            "--repository",
            repo,
            "--workspace",
            ws,
            "--statement",
            "foo returns one",
            "--observation",
            &observation.id.to_string(),
        ])
        .stdout,
    )
    .unwrap();

    let transaction: Transaction = serde_json::from_slice(
        &invoke(&[
            "begin-transaction",
            "--repository",
            repo,
            "--workspace",
            ws,
            "--intent",
            "exercise a clean transaction",
            "--claim",
            &claim.id.to_string(),
        ])
        .stdout,
    )
    .unwrap();
    assert_eq!(transaction.state, TransactionState::Open);
}

#[test]
fn transaction_carries_intent_findings_risks_and_preview_matches_acceptance() {
    let fixture = GitFixture::new();
    let workspace = fixture.root.path().join("workspace-state");
    let repo = fixture.repository.to_str().unwrap();
    let ws = workspace.to_str().unwrap();

    let observation = invoke(&[
        "observe",
        "--repository",
        repo,
        "--workspace",
        ws,
        "--path",
        "src/lib.rs",
    ]);
    let observation: Observation = serde_json::from_slice(&observation.stdout).unwrap();
    let claim = invoke(&[
        "claim",
        "--repository",
        repo,
        "--workspace",
        ws,
        "--statement",
        "foo returns one",
        "--observation",
        &observation.id.to_string(),
    ]);
    let claim: Claim = serde_json::from_slice(&claim.stdout).unwrap();
    let finding: Finding = serde_json::from_slice(
        &invoke_with_stdin(
            &[
                "record-finding",
                "--repository",
                repo,
                "--workspace",
                ws,
                "--provider",
                "clippy",
                "--severity",
                "warning",
                "--message",
                "needless return",
                "--path",
                "src/lib.rs",
            ],
            "",
        )
        .stdout,
    )
    .unwrap();

    // Intent is required.
    let no_intent = invoke_failure(&[
        "begin-transaction",
        "--repository",
        repo,
        "--workspace",
        ws,
        "--claim",
        &claim.id.to_string(),
    ]);
    assert!(String::from_utf8_lossy(&no_intent.stderr).contains("requires --intent"));

    let transaction: Transaction = serde_json::from_slice(
        &invoke(&[
            "begin-transaction",
            "--repository",
            repo,
            "--workspace",
            ws,
            "--intent",
            "resolve the needless-return lint in foo",
            "--claim",
            &claim.id.to_string(),
        ])
        .stdout,
    )
    .unwrap();
    assert_eq!(
        transaction.intent.as_deref(),
        Some("resolve the needless-return lint in foo")
    );

    invoke(&[
        "associate-finding",
        "--repository",
        repo,
        "--workspace",
        ws,
        "--transaction",
        &transaction.id.to_string(),
        "--id",
        &finding.id.to_string(),
    ]);
    invoke(&[
        "record-risk",
        "--repository",
        repo,
        "--workspace",
        ws,
        "--transaction",
        &transaction.id.to_string(),
        "--risk",
        "no integration test covers foo's callers",
    ]);

    let preview = |id: u64| -> Value {
        serde_json::from_slice(
            &invoke(&[
                "preview-transaction",
                "--repository",
                repo,
                "--workspace",
                ws,
                "--transaction",
                &id.to_string(),
            ])
            .stdout,
        )
        .unwrap()
    };

    // Before evidence: the association and risk show, and the preview reports the
    // transaction is NOT ready — matching what accept would do.
    let before = preview(transaction.id);
    assert_eq!(before["intent"], "resolve the needless-return lint in foo");
    assert_eq!(
        before["associated_findings"][0]["id"].as_u64().unwrap(),
        finding.id
    );
    assert_eq!(before["associated_findings"][0]["freshness"], "current");
    assert_eq!(
        before["residual_risks"][0],
        "no integration test covers foo's callers"
    );
    assert_eq!(before["ready_to_accept"], false);

    // The preview must never claim readiness the accept would deny: accept fails
    // here too.
    let premature = invoke(&[
        "accept-transaction",
        "--repository",
        repo,
        "--workspace",
        ws,
        "--id",
        &transaction.id.to_string(),
    ]);
    let premature: Transaction = serde_json::from_slice(&premature.stdout).unwrap();
    assert_ne!(premature.state, TransactionState::Accepted);

    // A begin fresh transaction (the prior one recorded a rejection but stays
    // open) and add passing evidence, then preview flips to ready and accept
    // succeeds — preview and acceptance agree in both directions.
    invoke(&[
        "evidence",
        "--repository",
        repo,
        "--workspace",
        ws,
        "--transaction",
        &transaction.id.to_string(),
        "--claim",
        &claim.id.to_string(),
        "--check",
        "cargo-clippy",
        "--invocation",
        "cargo clippy",
        "--result",
        "passed",
    ]);
    let after = preview(transaction.id);
    assert_eq!(after["ready_to_accept"], true);
    assert_eq!(after["evidence"][0]["outcome"], "passed");

    let accepted = invoke(&[
        "accept-transaction",
        "--repository",
        repo,
        "--workspace",
        ws,
        "--id",
        &transaction.id.to_string(),
    ]);
    let accepted: Transaction = serde_json::from_slice(&accepted.stdout).unwrap();
    assert_eq!(accepted.state, TransactionState::Accepted);

    // Associations survive restart.
    let resumed: WorkspaceStatus = serde_json::from_slice(
        &invoke(&["status", "--full", "--repository", repo, "--workspace", ws]).stdout,
    )
    .unwrap();
    let resumed_tx = resumed
        .transactions
        .iter()
        .find(|t| t.id == transaction.id)
        .unwrap();
    assert_eq!(resumed_tx.finding_ids, vec![finding.id]);
    assert_eq!(resumed_tx.residual_risks.len(), 1);
    assert_eq!(
        resumed_tx.intent.as_deref(),
        Some("resolve the needless-return lint in foo")
    );
}

/// Acceptance re-verifies the transaction's owned bytes against disk and rejects
/// post-apply drift (a formatter reflow, a stray edit) even when every claim and
/// its evidence are current — the exact hole the dogfood caught, where accepted
/// bytes silently diverged from the bytes the checks consumed. The acceptance
/// claim binds to `helper.rs`, which is never mutated, so this isolates the
/// disk-reverification gate from claim staleness. Preview and accept agree in
/// both directions, and byte-exact restoration recovers readiness.
#[test]
fn accept_reverifies_candidate_bytes_and_rejects_drift() {
    let fixture = GitFixture::new();
    let workspace = fixture.root.path().join("workspace-state");
    let repo = fixture.repository.to_str().unwrap();
    let ws = workspace.to_str().unwrap();
    let applied_bytes = "pub fn foo() -> i32 { 2 }\n";

    let observation: Observation = serde_json::from_slice(
        &invoke(&[
            "observe",
            "--repository",
            repo,
            "--workspace",
            ws,
            "--path",
            "src/helper.rs",
        ])
        .stdout,
    )
    .unwrap();
    let claim: Claim = serde_json::from_slice(
        &invoke(&[
            "claim",
            "--repository",
            repo,
            "--workspace",
            ws,
            "--statement",
            "helper returns one",
            "--observation",
            &observation.id.to_string(),
        ])
        .stdout,
    )
    .unwrap();
    let transaction: Transaction = serde_json::from_slice(
        &invoke(&[
            "begin-transaction",
            "--intent",
            "rewrite foo body",
            "--repository",
            repo,
            "--workspace",
            ws,
            "--claim",
            &claim.id.to_string(),
        ])
        .stdout,
    )
    .unwrap();
    invoke(&[
        "apply",
        "--repository",
        repo,
        "--workspace",
        ws,
        "--id",
        &transaction.id.to_string(),
        "--path",
        "src/lib.rs",
        "--content",
        applied_bytes,
    ]);
    invoke(&[
        "evidence",
        "--repository",
        repo,
        "--workspace",
        ws,
        "--transaction",
        &transaction.id.to_string(),
        "--claim",
        &claim.id.to_string(),
        "--check",
        "cargo-test",
        "--invocation",
        "cargo test",
        "--result",
        "passed",
    ]);

    let preview = |id: u64| -> Value {
        serde_json::from_slice(
            &invoke(&[
                "preview-transaction",
                "--repository",
                repo,
                "--workspace",
                ws,
                "--transaction",
                &id.to_string(),
            ])
            .stdout,
        )
        .unwrap()
    };
    let accept = |id: u64| -> Transaction {
        serde_json::from_slice(
            &invoke(&[
                "accept-transaction",
                "--repository",
                repo,
                "--workspace",
                ws,
                "--id",
                &id.to_string(),
            ])
            .stdout,
        )
        .unwrap()
    };

    // Bytes match what was applied: preview is ready and accept would succeed.
    assert_eq!(preview(transaction.id)["ready_to_accept"], true);

    // Simulate a formatter reflowing the file after apply. Same meaning, different
    // bytes — and different from the fingerprint the evidence was recorded against.
    fs::write(
        fixture.repository.join("src/lib.rs"),
        "pub fn foo() -> i32 {\n    2\n}\n",
    )
    .unwrap();

    let drifted_preview = preview(transaction.id);
    assert_eq!(drifted_preview["ready_to_accept"], false);
    assert!(
        drifted_preview["readiness_reason"]
            .as_str()
            .unwrap()
            .contains("drifted"),
        "preview should name the drift, got {}",
        drifted_preview["readiness_reason"]
    );

    let rejected = accept(transaction.id);
    assert_eq!(rejected.state, TransactionState::Open);
    assert!(
        rejected
            .last_rejection
            .as_deref()
            .unwrap()
            .contains("drifted"),
        "accept should reject on drift, got {:?}",
        rejected.last_rejection
    );

    // Restore the exact applied bytes: readiness recovers and accept commits. It
    // is the bytes, not merely the fact of an edit, that the gate turns on.
    fs::write(fixture.repository.join("src/lib.rs"), applied_bytes).unwrap();
    assert_eq!(preview(transaction.id)["ready_to_accept"], true);
    assert_eq!(accept(transaction.id).state, TransactionState::Accepted);
}

/// Evidence binds to the content-addressed candidate it was recorded against, and
/// acceptance requires that binding still hold. Mutating a further path after the
/// evidence is recorded moves the candidate, so the earlier passing check — which
/// never saw that path — no longer proves the bytes being committed, and accept
/// fails closed until evidence bound to the new candidate is recorded. The
/// materialization gate at record time is also exercised: evidence cannot be
/// recorded while an owned path is drifted.
#[test]
fn evidence_binds_to_candidate_and_stale_binding_cannot_accept() {
    let fixture = GitFixture::with_files(&[
        ("src/lib.rs", "pub fn foo() -> i32 { 1 }\n"),
        ("src/helper.rs", "pub fn helper() -> i32 { 1 }\n"),
        ("src/other.rs", "pub fn other() -> i32 { 1 }\n"),
    ]);
    let workspace = fixture.root.path().join("workspace-state");
    let repo = fixture.repository.to_str().unwrap();
    let ws = workspace.to_str().unwrap();

    let observation: Observation = serde_json::from_slice(
        &invoke(&[
            "observe",
            "--repository",
            repo,
            "--workspace",
            ws,
            "--path",
            "src/helper.rs",
        ])
        .stdout,
    )
    .unwrap();
    let claim: Claim = serde_json::from_slice(
        &invoke(&[
            "claim",
            "--repository",
            repo,
            "--workspace",
            ws,
            "--statement",
            "helper returns one",
            "--observation",
            &observation.id.to_string(),
        ])
        .stdout,
    )
    .unwrap();
    let transaction: Transaction = serde_json::from_slice(
        &invoke(&[
            "begin-transaction",
            "--intent",
            "rewrite foo, then other",
            "--repository",
            repo,
            "--workspace",
            ws,
            "--claim",
            &claim.id.to_string(),
        ])
        .stdout,
    )
    .unwrap();

    let apply = |path: &str, content: &str| {
        invoke(&[
            "apply",
            "--repository",
            repo,
            "--workspace",
            ws,
            "--id",
            &transaction.id.to_string(),
            "--path",
            path,
            "--content",
            content,
        ]);
    };
    let record_evidence = || {
        invoke(&[
            "evidence",
            "--repository",
            repo,
            "--workspace",
            ws,
            "--transaction",
            &transaction.id.to_string(),
            "--claim",
            &claim.id.to_string(),
            "--check",
            "cargo-test",
            "--invocation",
            "cargo test",
            "--result",
            "passed",
        ])
    };
    let preview = || -> Value {
        serde_json::from_slice(
            &invoke(&[
                "preview-transaction",
                "--repository",
                repo,
                "--workspace",
                ws,
                "--transaction",
                &transaction.id.to_string(),
            ])
            .stdout,
        )
        .unwrap()
    };
    let accept = || -> Transaction {
        serde_json::from_slice(
            &invoke(&[
                "accept-transaction",
                "--repository",
                repo,
                "--workspace",
                ws,
                "--id",
                &transaction.id.to_string(),
            ])
            .stdout,
        )
        .unwrap()
    };

    // Materialization gate: record evidence while an owned path is drifted and the
    // command fails closed — the check could not have consumed that candidate.
    apply("src/lib.rs", "pub fn foo() -> i32 { 2 }\n");
    fs::write(
        fixture.repository.join("src/lib.rs"),
        "pub fn foo() -> i32 { 999 }\n",
    )
    .unwrap();
    let refused = invoke_failure(&[
        "evidence",
        "--repository",
        repo,
        "--workspace",
        ws,
        "--transaction",
        &transaction.id.to_string(),
        "--claim",
        &claim.id.to_string(),
        "--check",
        "cargo-test",
        "--invocation",
        "cargo test",
        "--result",
        "passed",
    ]);
    assert!(
        String::from_utf8_lossy(&refused.stderr).contains("candidate not materialized"),
        "evidence must refuse an unmaterialized candidate, got {}",
        String::from_utf8_lossy(&refused.stderr)
    );

    // Restore, then record evidence against this candidate (C1). Ready to accept.
    fs::write(
        fixture.repository.join("src/lib.rs"),
        "pub fn foo() -> i32 { 2 }\n",
    )
    .unwrap();
    record_evidence();
    assert_eq!(preview()["ready_to_accept"], true);

    // Now mutate a further owned path: the candidate moves to C2, and the C1-bound
    // evidence no longer proves the committed bytes. Fail closed.
    apply("src/other.rs", "pub fn other() -> i32 { 2 }\n");
    let stale_binding = preview();
    assert_eq!(stale_binding["ready_to_accept"], false);
    assert!(
        stale_binding["readiness_reason"]
            .as_str()
            .unwrap()
            .contains("bound to the current candidate"),
        "preview should name the stale candidate binding, got {}",
        stale_binding["readiness_reason"]
    );
    assert_eq!(accept().state, TransactionState::Open);

    // Record evidence against the current candidate (C2): binding holds, accept
    // commits.
    record_evidence();
    assert_eq!(preview()["ready_to_accept"], true);
    assert_eq!(accept().state, TransactionState::Accepted);
}

fn claim_ids(claims: &[Claim]) -> Vec<u64> {
    claims.iter().map(|claim| claim.id).collect()
}

fn append_raw_event(workspace: &Path, event: Value) {
    let event_log = workspace.join("events.jsonl");
    let mut contents = fs::read_to_string(&event_log).unwrap();
    let sequence = contents.lines().count() as u64;
    contents.push_str(
        &serde_json::to_string(&serde_json::json!({
            "schema_version": 2,
            "sequence": sequence,
            "event": event,
        }))
        .unwrap(),
    );
    contents.push('\n');
    fs::write(event_log, contents).unwrap();
}

#[test]
fn in_repo_workspace_override_is_rejected_not_silently_forked() {
    // The original foreign-dogfood footgun: an agent passes `--workspace` at a
    // path *inside* the repo, which used to silently open a fresh state store
    // divorced from the git-identity workspace that orientation reads — a claim
    // recorded there never surfaces in `status`. The kernel must now refuse it
    // loudly (non-zero exit, explanatory stderr) rather than fork state. A
    // *sibling* state dir outside the repo stays valid and is exercised by every
    // other test in this file via `--workspace <root>/workspace-state`.
    let fixture = GitFixture::new();
    let in_repo = fixture.repository.join(".agent-workspace");

    let rejected = invoke_failure(&[
        "status",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        in_repo.to_str().unwrap(),
    ]);

    assert!(
        String::from_utf8_lossy(&rejected.stderr).contains("outside the repository"),
        "expected an in-repo --workspace to be rejected with guidance, got stderr: {}",
        String::from_utf8_lossy(&rejected.stderr)
    );
    assert!(
        !in_repo.exists(),
        "a rejected --workspace must not have created an in-repo state directory"
    );
}

fn invoke_with_stdin(arguments: &[&str], stdin: &str) -> Output {
    use std::io::Write;
    let mut child = Command::new(env!("CARGO_BIN_EXE_agent-workspace"))
        .args(arguments)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "command failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

/// `observe-read` on a whole-file read records a whole-file observation whose
/// model-visible byte count equals the file's, without the adapter computing a
/// byte range or fingerprint itself.
#[test]
fn observe_read_whole_file_records_whole_file_scope() {
    let fixture = GitFixture::with_files(&[("src/notes.txt", "alpha\nβeta\n")]);
    let workspace = fixture.root.path().join("workspace-state");
    let visible = "alpha\nβeta\n";

    let output = invoke_with_stdin(
        &[
            "observe-read",
            "--repository",
            fixture.repository.to_str().unwrap(),
            "--workspace",
            workspace.to_str().unwrap(),
            "--path",
            "src/notes.txt",
            "--provider",
            "claude-code.read",
        ],
        visible,
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["outcome"], "captured");
    assert_eq!(value["selector"]["kind"], "whole_file");
    assert_eq!(value["model_visible_bytes"], visible.len() as u64);
    assert_eq!(value["content"], visible);
}

/// A bounded read (one-indexed lines) maps to the exact UTF-8 byte range, and
/// the recorded content is the selected lines only. The adapter forwards the
/// raw selected text — chrome already stripped — and the kernel matches it
/// exactly; no harness presentation format lives in the kernel.
#[test]
fn observe_read_bounded_maps_lines_to_utf8_byte_range() {
    let fixture = GitFixture::with_files(&[("src/example.txt", "zero\nαlpha\nbeta\ntail\n")]);
    let workspace = fixture.root.path().join("workspace-state");
    let selected = "αlpha\nbeta";

    let output = invoke_with_stdin(
        &[
            "observe-read",
            "--repository",
            fixture.repository.to_str().unwrap(),
            "--workspace",
            workspace.to_str().unwrap(),
            "--path",
            "src/example.txt",
            "--offset",
            "2",
            "--limit",
            "2",
            "--model-visible-bytes",
            "64",
        ],
        selected,
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["outcome"], "captured");
    assert_eq!(value["selector"]["kind"], "byte_range");
    assert_eq!(value["selector"]["start"], "zero\n".len() as u64);
    assert_eq!(value["selector"]["end"], "zero\nαlpha\nbeta".len() as u64);
    assert_eq!(
        value["model_visible_bytes"], 64,
        "adapter may preserve model-boundary chrome bytes while matching stripped text"
    );
    assert_eq!(value["content"], selected);
}

/// The kernel is harness-agnostic: it does not tolerate any harness's chrome.
/// Un-stripped pagination-notice trailer that an earlier draft accepted now
/// fails closed — stripping presentation is the adapter's job, not the kernel's.
#[test]
fn observe_read_rejects_unstripped_harness_chrome() {
    let fixture = GitFixture::with_files(&[("src/example.txt", "zero\nαlpha\nbeta\ntail\n")]);
    let workspace = fixture.root.path().join("workspace-state");
    let with_chrome = "αlpha\nbeta\n\n[1 more lines in file. Use offset=4 to continue.]";

    let output = invoke_with_stdin(
        &[
            "observe-read",
            "--repository",
            fixture.repository.to_str().unwrap(),
            "--workspace",
            workspace.to_str().unwrap(),
            "--path",
            "src/example.txt",
            "--offset",
            "2",
            "--limit",
            "2",
        ],
        with_chrome,
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["outcome"], "skipped");
    assert_eq!(
        value["reason"],
        "model-visible read result does not match the current file selection"
    );
}

/// The capture fails closed on every unsafe condition, surfacing an explicit
/// skip reason instead of silently doing nothing: drift, truncation, a
/// non-positive offset, and a sensitive path.
#[test]
fn observe_read_fails_closed_with_explicit_skip_reasons() {
    let fixture =
        GitFixture::with_files(&[("src/data.txt", "before\n"), ("src/.env", "SECRET=1\n")]);
    let workspace = fixture.root.path().join("workspace-state");
    let base = |extra: &[&str]| -> Vec<String> {
        let mut arguments = vec![
            "observe-read".to_owned(),
            "--repository".to_owned(),
            fixture.repository.to_str().unwrap().to_owned(),
            "--workspace".to_owned(),
            workspace.to_str().unwrap().to_owned(),
        ];
        arguments.extend(extra.iter().map(|&value| value.to_owned()));
        arguments
    };
    let skip_reason = |arguments: &[String], stdin: &str| -> String {
        let borrowed: Vec<&str> = arguments.iter().map(String::as_str).collect();
        let output = invoke_with_stdin(&borrowed, stdin);
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["outcome"], "skipped");
        value["reason"].as_str().unwrap().to_owned()
    };

    assert_eq!(
        skip_reason(&base(&["--path", "src/data.txt"]), "after\n"),
        "model-visible read result does not match the current file selection"
    );
    assert_eq!(
        skip_reason(
            &base(&["--path", "src/data.txt", "--truncated"]),
            "before\n"
        ),
        "native read result was byte/line truncated"
    );
    assert_eq!(
        skip_reason(
            &base(&["--path", "src/data.txt", "--offset", "0"]),
            "before\n"
        ),
        "read offset is not a positive integer"
    );
    assert_eq!(
        skip_reason(&base(&["--path", "src/.env"]), "SECRET=1\n"),
        "path matches a sensitive-file pattern"
    );

    // A fail-closed capture records nothing: no observation event is appended.
    let log = fs::read_to_string(workspace.join("events.jsonl")).unwrap_or_default();
    assert!(
        !log.contains("observation_recorded"),
        "skipped reads must not append observation events: {log}"
    );
}

fn invoke_failure(arguments: &[&str]) -> Output {
    let output = Command::new(env!("CARGO_BIN_EXE_agent-workspace"))
        .args(arguments)
        .output()
        .unwrap();
    assert!(!output.status.success(), "command unexpectedly succeeded");
    output
}

/// The fused belief verb is the starvation fix: observations are captured
/// ambiently by adapters, but claims used to be a deliberate two-step CLI
/// detour, so observations piled up and claims almost never landed. The
/// fail-guard case matters as much as the happy path — if `record_belief`
/// captured a duplicate observation while a fresh one existed, it would re-fork
/// the two ledgers the verb exists to join.
#[test]
fn record_belief_reuses_a_fresh_ambient_observation_instead_of_duplicating_it() {
    let fixture = GitFixture::new();
    let workspace = fixture.root.path().join("workspace-state");
    let handle = Workspace::open(&fixture.repository, &workspace).unwrap();

    // The ambient capture an adapter's read hook would have recorded.
    let ambient = handle
        .capture_file_observation(
            "src/lib.rs",
            "adapter.capture",
            ObservationCaptureOptions::default(),
        )
        .unwrap();

    let belief = handle
        .record_belief(
            "foo returns one",
            &["src/lib.rs".into()],
            agent_workspace::ClaimScopeStrategy::Declared,
        )
        .unwrap();

    let support: &BeliefSupport = &belief.supports[0];
    assert_eq!(support.path, Path::new("src/lib.rs"));
    assert_eq!(support.observation_id, ambient.observation.id);
    assert!(
        support.reused,
        "a fresh observation must be reused, not duplicated"
    );
    assert_eq!(
        belief.claim.supporting_observation_ids,
        vec![ambient.observation.id]
    );
    assert_eq!(
        belief.claim.report.freshness_within_scope,
        FreshnessWithinScope::Current
    );
    // Exactly one observation exists: the ambient one, now joined to a claim.
    let status = handle.resume_status().unwrap();
    assert_eq!(status.observations.len(), 1);
    assert_eq!(status.claims.len(), 1);
    // The fusion focuses the supporting observation with the statement itself
    // as the reason, so the belief's provenance lands in the working set.
    let entry = status
        .working_set
        .iter()
        .find(|entry| entry.observation_id == ambient.observation.id)
        .expect("the belief's support is focused");
    assert_eq!(entry.reason, "foo returns one");
}

/// The write-back lag: observations recorded since the most recent claim. It is
/// the proprioceptive signal an adapter's orientation surfaces so a resuming
/// agent sees whether the last session sensed a lot but concluded little. It is
/// a "did I write anything back" signal, not per-observation coverage — any
/// claim resets it, even one that does not cite every outstanding observation.
#[test]
fn write_back_lag_counts_observations_since_the_last_claim() {
    let fixture = GitFixture::new();
    let workspace = fixture.root.path().join("workspace-state");
    let handle = Workspace::open(&fixture.repository, &workspace).unwrap();

    let lag = |handle: &Workspace| {
        handle
            .resume_brief_status()
            .unwrap()
            .observations_since_last_claim
    };

    // A fresh workspace has sensed nothing and owes nothing.
    assert_eq!(lag(&handle), 0);

    // Two ambient captures with no belief drawn: the lag climbs, one per read.
    handle
        .capture_file_observation(
            "src/lib.rs",
            "adapter.capture",
            ObservationCaptureOptions::default(),
        )
        .unwrap();
    assert_eq!(lag(&handle), 1);
    handle
        .capture_file_observation(
            "src/helper.rs",
            "adapter.capture",
            ObservationCaptureOptions::default(),
        )
        .unwrap();
    assert_eq!(lag(&handle), 2);

    // Recording a belief is writing back: the lag resets to zero even though the
    // uncited helper observation now sits behind the line. This is "did I
    // conclude anything", not "is every observation cited".
    handle
        .record_belief(
            "foo returns one",
            &["src/lib.rs".into()],
            agent_workspace::ClaimScopeStrategy::Declared,
        )
        .unwrap();
    assert_eq!(lag(&handle), 0);

    // A fresh observation after the belief starts the lag climbing again — it is
    // a live signal, not a one-way latch.
    fs::write(
        fixture.repository.join("src/lib.rs"),
        "pub fn foo() -> i32 { 42 }\n",
    )
    .unwrap();
    handle
        .capture_file_observation(
            "src/lib.rs",
            "adapter.capture",
            ObservationCaptureOptions::default(),
        )
        .unwrap();
    assert_eq!(lag(&handle), 1);
}

#[test]
fn record_belief_captures_when_no_observation_exists_and_stales_after_an_edit() {
    let fixture = GitFixture::new();
    let workspace = fixture.root.path().join("workspace-state");
    let handle = Workspace::open(&fixture.repository, &workspace).unwrap();

    let belief = handle
        .record_belief(
            "foo returns one",
            &["src/lib.rs".into()],
            agent_workspace::ClaimScopeStrategy::Declared,
        )
        .unwrap();
    assert!(!belief.supports[0].reused);
    assert_eq!(
        belief.claim.report.freshness_within_scope,
        FreshnessWithinScope::Current
    );

    fs::write(
        fixture.repository.join("src/lib.rs"),
        "pub fn foo() -> i32 { 2 }\n",
    )
    .unwrap();

    // The teeth: the belief is a real claim bound to real inputs, so the S1
    // staleness machinery turns it stale on an out-of-band edit.
    let status = handle.resume_status().unwrap();
    assert_eq!(status.claims.len(), 1);
    assert_eq!(
        status.claims[0].report.freshness_within_scope,
        FreshnessWithinScope::Stale
    );
}

#[test]
fn record_belief_re_observes_a_stale_dependency_rather_than_reusing_it() {
    let fixture = GitFixture::new();
    let workspace = fixture.root.path().join("workspace-state");
    let handle = Workspace::open(&fixture.repository, &workspace).unwrap();

    let ambient = handle
        .capture_file_observation(
            "src/lib.rs",
            "adapter.capture",
            ObservationCaptureOptions::default(),
        )
        .unwrap();
    // The file changed after the ambient capture: its observation is stale now.
    fs::write(
        fixture.repository.join("src/lib.rs"),
        "pub fn foo() -> i32 { 2 }\n",
    )
    .unwrap();

    let belief = handle
        .record_belief(
            "foo returns two",
            &["src/lib.rs".into()],
            agent_workspace::ClaimScopeStrategy::Declared,
        )
        .unwrap();

    assert!(
        !belief.supports[0].reused,
        "a stale observation must not be reused"
    );
    assert_ne!(belief.supports[0].observation_id, ambient.observation.id);
    assert_eq!(
        belief.claim.report.freshness_within_scope,
        FreshnessWithinScope::Current
    );
    assert_eq!(
        belief.claim.supporting_observation_ids,
        vec![belief.supports[0].observation_id]
    );
}

#[test]
fn record_belief_requires_a_citation() {
    let fixture = GitFixture::new();
    let workspace = fixture.root.path().join("workspace-state");
    let handle = Workspace::open(&fixture.repository, &workspace).unwrap();

    let error = handle
        .record_belief(
            "uncited",
            &[],
            agent_workspace::ClaimScopeStrategy::Declared,
        )
        .unwrap_err();
    assert!(
        error.to_string().contains("at least one cited path"),
        "unexpected error: {error}"
    );

    // The CLI enforces the same schema before the kernel is even invoked, so
    // the friction-free path cannot silently produce uncited bookkeeping.
    let output = Command::new(env!("CARGO_BIN_EXE_agent-workspace"))
        .args([
            "record-belief",
            "--repository",
            fixture.repository.to_str().unwrap(),
            "--workspace",
            workspace.to_str().unwrap(),
            "--statement",
            "uncited",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--rests-on"), "unexpected stderr: {stderr}");
}

#[test]
fn record_belief_cli_end_to_end_surfaces_the_claim_in_status() {
    let fixture = GitFixture::new();
    let workspace = fixture.root.path().join("workspace-state");

    let output = invoke(&[
        "record-belief",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
        "--statement",
        "foo returns one",
        "--rests-on",
        "src/lib.rs",
        "--rests-on",
        "src/helper.rs",
    ]);
    let belief: Belief = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(belief.supports.len(), 2);
    assert!(belief.supports.iter().all(|support| !support.reused));
    assert_eq!(
        belief.claim.report.freshness_within_scope,
        FreshnessWithinScope::Current
    );

    let status = invoke(&[
        "status",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
    ]);
    let status: Value = serde_json::from_slice(&status.stdout).unwrap();
    let claims = status["claims"].as_array().unwrap();
    assert_eq!(claims.len(), 1);
    assert_eq!(claims[0]["headline"], "foo returns one");
    assert_eq!(claims[0]["freshness"], "current");
}

fn invoke(arguments: &[&str]) -> Output {
    let output = Command::new(env!("CARGO_BIN_EXE_agent-workspace"))
        .args(arguments)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "command failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

struct GitFixture {
    root: TempDir,
    repository: std::path::PathBuf,
}

impl GitFixture {
    fn new() -> Self {
        Self::with_files(&[
            ("src/lib.rs", "pub fn foo() -> i32 { 1 }\n"),
            ("src/helper.rs", "pub fn helper() -> i32 { 1 }\n"),
        ])
    }

    fn with_task_source(source: &str) -> Self {
        Self::with_files(&[("src/task.rs", source)])
    }

    fn with_files(files: &[(&str, &str)]) -> Self {
        let root = TempDir::new().unwrap();
        let repository = root.path().join("repository");
        for (path, contents) in files {
            let path = repository.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, contents).unwrap();
        }

        git(&repository, &["init", "--quiet"]);
        git(
            &repository,
            &["config", "user.email", "fixture@example.invalid"],
        );
        git(&repository, &["config", "user.name", "Fixture"]);
        // Stage exactly the declared files rather than assuming a `src/`
        // directory — fixtures put files wherever the scenario needs them.
        for (path, _) in files {
            git(&repository, &["add", path]);
        }
        git(&repository, &["commit", "--quiet", "-m", "fixture"]);

        Self { root, repository }
    }
}

#[test]
fn state_root_is_shared_across_worktrees_and_separate_across_clones() {
    // One logical workspace per project: a linked worktree of the same
    // repository must resolve to the same external state root, while an
    // independent clone must not — identity comes from the git common
    // directory, never the remote URL.
    use agent_workspace::resolve_state_root;

    let state = TempDir::new().unwrap();
    let state_root = state.path();

    let primary = GitFixture::new();
    let worktree = primary.root.path().join("linked-worktree");
    git(
        &primary.repository,
        &["worktree", "add", "--quiet", worktree.to_str().unwrap()],
    );

    let clone = GitFixture::new();

    let resolve = |repo: &Path| resolve_state_root(repo, None, Some(state_root)).unwrap();

    let main_root = resolve(&primary.repository);
    let worktree_root = resolve(&worktree);
    let clone_root = resolve(&clone.repository);

    assert_eq!(
        main_root, worktree_root,
        "linked worktrees of one repository share state"
    );
    assert_ne!(
        main_root, clone_root,
        "independent clones must not share state"
    );
    assert!(
        main_root.starts_with(state_root),
        "resolved state lives under the external state root"
    );
}

#[test]
fn state_path_reports_the_resolved_root_without_creating_it() {
    // `state-path` is pure resolution for transparency: it must print where
    // state lives and touch nothing, so an adapter or human can inspect the
    // location before any workspace exists there.
    let fixture = GitFixture::new();
    let state = TempDir::new().unwrap();
    let base = state.path().join("root");

    let output = invoke(&[
        "state-path",
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--state-root",
        base.to_str().unwrap(),
    ]);
    let reported = String::from_utf8(output.stdout).unwrap();
    let reported = reported.trim();

    assert!(
        Path::new(reported).starts_with(&base),
        "reported path {reported} lives under the state root"
    );
    assert!(
        !base.exists(),
        "state-path must not create the state directory"
    );
}

#[test]
fn explicit_workspace_override_bypasses_resolution() {
    // The legacy `--workspace` path is honored verbatim so existing adapters
    // and fixtures keep working while they are repointed.
    use agent_workspace::resolve_state_root;

    let fixture = GitFixture::new();
    let explicit = fixture.root.path().join("verbatim-state");
    let state = TempDir::new().unwrap();

    let resolved = resolve_state_root(
        &fixture.repository,
        Some(explicit.as_path()),
        Some(state.path()),
    )
    .unwrap();

    assert_eq!(resolved, explicit);
}

fn git(repository: &Path, arguments: &[&str]) {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(repository)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
