//! MCP front door: exposes the workspace verbs over the harness-agnostic Model
//! Context Protocol, so any MCP client (Claude Code, Cursor, Zed, ...) gets the
//! same first-class write surface the Pi adapter has as native tools — instead
//! of shelling the raw CLI. Unlike the per-harness adapters, one stdio server
//! serves them all; this subsumes adapters rather than adding one.
//!
//! The kernel owns semantics; this module is a thin *in-process* transport. Each
//! tool call replays the same `open -> lock -> op` path a CLI invocation runs, so
//! an MCP call and an `agent-workspace <verb>` call are semantically identical
//! (fresh log replay, per-call exclusive lock). Async is contained here: `fn
//! main` stays synchronous and only the `mcp` subcommand builds a runtime and
//! blocks on it.

use std::path::PathBuf;

use agent_workspace::{ClaimScopeStrategy, Workspace, resolve_state_root};
use rmcp::{
    ErrorData as McpError, ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router,
    transport::stdio,
};

/// The MCP server. Holds only the target repository; all state lives in the
/// kernel's external store, resolved per call exactly as the CLI resolves it.
#[derive(Clone)]
pub struct WorkspaceServer {
    repository: PathBuf,
    // Read by the `#[tool_handler]`-generated `ServerHandler` methods; the
    // dead-code pass can't see through the macro, hence the allow.
    #[allow(dead_code)]
    tool_router: ToolRouter<WorkspaceServer>,
}

/// Input schema for `workspace_record_belief`, mirroring the Pi tool one-for-one
/// so the two adapters present a single surface.
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct RecordBeliefParams {
    /// The belief itself, thesis-first: the assertion you are staking on the cited files.
    pub statement: String,
    /// Cited supporting paths (required, non-empty), each repository-relative.
    pub rests_on: Vec<String>,
    /// Claim scope: `declared` (default) binds only the cited paths;
    /// `conservative-siblings` also fingerprints their repository siblings.
    pub scope: Option<String>,
}

#[tool_router]
impl WorkspaceServer {
    pub fn new(repository: PathBuf) -> Self {
        Self {
            repository,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        description = "Record a belief — the fused write verb: \"I now believe X, and it rests on files Y, Z.\" Replaces the raw-CLI observe-then-claim two-step. For each rests_on path the kernel reuses the freshest current observation (typically your ambient read captures) or else captures the whole file. Citation is mandatory: at least one rests_on path is required — a belief you cannot cite cannot be recorded. Rejections are strict and name the failed inputs; re-read the named file, then re-record. A claim the workspace later reports as stale outranks your remembered belief."
    )]
    fn workspace_record_belief(
        &self,
        Parameters(params): Parameters<RecordBeliefParams>,
    ) -> Result<CallToolResult, McpError> {
        let scope = match params.scope.as_deref() {
            Some("conservative-siblings") => ClaimScopeStrategy::ConservativeSiblingFiles,
            _ => ClaimScopeStrategy::Declared,
        };
        let rests_on: Vec<PathBuf> = params.rests_on.iter().map(PathBuf::from).collect();

        // The kernel's rejection is the product; surface it verbatim as a
        // tool-level error (never softened), so the agent sees which inputs
        // drifted, not a generic failure.
        match self.record(params.statement, &rests_on, scope) {
            Ok(json) => Ok(CallToolResult::success(vec![ContentBlock::text(json)])),
            Err(message) => Ok(CallToolResult::error(vec![ContentBlock::text(message)])),
        }
    }
}

impl WorkspaceServer {
    /// The in-process `open -> lock -> op` path, mirroring CLI dispatch. Returns
    /// the kernel's JSON on success, or its (strict, input-naming) error text.
    fn record(
        &self,
        statement: String,
        rests_on: &[PathBuf],
        scope: ClaimScopeStrategy,
    ) -> Result<String, String> {
        let root =
            resolve_state_root(&self.repository, None, None).map_err(|error| error.to_string())?;
        let workspace =
            Workspace::open(&self.repository, &root).map_err(|error| error.to_string())?;
        let _lock = workspace
            .lock_exclusive()
            .map_err(|error| error.to_string())?;
        let belief = workspace
            .record_belief(statement, rests_on, scope)
            .map_err(|error| error.to_string())?;
        serde_json::to_string_pretty(&belief).map_err(|error| error.to_string())
    }
}

#[tool_handler]
impl ServerHandler for WorkspaceServer {
    fn get_info(&self) -> ServerInfo {
        // `ServerInfo`/`Implementation` are `#[non_exhaustive]`, so build from
        // the default and set the fields we care about.
        let mut server_info = Implementation::default();
        server_info.name = "agent-workspace".to_owned();
        server_info.version = env!("CARGO_PKG_VERSION").to_owned();

        let mut info = ServerInfo::default();
        info.server_info = server_info;
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info.instructions = Some(
            "Agent Workspace: record beliefs about code (workspace_record_belief), citing the \
             files each rests on, so a future session gets a freshness signal instead of silent \
             staleness. A claim reported as stale outranks your remembered belief."
                .to_owned(),
        );
        info
    }
}

/// Serve the workspace over stdio. Builds a contained current-thread runtime so
/// the rest of the binary stays synchronous; blocks until the client hangs up.
pub fn serve(repository: PathBuf) -> Result<(), std::io::Error> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()?;
    runtime.block_on(async move {
        let service = WorkspaceServer::new(repository)
            .serve(stdio())
            .await
            .map_err(std::io::Error::other)?;
        service.waiting().await.map_err(std::io::Error::other)?;
        Ok(())
    })
}
