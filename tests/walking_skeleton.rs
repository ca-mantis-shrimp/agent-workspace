use agent_workspace::{
    Claim, ClaimInputSource, ClaimLifecycle, DeltaStatus, Evidence, FreshnessWithinScope,
    Normalizer, Objective, Observation, ObservationCapture, ObservationSelector,
    RevealedObservation, ScopeCompleteness, ScopeSource, Transaction, TransactionState,
    WorkspaceStatus,
};
use serde_json::Value;
use std::fs;
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
        Some("acceptance claims lack current passing evidence")
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
        &invoke(&["delta", "--repository", &repo, "--workspace", &ws]).stdout,
    )
    .unwrap();
    assert_eq!(latest.checkpoint.label, "second");
    assert_eq!(claim_ids(&latest.claims_recorded), vec![late.id]);

    let from_first: DeltaStatus = serde_json::from_slice(
        &invoke(&[
            "delta",
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
        &invoke(&["delta", "--repository", &repo, "--workspace", &ws]).stdout,
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

fn invoke_failure(arguments: &[&str]) -> Output {
    let output = Command::new(env!("CARGO_BIN_EXE_agent-workspace"))
        .args(arguments)
        .output()
        .unwrap();
    assert!(!output.status.success(), "command unexpectedly succeeded");
    output
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
        git(&repository, &["add", "src"]);
        git(&repository, &["commit", "--quiet", "-m", "fixture"]);

        Self { root, repository }
    }
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
