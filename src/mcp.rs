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

use agent_workspace::{
    ClaimScopeStrategy, ReadCaptureOutcome, ReadCaptureRequest, Workspace, resolve_state_root,
};
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

/// Input schema for `workspace_bind_objective`, exposing the CLI
/// `bind-objective` verb over the same thin transport.
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct BindObjectiveParams {
    /// Why the current work exists, thesis-first.
    pub intent: String,
    /// Optional external authority reference (e.g. a Clearhead action id or URL).
    pub external_reference: Option<String>,
}

/// Input schema for `workspace_supersede_claim`, exposing the CLI
/// `supersede-claim` verb over the same thin transport.
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SupersedeClaimParams {
    /// Id of the active claim being retired.
    pub claim_id: u64,
    /// Id of the active replacement claim. Record the new belief first
    /// (`workspace_record_belief`), then cite it here.
    pub replacement_claim_id: u64,
    /// Why the old claim no longer holds. Empty reasons are rejected.
    pub reason: String,
}

/// Input schema for `workspace_checkpoint`, exposing the CLI `checkpoint`
/// verb over the same thin transport.
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CheckpointParams {
    /// Unique label for the checkpoint line; later deltas diff against it.
    pub label: String,
    /// Optional note recorded with the checkpoint.
    pub note: Option<String>,
}

/// Input schema for `workspace_observe_read`, exposing the CLI `observe-read`
/// verb so capture hooks can route through MCP instead of raw CLI stdin.
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ObserveReadParams {
    /// Repository-relative path the native read tool opened.
    pub path: String,
    /// Harness/provider identifier stamped on the observation (default `read`).
    pub provider: Option<String>,
    /// One-indexed first line the native read returned; omitted reads from the top.
    pub offset: Option<usize>,
    /// Line count the native read returned; omitted runs through end of file.
    pub limit: Option<usize>,
    /// The exact text the model saw, with harness chrome (line-number prefixes,
    /// pagination notices) already stripped. Matched against the file, not trusted.
    pub model_visible_text: String,
    /// Total bytes delivered at the model boundary, including stripped chrome.
    pub model_visible_bytes: Option<usize>,
    /// Whether the harness reported the native read result as truncated.
    #[serde(default)]
    pub truncated: bool,
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

    #[tool(
        description = "Bind (or rebind) the workspace objective: declare why the current work exists, with an optional reference to an external authority such as a Clearhead action. This records an ObjectiveBound event; a future status/delta will surface the intent and the transition."
    )]
    fn workspace_bind_objective(
        &self,
        Parameters(params): Parameters<BindObjectiveParams>,
    ) -> Result<CallToolResult, McpError> {
        match self.bind(params.intent, params.external_reference) {
            Ok(json) => Ok(CallToolResult::success(vec![ContentBlock::text(json)])),
            Err(message) => Ok(CallToolResult::error(vec![ContentBlock::text(message)])),
        }
    }

    #[tool(
        description = "Retire a claim that no longer holds: supersede_claim(claim_id, replacement_claim_id, reason). Record the revised belief first with workspace_record_belief, then retire the old claim citing the replacement. The reason is mandatory; an already-superseded or missing claim id is rejected strictly."
    )]
    fn workspace_supersede_claim(
        &self,
        Parameters(params): Parameters<SupersedeClaimParams>,
    ) -> Result<CallToolResult, McpError> {
        match self.supersede(params.claim_id, params.replacement_claim_id, params.reason) {
            Ok(json) => Ok(CallToolResult::success(vec![ContentBlock::text(json)])),
            Err(message) => Ok(CallToolResult::error(vec![ContentBlock::text(message)])),
        }
    }

    #[tool(
        description = "Draw a named line in the workspace log: reconcile all active claims to current truth, then record a checkpoint that a future session's delta diffs against. Labels must be unique. Use it to close a coherent slice of work."
    )]
    fn workspace_checkpoint(
        &self,
        Parameters(params): Parameters<CheckpointParams>,
    ) -> Result<CallToolResult, McpError> {
        match self.checkpoint(params.label, params.note) {
            Ok(json) => Ok(CallToolResult::success(vec![ContentBlock::text(json)])),
            Err(message) => Ok(CallToolResult::error(vec![ContentBlock::text(message)])),
        }
    }

    #[tool(
        description = "Record a read observation so the file gains provenance and a freshness signal: forward the exact text a read tool showed the model for path, with harness chrome already stripped. Skips are first-class and name the reason (truncated, sensitive path, not UTF-8, text drift) — surface it, don't retry blindly."
    )]
    fn workspace_observe_read(
        &self,
        Parameters(params): Parameters<ObserveReadParams>,
    ) -> Result<CallToolResult, McpError> {
        match self.observe(params) {
            Ok(json) => Ok(CallToolResult::success(vec![ContentBlock::text(json)])),
            Err(message) => Ok(CallToolResult::error(vec![ContentBlock::text(message)])),
        }
    }
}

impl WorkspaceServer {
    /// The in-process `open -> lock -> op` path, mirroring CLI dispatch. Runs
    /// `op` under the per-call exclusive lock and returns the kernel's JSON on
    /// success, or its (strict, input-naming) error text. Every tool below is a
    /// thin closure over this, so an MCP call and a CLI call stay identical.
    fn run<T, F>(&self, op: F) -> Result<String, String>
    where
        F: FnOnce(&Workspace) -> Result<T, agent_workspace::WorkspaceError>,
        T: serde::Serialize,
    {
        let root =
            resolve_state_root(&self.repository, None, None).map_err(|error| error.to_string())?;
        let workspace =
            Workspace::open(&self.repository, &root).map_err(|error| error.to_string())?;
        let _lock = workspace
            .lock_exclusive()
            .map_err(|error| error.to_string())?;
        let value = op(&workspace).map_err(|error| error.to_string())?;
        serde_json::to_string_pretty(&value).map_err(|error| error.to_string())
    }

    fn record(
        &self,
        statement: String,
        rests_on: &[PathBuf],
        scope: ClaimScopeStrategy,
    ) -> Result<String, String> {
        let rests_on: Vec<PathBuf> = rests_on.to_vec();
        self.run(move |workspace| workspace.record_belief(statement, &rests_on, scope))
    }

    fn bind(&self, intent: String, external_reference: Option<String>) -> Result<String, String> {
        self.run(move |workspace| workspace.bind_objective(intent, external_reference))
    }

    fn supersede(
        &self,
        claim_id: u64,
        replacement_claim_id: u64,
        reason: String,
    ) -> Result<String, String> {
        self.run(move |workspace| workspace.supersede_claim(claim_id, replacement_claim_id, reason))
    }

    fn checkpoint(&self, label: String, note: Option<String>) -> Result<String, String> {
        self.run(move |workspace| workspace.checkpoint(label, note))
    }

    /// Read capture maps `Skipped` to a first-class JSON outcome (never a silent
    /// no-op or a generic error), exactly as the CLI's `observe-read` prints it.
    fn observe(&self, params: ObserveReadParams) -> Result<String, String> {
        let request = ReadCaptureRequest {
            offset: params.offset,
            limit: params.limit,
            model_visible_text: params.model_visible_text,
            model_visible_bytes: params.model_visible_bytes,
            truncated: params.truncated,
        };
        self.run(move |workspace| {
            let outcome = workspace.capture_read_observation(
                &params.path,
                params.provider.unwrap_or_else(|| "read".to_owned()),
                request,
            )?;
            match outcome {
                ReadCaptureOutcome::Captured(capture) => {
                    let mut value = serde_json::to_value(&*capture)?;
                    if let Some(object) = value.as_object_mut() {
                        object.insert("outcome".to_owned(), serde_json::json!("captured"));
                    }
                    Ok(value)
                }
                ReadCaptureOutcome::Skipped { reason } => {
                    Ok(serde_json::json!({ "outcome": "skipped", "reason": reason }))
                }
            }
        })
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
            "Agent Workspace: bind the current objective (workspace_bind_objective), record \
             beliefs about code (workspace_record_belief, citing the files each rests on), \
             retire revised claims (workspace_supersede_claim), and checkpoint coherent \
             slices of work (workspace_checkpoint) so a future session gets a freshness \
             signal instead of silent staleness. Capture reads through \
             workspace_observe_read. A claim reported as stale outranks your remembered \
             belief."
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
