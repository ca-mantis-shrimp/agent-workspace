use agent_workspace::{FreshnessWithinScope, Observation, ScopeCompleteness, ScopeSource};
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

        git(&repository, &["init", "--quiet"]);
        git(
            &repository,
            &["config", "user.email", "fixture@example.invalid"],
        );
        git(&repository, &["config", "user.name", "Fixture"]);
        git(&repository, &["add", "src/lib.rs"]);
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
