//! Acceptance coverage for the MCP front door: drive the server over stdio the
//! way a real MCP client (Claude Code, Cursor, ...) does, and assert the tool is
//! routed, a belief lands, and a bad citation is rejected strictly.
//!
//! Gated on the `mcp` feature so a default `cargo test` stays on the small,
//! synchronous kernel. Run with `cargo test --features mcp`.
#![cfg(feature = "mcp")]

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(dir)
        .status()
        .expect("run git");
    assert!(status.success(), "git {args:?} failed");
}

/// A throwaway committed repo with one tracked file to cite.
fn make_repo(dir: &Path) {
    git(dir, &["init", "-q"]);
    git(dir, &["config", "user.email", "t@t"]);
    git(dir, &["config", "user.name", "t"]);
    std::fs::write(dir.join("hello.txt"), "hello world\n").unwrap();
    git(dir, &["add", "."]);
    git(dir, &["commit", "-qm", "init"]);
}

struct Server {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl Server {
    fn send(&mut self, message: Value) {
        writeln!(self.stdin, "{message}").unwrap();
        self.stdin.flush().unwrap();
    }

    /// Read newline-delimited JSON-RPC until a response with the given id.
    fn recv_id(&mut self, id: i64) -> Value {
        loop {
            let mut line = String::new();
            let read = self.stdout.read_line(&mut line).unwrap();
            assert!(read > 0, "server closed before responding to id {id}");
            let value: Value = serde_json::from_str(line.trim()).unwrap();
            if value.get("id").and_then(Value::as_i64) == Some(id) {
                return value;
            }
        }
    }
}

impl Server {
    /// Initialize, complete the handshake, and return the advertised tool names.
    fn handshake(&mut self) -> Vec<String> {
        self.send(json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {"protocolVersion": "2025-06-18", "capabilities": {},
                       "clientInfo": {"name": "test", "version": "0"}}
        }));
        let init = self.recv_id(1);
        assert_eq!(init["result"]["serverInfo"]["name"], "agent-workspace");
        self.send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
        self.send(json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}));
        let tools = self.recv_id(2);
        tools["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_owned())
            .collect()
    }

    /// Call a tool and return the JSON-RPC response.
    fn call(&mut self, id: i64, name: &str, arguments: Value) -> Value {
        self.send(json!({
            "jsonrpc": "2.0", "id": id, "method": "tools/call",
            "params": {"name": name, "arguments": arguments}
        }));
        self.recv_id(id)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn run_cli(repo: &Path, state: &Path, args: &[&str]) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_agent-workspace"))
        .args(args)
        .args(["--repository", repo.to_str().unwrap()])
        .env("XDG_STATE_HOME", state)
        .output()
        .expect("run agent-workspace CLI");
    assert!(
        output.status.success(),
        "CLI {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("CLI returned JSON")
}

fn tool_json(response: &Value) -> Value {
    let text = response["result"]["content"][0]["text"].as_str().unwrap();
    serde_json::from_str(text).expect("tool result text is JSON")
}

fn start(repo: &Path, state: &Path) -> Server {
    let mut child = Command::new(env!("CARGO_BIN_EXE_agent-workspace"))
        .args(["mcp", "--repository", repo.to_str().unwrap()])
        .env("XDG_STATE_HOME", state) // isolate the kernel state store
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn mcp server");
    let stdin = child.stdin.take().unwrap();
    let stdout = BufReader::new(child.stdout.take().unwrap());
    Server {
        child,
        stdin,
        stdout,
    }
}

#[test]
fn mcp_server_projects_the_complete_read_surface_over_stdio() {
    let repo = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    make_repo(repo.path());
    let mut server = start(repo.path(), state.path());

    let names = server.handshake();
    for name in [
        "workspace_status",
        "workspace_delta",
        "workspace_working_set",
        "workspace_findings",
        "workspace_transaction_preview",
    ] {
        assert!(
            names.contains(&name.to_owned()),
            "{name} not routed; got {names:?}"
        );
    }

    let bound = server.call(
        3,
        "workspace_bind_objective",
        json!({"intent": "exercise the MCP read surface"}),
    );
    assert_eq!(bound["result"]["isError"], json!(false));
    let belief = server.call(
        4,
        "workspace_record_belief",
        json!({"statement": "hello.txt contains the fixture greeting",
               "rests_on": ["hello.txt"]}),
    );
    let claim_id = tool_json(&belief)["id"].as_u64().unwrap();
    let transaction = run_cli(
        repo.path(),
        state.path(),
        &[
            "begin-transaction",
            "--intent",
            "preview me",
            "--claim",
            &claim_id.to_string(),
        ],
    );
    let transaction_id = transaction["id"].as_u64().unwrap();
    let checkpoint = server.call(
        5,
        "workspace_checkpoint",
        json!({"label": "read-surface-baseline"}),
    );
    assert_eq!(checkpoint["result"]["isError"], json!(false));

    let brief_status = tool_json(&server.call(6, "workspace_status", json!({})));
    assert_eq!(
        brief_status["objective"]["intent"],
        "exercise the MCP read surface"
    );
    assert_eq!(brief_status["counts"]["open_transactions"], 1);
    let full_status = tool_json(&server.call(7, "workspace_status", json!({"full": true})));
    assert!(full_status["observations"].as_array().is_some());

    let brief_delta = tool_json(&server.call(8, "workspace_delta", json!({})));
    assert_eq!(brief_delta["checkpoint"]["label"], "read-surface-baseline");
    let full_delta = tool_json(&server.call(
        9,
        "workspace_delta",
        json!({"full": true, "since": "read-surface-baseline"}),
    ));
    assert_eq!(full_delta["checkpoint"]["label"], "read-surface-baseline");

    let working_set = tool_json(&server.call(10, "workspace_working_set", json!({})));
    assert!(working_set["locations"].as_array().is_some());
    let findings = tool_json(&server.call(11, "workspace_findings", json!({})));
    assert!(findings["open"].as_array().is_some());

    let preview = tool_json(&server.call(
        12,
        "workspace_transaction_preview",
        json!({"transaction": transaction_id}),
    ));
    assert_eq!(preview["id"], transaction_id);
    let missing = server.call(
        13,
        "workspace_transaction_preview",
        json!({"transaction": transaction_id + 999}),
    );
    assert_eq!(missing["result"]["isError"], json!(true));
    assert!(
        missing["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("not found")
    );
}

#[test]
fn mcp_server_records_a_belief_over_stdio() {
    let repo = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    make_repo(repo.path());
    let mut server = start(repo.path(), state.path());

    let names = server.handshake();
    assert!(
        names.contains(&"workspace_record_belief".to_owned()),
        "tool not routed; got {names:?}"
    );

    // A well-cited belief lands.
    let ok = server.call(
        3,
        "workspace_record_belief",
        json!({"statement": "hello.txt greets the world",
               "rests_on": ["hello.txt"]}),
    );
    assert_eq!(
        ok["result"]["isError"],
        json!(false),
        "record_belief should succeed: {ok}"
    );
    // The default receipt is compact: the id, the freshness verdict, and
    // per-path reuse — not the audit-grade claim record. The heavy inputs and
    // operational coverage must not be echoed into the working context.
    let text = ok["result"]["content"][0]["text"].as_str().unwrap();
    let brief: Value = serde_json::from_str(text).unwrap();
    assert!(
        brief["id"].is_u64() && brief["freshness"].is_string(),
        "expected a compact receipt {{id, freshness, supports}}: {text}"
    );
    assert_eq!(
        brief["supports"][0]["path"],
        json!("hello.txt"),
        "compact receipt must carry per-path reuse: {text}"
    );
    assert!(
        brief.get("claim").is_none() && !text.contains("recorded_input_fingerprint"),
        "compact receipt must omit the audit record: {text}"
    );

    // `full: true` still returns the whole audit record for the rare caller
    // that wants it.
    let full = server.call(
        5,
        "workspace_record_belief",
        json!({"statement": "hello.txt greets the world, in full",
               "rests_on": ["hello.txt"], "full": true}),
    );
    let full_text = full["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        full_text.contains("\"claim\"") && full_text.contains("recorded_input_fingerprint"),
        "full receipt must carry the audit record: {full_text}"
    );

    // A belief citing a file that does not exist is rejected strictly, and the
    // kernel's error reaches the client as a tool-level error (never softened).
    let bad = server.call(
        4,
        "workspace_record_belief",
        json!({"statement": "cites a ghost",
               "rests_on": ["does-not-exist.txt"]}),
    );
    assert_eq!(
        bad["result"]["isError"],
        json!(true),
        "bad citation must be an error: {bad}"
    );
}

#[test]
fn mcp_server_names_a_missing_repository_instead_of_a_bare_io_error() {
    // Simulate the client starting the server with a `--repository` that never
    // resolved (e.g. an unexpanded `${CLAUDE_PROJECT_DIR}` literal): the parent
    // exists but the target path does not. The agent must get a named,
    // actionable error, not a context-free "No such file or directory".
    let parent = tempfile::tempdir().unwrap();
    let missing = parent.path().join("unexpanded-project-dir");
    let state = tempfile::tempdir().unwrap();
    let mut server = start(&missing, state.path());

    // Startup is intentionally non-fatal, so tools still list and the per-call
    // error is what reaches the agent.
    let names = server.handshake();
    assert!(
        names.contains(&"workspace_record_belief".to_owned()),
        "tools should still list with a bad repository; got {names:?}"
    );

    let bad = server.call(
        3,
        "workspace_record_belief",
        json!({"statement": "anything", "rests_on": ["src/lib.rs"]}),
    );
    assert_eq!(
        bad["result"]["isError"],
        json!(true),
        "a missing repository must surface as an error: {bad}"
    );
    let text = bad["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("does not exist") && text.contains("unexpanded-project-dir"),
        "error must name the bad path and its cause, got: {text}"
    );
}

#[test]
fn mcp_server_binds_an_objective_over_stdio() {
    let repo = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    make_repo(repo.path());
    let mut server = start(repo.path(), state.path());

    let names = server.handshake();
    assert!(
        names.contains(&"workspace_bind_objective".to_owned()),
        "bind-objective tool not routed; got {names:?}"
    );

    // Binding an objective records it and returns the projected Objective.
    let ok = server.call(
        3,
        "workspace_bind_objective",
        json!({"intent": "write-api slice 2",
               "external_reference": "clearhead:01a06f11"}),
    );
    assert_eq!(
        ok["result"]["isError"],
        json!(false),
        "bind_objective should succeed: {ok}"
    );
    let text = ok["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("\"intent\""),
        "expected an objective in the result: {text}"
    );

    // An empty intent is rejected strictly so bad objectives cannot land silently.
    let bad = server.call(4, "workspace_bind_objective", json!({"intent": "   "}));
    assert_eq!(
        bad["result"]["isError"],
        json!(true),
        "empty intent must be an error: {bad}"
    );
}

#[test]
fn mcp_server_supersedes_a_claim_over_stdio() {
    let repo = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    make_repo(repo.path());
    let mut server = start(repo.path(), state.path());

    let names = server.handshake();
    assert!(
        names.contains(&"workspace_supersede_claim".to_owned()),
        "supersede tool not routed; got {names:?}"
    );

    // Two active claims to chain: 2 replaces 1.
    let first = server.call(
        3,
        "workspace_record_belief",
        json!({"statement": "one", "rests_on": ["hello.txt"]}),
    );
    let second = server.call(
        4,
        "workspace_record_belief",
        json!({"statement": "two", "rests_on": ["hello.txt"]}),
    );
    let claim = |response: &Value| -> u64 {
        let text = response["result"]["content"][0]["text"].as_str().unwrap();
        let belief: Value = serde_json::from_str(text).unwrap();
        belief["id"].as_u64().unwrap()
    };
    let (id1, id2) = (claim(&first), claim(&second));

    // Supersession lands and returns the retired claim.
    let ok = server.call(
        5,
        "workspace_supersede_claim",
        json!({"claim_id": id1, "replacement_claim_id": id2,
               "reason": "revised after re-reading"}),
    );
    assert_eq!(
        ok["result"]["isError"],
        json!(false),
        "supersede should succeed: {ok}"
    );
    // The compact receipt names the id and the new disposition — which for a
    // supersession already carries the replacement id — and omits the audit
    // record.
    let text = ok["result"]["content"][0]["text"].as_str().unwrap();
    let brief: Value = serde_json::from_str(text).unwrap();
    assert_eq!(
        brief["id"],
        json!(id1),
        "receipt names the superseded id: {text}"
    );
    assert_eq!(
        brief["lifecycle"]["replacement_claim_id"],
        json!(id2),
        "compact receipt must name the replacement: {text}"
    );
    assert!(
        !text.contains("recorded_input_fingerprint"),
        "compact receipt must omit the audit record: {text}"
    );

    // Re-superseding a retired claim is rejected strictly.
    let bad = server.call(
        6,
        "workspace_supersede_claim",
        json!({"claim_id": id1, "replacement_claim_id": id2, "reason": "again"}),
    );
    assert_eq!(
        bad["result"]["isError"],
        json!(true),
        "double supersede must be an error: {bad}"
    );
}

#[test]
fn mcp_server_retires_a_claim_without_a_replacement_over_stdio() {
    let repo = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    make_repo(repo.path());
    let mut server = start(repo.path(), state.path());

    let names = server.handshake();
    assert!(
        names.contains(&"workspace_retire_claim".to_owned()),
        "retire tool not routed; got {names:?}"
    );

    let recorded = server.call(
        3,
        "workspace_record_belief",
        json!({"statement": "a belief whose work is done", "rests_on": ["hello.txt"]}),
    );
    let text = recorded["result"]["content"][0]["text"].as_str().unwrap();
    let id = serde_json::from_str::<Value>(text).unwrap()["id"]
        .as_u64()
        .unwrap();

    // Retirement lands with no replacement id, and reports the retired lifecycle.
    let ok = server.call(
        4,
        "workspace_retire_claim",
        json!({"claim_id": id, "reason": "subject work completed"}),
    );
    assert_eq!(
        ok["result"]["isError"],
        json!(false),
        "retire should succeed: {ok}"
    );
    let ok_text = ok["result"]["content"][0]["text"].as_str().unwrap();
    let brief: Value = serde_json::from_str(ok_text).unwrap();
    assert_eq!(
        brief["id"],
        json!(id),
        "receipt names the retired id: {ok_text}"
    );
    assert_eq!(
        brief["lifecycle"]["state"],
        json!("retired"),
        "expected a retired lifecycle: {ok_text}"
    );
    assert!(
        !ok_text.contains("recorded_input_fingerprint"),
        "compact receipt must omit the audit record: {ok_text}"
    );

    // Retiring an already-inactive claim is rejected strictly.
    let bad = server.call(
        5,
        "workspace_retire_claim",
        json!({"claim_id": id, "reason": "again"}),
    );
    assert_eq!(
        bad["result"]["isError"],
        json!(true),
        "double retire must be an error: {bad}"
    );
}

#[test]
fn mcp_server_checkpoints_and_captures_reads_over_stdio() {
    let repo = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    make_repo(repo.path());
    let mut server = start(repo.path(), state.path());

    let names = server.handshake();
    assert!(
        names.contains(&"workspace_checkpoint".to_owned())
            && names.contains(&"workspace_observe_read".to_owned()),
        "checkpoint/observe tools not routed; got {names:?}"
    );

    // A faithful read capture lands; a truncated one skips with its reason
    // (first-class, never a silent no-op or a generic error).
    let ok = server.call(
        3,
        "workspace_observe_read",
        json!({"path": "hello.txt", "model_visible_text": "hello world\n"}),
    );
    assert_eq!(
        ok["result"]["isError"],
        json!(false),
        "observe_read should succeed: {ok}"
    );
    let text = ok["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("\"outcome\": \"captured\""),
        "expected a captured observation: {text}"
    );

    let skipped = server.call(
        4,
        "workspace_observe_read",
        json!({"path": "hello.txt", "model_visible_text": "hello world\n",
               "truncated": true}),
    );
    assert_eq!(skipped["result"]["isError"], json!(false));
    let text = skipped["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("\"outcome\": \"skipped\"") && text.contains("truncated"),
        "expected a first-class skip: {text}"
    );

    // A checkpoint draws the line; a duplicate label is rejected strictly.
    let ok = server.call(
        5,
        "workspace_checkpoint",
        json!({"label": "slice-done", "note": "verbs exposed"}),
    );
    assert_eq!(
        ok["result"]["isError"],
        json!(false),
        "checkpoint should succeed: {ok}"
    );
    let text = ok["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("slice-done") && text.contains("git_revision"),
        "expected a checkpoint marker: {text}"
    );

    let bad = server.call(6, "workspace_checkpoint", json!({"label": "slice-done"}));
    assert_eq!(
        bad["result"]["isError"],
        json!(true),
        "duplicate label must be an error: {bad}"
    );
}
