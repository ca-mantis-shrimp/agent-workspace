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

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
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
fn mcp_server_records_a_belief_over_stdio() {
    let repo = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    make_repo(repo.path());
    let mut server = start(repo.path(), state.path());

    // Handshake.
    server.send(json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {"protocolVersion": "2025-06-18", "capabilities": {},
                   "clientInfo": {"name": "test", "version": "0"}}
    }));
    let init = server.recv_id(1);
    assert_eq!(init["result"]["serverInfo"]["name"], "agent-workspace");
    server.send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));

    // The tool is routed and carries its schema.
    server.send(json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}));
    let tools = server.recv_id(2);
    let names: Vec<&str> = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(
        names.contains(&"workspace_record_belief"),
        "tool not routed; got {names:?}"
    );

    // A well-cited belief lands.
    server.send(json!({
        "jsonrpc": "2.0", "id": 3, "method": "tools/call",
        "params": {"name": "workspace_record_belief",
                   "arguments": {"statement": "hello.txt greets the world",
                                 "rests_on": ["hello.txt"]}}
    }));
    let ok = server.recv_id(3);
    assert_eq!(
        ok["result"]["isError"],
        json!(false),
        "record_belief should succeed: {ok}"
    );
    let text = ok["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("\"claim\""),
        "expected a claim in the result: {text}"
    );

    // A belief citing a file that does not exist is rejected strictly, and the
    // kernel's error reaches the client as a tool-level error (never softened).
    server.send(json!({
        "jsonrpc": "2.0", "id": 4, "method": "tools/call",
        "params": {"name": "workspace_record_belief",
                   "arguments": {"statement": "cites a ghost",
                                 "rests_on": ["does-not-exist.txt"]}}
    }));
    let bad = server.recv_id(4);
    assert_eq!(
        bad["result"]["isError"],
        json!(true),
        "bad citation must be an error: {bad}"
    );
}
