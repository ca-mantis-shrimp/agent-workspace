//! Acceptance coverage for knowledge bindings (knowledge pulse contract §8):
//! thin bindings with pinned sources, explicit applicability, and the bounded
//! status/delta pulse. KP12 (surface parity) lives in `mcp_stdio.rs`; KP14
//! (wake entry) arrives with the wake governs slice.

use agent_workspace::{
    BindingLifecycle, ClaimScopeStrategy, KnowledgeBindingRequest, KnowledgeReference, SourceState,
    Workspace,
};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;

const BOUNDARY_BODY: &str = "# Boundary\nThe source body sentence stays in its repository.\n";

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(dir)
        .status()
        .expect("run git");
    assert!(status.success(), "git {args:?} failed in {}", dir.display());
}

fn commit_repo(dir: &Path, files: &[(&str, &str)], message: &str) {
    for (path, contents) in files {
        let path = dir.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }
    git(dir, &["init", "--quiet"]);
    git(dir, &["config", "user.email", "fixture@example.invalid"]);
    git(dir, &["config", "user.name", "Fixture"]);
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "--quiet", "-m", message]);
}

/// A project repository plus a sibling knowledge repository at `../kb`.
struct Fixture {
    root: TempDir,
    project: PathBuf,
    state: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = TempDir::new().unwrap();
        let project = root.path().join("project");
        commit_repo(
            &project,
            &[
                ("src/lib.rs", "pub fn foo() -> i32 { 1 }\n"),
                ("src/legend.rs", "pub fn legend() {}\n"),
                ("docs/rule.md", "# Rule\nKeep modules pure.\n"),
            ],
            "project fixture",
        );
        commit_repo(
            &root.path().join("kb"),
            &[("decisions/boundary.md", BOUNDARY_BODY)],
            "knowledge fixture",
        );
        let state = root.path().join("state");
        Self {
            root,
            project,
            state,
        }
    }

    fn workspace(&self) -> Workspace {
        Workspace::open(&self.project, &self.state).unwrap()
    }

    fn kb(&self) -> PathBuf {
        self.root.path().join("kb")
    }

    /// A fresh kernel process against this workspace: no in-memory state.
    fn cli(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_agent-workspace"))
            .args(args)
            .args([
                "--repository",
                self.project.to_str().unwrap(),
                "--workspace",
                self.state.to_str().unwrap(),
            ])
            .output()
            .unwrap()
    }

    fn cli_json(&self, args: &[&str]) -> Value {
        let output = self.cli(args);
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    }

    fn brief(&self) -> Value {
        self.cli_json(&["status", "--compact"])
    }
}

const BOUNDARY_HEADLINE: &str =
    "Curated knowledge lives in OKF; the workspace holds situated state";

fn foreign_boundary() -> KnowledgeBindingRequest {
    KnowledgeBindingRequest {
        headline: BOUNDARY_HEADLINE.to_owned(),
        reference: Some(KnowledgeReference::RepositoryFile {
            repository: Some("../kb".to_owned()),
            path: "decisions/boundary.md".into(),
        }),
        ..KnowledgeBindingRequest::default()
    }
}

fn standalone(headline: &str) -> KnowledgeBindingRequest {
    KnowledgeBindingRequest {
        headline: headline.to_owned(),
        ..KnowledgeBindingRequest::default()
    }
}

fn project_file(headline: &str, path: &str) -> KnowledgeBindingRequest {
    KnowledgeBindingRequest {
        headline: headline.to_owned(),
        reference: Some(KnowledgeReference::RepositoryFile {
            repository: None,
            path: path.into(),
        }),
        ..KnowledgeBindingRequest::default()
    }
}

fn pulse_ids(brief: &Value) -> Vec<u64> {
    brief["knowledge"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["id"].as_u64().unwrap())
        .collect()
}

#[test]
fn kp1_a_cold_wake_pulses_the_governing_binding_without_a_recap() {
    let fixture = Fixture::new();
    let receipt = fixture.cli_json(&[
        "bind-knowledge",
        "--headline",
        BOUNDARY_HEADLINE,
        "--source-repository",
        "../kb",
        "--path",
        "decisions/boundary.md",
    ]);
    assert_eq!(receipt["authority"], "reference");
    assert_eq!(receipt["source"], "current");
    let id = receipt["id"].as_u64().unwrap();

    let brief = fixture.brief();
    let pulse = &brief["knowledge"][0];
    assert_eq!(pulse["id"], id);
    assert_eq!(pulse["headline"], BOUNDARY_HEADLINE);
    assert_eq!(pulse["authority"], "reference");
    assert_eq!(pulse["reference"], "../kb:decisions/boundary.md");
    assert_eq!(pulse["source"], "current");
    assert_eq!(pulse["why"], "repository");
    assert_eq!(brief["knowledge_omitted"], 0);
    assert_eq!(brief["counts"]["active_bindings"], 1);

    let revealed = fixture.cli_json(&["reveal", &format!("k{id}")]);
    assert_eq!(revealed["kind"], "knowledge");
    assert_eq!(revealed["record"]["headline"], BOUNDARY_HEADLINE);
}

#[test]
fn kp2_the_pulse_is_a_pointer_and_detail_is_one_step_away() {
    let fixture = Fixture::new();
    let workspace = fixture.workspace();
    let mut request = foreign_boundary();
    request.detail = Some("Full rationale lives in the decision.".to_owned());
    workspace.bind_knowledge(request).unwrap();

    let pulse = fixture.brief()["knowledge"][0].clone();
    assert!(pulse.get("detail").is_none(), "{pulse}");
    assert!(!pulse.to_string().contains("source body sentence"));

    let status = workspace.resume_status().unwrap();
    let binding = &status.knowledge[0];
    assert_eq!(
        binding.detail.as_deref(),
        Some("Full rationale lives in the decision.")
    );
    let pin = binding.pin.as_ref().unwrap();
    assert!(pin.revision.is_some());
    assert_eq!(pin.fingerprint.len(), 64);
    assert!(fixture.kb().join("decisions/boundary.md").is_file());

    let log = fs::read_to_string(workspace.event_log_path()).unwrap();
    assert!(
        !log.contains("source body sentence"),
        "source text must never enter workspace state"
    );
}

#[test]
fn kp3_a_revised_source_reports_changed_and_re_affirming_re_pins_it() {
    let fixture = Fixture::new();
    let workspace = fixture.workspace();
    let bound = workspace.bind_knowledge(foreign_boundary()).unwrap();
    workspace.checkpoint("c0", None).unwrap();

    fs::write(
        fixture.kb().join("decisions/boundary.md"),
        "# Boundary\nRevised.\n",
    )
    .unwrap();
    git(&fixture.kb(), &["commit", "--quiet", "-a", "-m", "revise"]);

    let status = workspace.resume_status().unwrap();
    let binding = &status.knowledge[0];
    assert_eq!(binding.source_state(), Some(SourceState::Changed));
    let pinned_revision = binding.pin.as_ref().unwrap().revision.clone().unwrap();
    assert!(
        binding
            .source
            .as_ref()
            .unwrap()
            .reason
            .contains(&pinned_revision)
    );
    assert_eq!(binding.lifecycle, BindingLifecycle::Active);
    assert_eq!(binding.headline, bound.headline);
    let delta = workspace.delta_brief_since(Some("c0")).unwrap();
    assert_eq!(delta.knowledge_source_changed.recent_ids, vec![bound.id]);

    let mut reaffirm = foreign_boundary();
    reaffirm.supersedes = Some(bound.id);
    let renewed = workspace.bind_knowledge(reaffirm).unwrap();
    assert_eq!(renewed.source_state(), Some(SourceState::Current));
    let delta = workspace.delta_brief_since(Some("c0")).unwrap();
    assert_eq!(delta.knowledge_established.recent_ids, vec![renewed.id]);
    assert_eq!(delta.knowledge_ended.recent_ids, vec![bound.id]);
}

#[test]
fn kp4_an_unavailable_provider_fails_honestly_and_recovers() {
    let fixture = Fixture::new();
    let workspace = fixture.workspace();
    let bound = workspace.bind_knowledge(foreign_boundary()).unwrap();
    workspace.checkpoint("c0", None).unwrap();

    let moved = fixture.root.path().join("kb-moved");
    fs::rename(fixture.kb(), &moved).unwrap();
    let pulse = fixture.brief()["knowledge"][0].clone();
    assert_eq!(pulse["source"], "unavailable");
    assert_eq!(pulse["headline"], BOUNDARY_HEADLINE);
    assert_eq!(pulse["reference"], "../kb:decisions/boundary.md");
    let source = workspace
        .reconcile_knowledge(bound.id)
        .unwrap()
        .source
        .unwrap();
    assert!(
        source.reason.contains("does not resolve"),
        "{}",
        source.reason
    );
    assert_eq!(
        workspace
            .delta_brief_since(Some("c0"))
            .unwrap()
            .knowledge_source_changed
            .recent_ids,
        vec![bound.id]
    );

    fs::rename(&moved, fixture.kb()).unwrap();
    assert_eq!(
        workspace
            .reconcile_knowledge(bound.id)
            .unwrap()
            .source_state(),
        Some(SourceState::Current)
    );

    // An unrelated repository at the same locator, even with identical bytes,
    // is not the pinned source.
    fs::rename(fixture.kb(), &moved).unwrap();
    commit_repo(
        &fixture.kb(),
        &[("decisions/boundary.md", BOUNDARY_BODY)],
        "an unrelated repository",
    );
    let impostor = workspace
        .reconcile_knowledge(bound.id)
        .unwrap()
        .source
        .unwrap();
    assert_eq!(impostor.state, SourceState::Unavailable);
    assert!(
        impostor.reason.contains("different repository"),
        "{}",
        impostor.reason
    );
}

#[test]
fn kp5_a_bare_binding_is_its_own_record() {
    let fixture = Fixture::new();
    let mut request = standalone("Keep modules pure; side effects at the edges");
    request.detail = Some("Data in, data out.".to_owned());
    let bound = fixture.workspace().bind_knowledge(request).unwrap();
    assert!(bound.brief().source.is_none());

    let pulse = fixture.brief()["knowledge"][0].clone();
    assert_eq!(pulse["authority"], "workspace");
    assert!(pulse.get("source").is_none(), "{pulse}");
    assert!(pulse.get("reference").is_none(), "{pulse}");
}

#[test]
fn kp6_a_path_scoped_binding_appears_only_while_work_touches_it() {
    let fixture = Fixture::new();
    let workspace = fixture.workspace();
    let mut request = standalone("Legend labels stay bounded");
    request.scope_paths = vec!["src/legend.rs".to_owned()];
    let bound = workspace.bind_knowledge(request).unwrap();

    let brief = fixture.brief();
    assert!(pulse_ids(&brief).is_empty());
    assert_eq!(brief["knowledge_omitted"], 0);
    assert_eq!(brief["counts"]["active_bindings"], 1);

    let belief = workspace
        .record_belief(
            "legend renders nothing yet",
            &["src/legend.rs".into()],
            ClaimScopeStrategy::Declared,
        )
        .unwrap();
    let brief = fixture.brief();
    assert_eq!(pulse_ids(&brief), vec![bound.id]);
    assert_eq!(
        brief["knowledge"][0]["why"],
        format!("path src/legend.rs via claim {}", belief.claim.id)
    );

    workspace
        .retire_claim(belief.claim.id, "work done")
        .unwrap();
    assert!(pulse_ids(&fixture.brief()).is_empty());
}

#[test]
fn kp7_ranking_puts_changed_sources_then_path_matches_then_newest() {
    let fixture = Fixture::new();
    let workspace = fixture.workspace();
    let changed = workspace
        .bind_knowledge(project_file("The rule document governs", "docs/rule.md"))
        .unwrap();
    workspace
        .bind_knowledge(standalone("Oldest repository rule"))
        .unwrap();
    let mut scoped = standalone("Source files follow the rule");
    scoped.scope_paths = vec!["src/".to_owned()];
    let scoped = workspace.bind_knowledge(scoped).unwrap();
    workspace
        .bind_knowledge(standalone("Newer repository rule"))
        .unwrap();
    let newest = workspace
        .bind_knowledge(standalone("Newest repository rule"))
        .unwrap();
    workspace
        .record_belief(
            "foo returns one",
            &["src/lib.rs".into()],
            ClaimScopeStrategy::Declared,
        )
        .unwrap();
    fs::write(fixture.project.join("docs/rule.md"), "# Rule\nEdited.\n").unwrap();

    let brief = fixture.brief();
    assert_eq!(pulse_ids(&brief), vec![changed.id, scoped.id, newest.id]);
    assert_eq!(brief["knowledge"][0]["source"], "changed");
    assert_eq!(brief["knowledge_omitted"], 2);
    assert_eq!(fixture.brief()["knowledge"], brief["knowledge"]);
}

#[test]
fn kp8_supersession_retirement_and_duplicates() {
    let fixture = Fixture::new();
    let workspace = fixture.workspace();
    let first = workspace.bind_knowledge(foreign_boundary()).unwrap();
    let duplicate = workspace.bind_knowledge(foreign_boundary()).unwrap_err();
    assert!(
        duplicate
            .to_string()
            .contains(&format!("already bound as k{}", first.id)),
        "{duplicate}"
    );

    let mut replacement = project_file("The rule document governs", "docs/rule.md");
    replacement.supersedes = Some(first.id);
    let replacement = workspace.bind_knowledge(replacement).unwrap();
    let status = workspace.resume_status().unwrap();
    assert_eq!(
        status
            .knowledge
            .iter()
            .map(|binding| binding.id)
            .collect::<Vec<_>>(),
        vec![replacement.id]
    );
    assert_eq!(
        status.ended_knowledge[0].lifecycle,
        BindingLifecycle::Superseded {
            replacement_binding_id: replacement.id
        }
    );
    assert_eq!(pulse_ids(&fixture.brief()), vec![replacement.id]);

    let retired = workspace
        .retire_knowledge(replacement.id, "no longer governs")
        .unwrap();
    assert_eq!(
        retired.lifecycle,
        BindingLifecycle::Retired {
            reason: "no longer governs".to_owned()
        }
    );
    assert!(workspace.retire_knowledge(replacement.id, "again").is_err());
    assert!(
        workspace
            .retire_knowledge(999, "missing")
            .unwrap_err()
            .to_string()
            .contains("knowledge binding 999 not found")
    );
    let mut stale_supersede = standalone("Replaces an ended binding");
    stale_supersede.supersedes = Some(first.id);
    assert!(workspace.bind_knowledge(stale_supersede).is_err());

    // Once every binding to a source has ended, it may be bound again.
    workspace.bind_knowledge(foreign_boundary()).unwrap();
}

#[test]
fn kp9_code_changes_never_touch_bindings() {
    let fixture = Fixture::new();
    let workspace = fixture.workspace();
    let rule = workspace
        .bind_knowledge(standalone("Keep modules pure"))
        .unwrap();
    let document = workspace
        .bind_knowledge(project_file("The rule document governs", "docs/rule.md"))
        .unwrap();

    fs::write(
        fixture.project.join("src/lib.rs"),
        "pub fn foo() -> i32 { 2 }\n",
    )
    .unwrap();
    git(
        &fixture.project,
        &["commit", "--quiet", "-a", "-m", "change code"],
    );

    let status = workspace.resume_status().unwrap();
    let by_id = |id| {
        status
            .knowledge
            .iter()
            .find(|binding| binding.id == id)
            .unwrap()
    };
    assert_eq!(by_id(rule.id).lifecycle, BindingLifecycle::Active);
    assert_eq!(by_id(rule.id).source_state(), None);
    assert_eq!(
        by_id(document.id).source_state(),
        Some(SourceState::Current)
    );
}

#[test]
fn kp10_source_state_is_worktree_relative() {
    let fixture = Fixture::new();
    let linked = fixture.root.path().join("linked");
    git(
        &fixture.project,
        &["worktree", "add", "--quiet", linked.to_str().unwrap()],
    );
    let primary = fixture.workspace();
    primary
        .bind_knowledge(project_file("The rule document governs", "docs/rule.md"))
        .unwrap();
    fs::write(linked.join("docs/rule.md"), "# Rule\nEdited in B.\n").unwrap();
    let secondary = Workspace::open(&linked, &fixture.state).unwrap();

    let state_in =
        |workspace: &Workspace| workspace.resume_status().unwrap().knowledge[0].source_state();
    assert_eq!(state_in(&secondary), Some(SourceState::Changed));
    assert_eq!(state_in(&primary), Some(SourceState::Current));
    assert_eq!(state_in(&secondary), Some(SourceState::Changed));
}

#[test]
fn kp11_a_log_without_knowledge_events_replays_empty() {
    let fixture = Fixture::new();
    let workspace = fixture.workspace();
    workspace.set_intent("ship the pulse", None).unwrap();
    workspace.checkpoint("c0", None).unwrap();

    let brief = fixture.brief();
    assert!(pulse_ids(&brief).is_empty());
    assert_eq!(brief["knowledge_omitted"], 0);
    assert_eq!(brief["counts"]["active_bindings"], 0);
    let delta = fixture.cli_json(&["delta", "--compact", "--since", "c0"]);
    for field in [
        "knowledge_established",
        "knowledge_ended",
        "knowledge_source_changed",
    ] {
        assert_eq!(delta[field]["total"], 0, "{field}");
    }
}

#[test]
fn kp13_invalid_writes_fail_by_name_and_append_nothing() {
    let fixture = Fixture::new();
    let workspace = fixture.workspace();
    fs::create_dir_all(fixture.root.path().join("plain")).unwrap();
    let log_lines = || {
        fs::read_to_string(workspace.event_log_path())
            .unwrap_or_default()
            .lines()
            .count()
    };
    let foreign = |repository: &str, path: &str| KnowledgeBindingRequest {
        headline: "Foreign rule".to_owned(),
        reference: Some(KnowledgeReference::RepositoryFile {
            repository: Some(repository.to_owned()),
            path: path.into(),
        }),
        ..KnowledgeBindingRequest::default()
    };
    let mut long_detail = standalone("Detail too long");
    long_detail.detail = Some("d".repeat(401));
    let mut too_many_prefixes = standalone("Too many prefixes");
    too_many_prefixes.scope_paths = (0..9).map(|n| format!("src/{n}/")).collect();
    let mut absolute_prefix = standalone("Absolute prefix");
    absolute_prefix.scope_paths = vec!["/src/".to_owned()];
    let mut missing_supersede = standalone("Replaces nothing");
    missing_supersede.supersedes = Some(999);
    let blank_locator = KnowledgeBindingRequest {
        headline: "Blank locator".to_owned(),
        reference: Some(KnowledgeReference::Opaque {
            locator: "  ".to_owned(),
        }),
        ..KnowledgeBindingRequest::default()
    };

    let cases = vec![
        (standalone("   "), "headline must not be blank"),
        (standalone(&"h".repeat(121)), "headline is 121 chars"),
        (long_detail, "detail is 401 chars"),
        (project_file("Absolute", "/etc/hosts"), "must be relative"),
        (
            project_file("Escaping", "../kb/decisions/boundary.md"),
            "must be relative",
        ),
        (project_file("Sensitive", ".env"), "sensitive"),
        (
            project_file("Missing", "docs/missing.md"),
            "not a regular file",
        ),
        (foreign("../nowhere", "x.md"), "does not resolve"),
        (foreign("../plain", "x.md"), "not a Git repository"),
        (too_many_prefixes, "at most 8 path prefixes"),
        (absolute_prefix, "must be repository-relative"),
        (missing_supersede, "knowledge binding 999 not found"),
        (blank_locator, "locator must not be blank"),
    ];
    for (request, message) in cases {
        let before = log_lines();
        let error = workspace.bind_knowledge(request).unwrap_err().to_string();
        assert!(error.contains(message), "expected {message:?} in {error:?}");
        assert_eq!(log_lines(), before, "a rejected write must append nothing");
    }

    let both = KnowledgeReference::from_parts(
        Some("docs/rule.md".into()),
        None,
        Some("https://example.invalid".to_owned()),
    )
    .unwrap_err();
    assert!(both.to_string().contains("at most one reference"));
}
