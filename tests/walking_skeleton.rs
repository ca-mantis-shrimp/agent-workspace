use agent_workspace::{
    Claim, ClaimInputSource, Evidence, FreshnessWithinScope, Observation, ScopeCompleteness,
    ScopeSource, Transaction, TransactionState, WorkspaceStatus,
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
    let legacy_log = fs::read_to_string(&event_log_path)
        .unwrap()
        .replace("\"schema_version\":2", "\"schema_version\":1")
        .replace("reconciliation_fingerprint", "repository_fingerprint");
    fs::write(&event_log_path, legacy_log).unwrap();
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
        "--repository",
        fixture.repository.to_str().unwrap(),
        "--workspace",
        workspace.to_str().unwrap(),
    ]);
    let resumed_again: WorkspaceStatus = serde_json::from_slice(&resumed_again.stdout).unwrap();
    assert_eq!(resumed, resumed_again);
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
        let root = TempDir::new().unwrap();
        let repository = root.path().join("repository");
        fs::create_dir_all(repository.join("src")).unwrap();
        fs::write(repository.join("src/lib.rs"), "pub fn foo() -> i32 { 1 }\n").unwrap();
        fs::write(
            repository.join("src/helper.rs"),
            "pub fn helper() -> i32 { 1 }\n",
        )
        .unwrap();

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
