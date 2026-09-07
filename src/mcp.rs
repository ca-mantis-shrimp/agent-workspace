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
    /// Return the full audit record (fingerprints, selectors, coverage) instead
    /// of the default compact receipt `{id, freshness, supports:[{path,reused}]}`.
    #[serde(default)]
    pub full: bool,
}

/// Input schema for `workspace_amend_claim`, exposing the CLI `amend-claim`
/// verb: revise an existing active belief in place.
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct AmendClaimParams {
    /// Id of the active claim to revise. Its id and lifecycle are preserved.
    pub claim_id: u64,
    /// The revised belief, thesis-first: the assertion as it stands now.
    pub statement: String,
    /// Cited supporting paths for the revision (required, non-empty), each
    /// repository-relative. Freshness is re-anchored to these as they are now.
    pub rests_on: Vec<String>,
    /// Claim scope: `declared` (default) binds only the cited paths;
    /// `conservative-siblings` also fingerprints their repository siblings.
    pub scope: Option<String>,
    /// Return the full audit record instead of the default compact receipt
    /// `{id, freshness, supports:[{path, reused}]}`.
    #[serde(default)]
    pub full: bool,
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
    /// Return the full superseded claim instead of the default compact receipt
    /// `{id, lifecycle}` (the disposition already names the replacement).
    #[serde(default)]
    pub full: bool,
}

/// Input schema for `workspace_retire_claim`, exposing the CLI `retire-claim`
/// verb: retirement without a replacement.
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct RetireClaimParams {
    /// Id of the active claim to retire.
    pub claim_id: u64,
    /// Why the belief is no longer maintained (a mistaken record, or one whose
    /// subject work is done). Empty reasons are rejected.
    pub reason: String,
    /// Return the full retired claim instead of the default compact receipt
    /// `{id, lifecycle}`.
    #[serde(default)]
    pub full: bool,
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

/// Input schema shared by projections that offer a bounded default and a full
/// audit expansion.
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct FullParams {
    /// Return the complete audit projection instead of the bounded default.
    #[serde(default)]
    pub full: bool,
}

/// Input schema for `workspace_delta`.
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DeltaParams {
    /// Return complete changed entities instead of the bounded id summary.
    #[serde(default)]
    pub full: bool,
    /// Diff against this checkpoint label instead of the latest checkpoint.
    pub since: Option<String>,
}

/// Input schema for `workspace_transaction_preview`.
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct TransactionPreviewParams {
    /// Id of the transaction to preview.
    pub transaction: u64,
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
        description = "Orient in the persistent agent workspace: objective, a kernel-bounded stale-first claim window with explicit omission count, aggregate freshness, open transactions, and latest checkpoint. `full` returns the complete audit record. A stale claim outranks your remembered belief."
    )]
    fn workspace_status(
        &self,
        Parameters(params): Parameters<FullParams>,
    ) -> Result<CallToolResult, McpError> {
        self.tool_result(self.status(params.full))
    }

    #[tool(
        description = "Kernel-bounded changes since a checkpoint: objective shift plus total/recent ids for claims, observations, and transactions. Use after workspace_status when resuming; `full` returns complete changed entities and `since` selects a checkpoint label."
    )]
    fn workspace_delta(
        &self,
        Parameters(params): Parameters<DeltaParams>,
    ) -> Result<CallToolResult, McpError> {
        self.tool_result(self.delta(params.full, params.since))
    }

    #[tool(
        description = "The bounded attention model: ranked semantic locations you have focused (path, selector, revision, freshness, why), current observations not yet cited by any claim, and the ordered navigation trail. Every section is kernel-bounded with an explicit omission count."
    )]
    fn workspace_working_set(&self) -> Result<CallToolResult, McpError> {
        self.tool_result(self.working_set())
    }

    #[tool(
        description = "The persistent quickfix-like queue: open provider-reported findings ranked most-severe first, kernel-bounded with an explicit omission count, plus freshness and disposition counts."
    )]
    fn workspace_findings(&self) -> Result<CallToolResult, McpError> {
        self.tool_result(self.findings())
    }

    #[tool(
        description = "Review a change transaction before accepting it: intent, affected locations, associated findings, evidence, acceptance claims, residual risks, and current readiness. A missing transaction id is a strict tool error."
    )]
    fn workspace_transaction_preview(
        &self,
        Parameters(params): Parameters<TransactionPreviewParams>,
    ) -> Result<CallToolResult, McpError> {
        self.tool_result(self.transaction_preview(params.transaction))
    }

    #[tool(
        description = "Record a belief — the fused write verb: \"I now believe X, and it rests on files Y, Z.\" Replaces the raw-CLI observe-then-claim two-step. For each rests_on path the kernel reuses the freshest current observation (typically your ambient read captures) or else captures the whole file. Citation is mandatory: at least one rests_on path is required — a belief you cannot cite cannot be recorded. Rejections are strict and name the failed inputs; re-read the named file, then re-record. Returns a compact receipt {id, freshness, supports:[{path, reused}]} by default; pass full:true for the whole audit record (fingerprints, selectors, coverage). A claim the workspace later reports as stale outranks your remembered belief."
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
        match self.record(params.statement, &rests_on, scope, params.full) {
            Ok(json) => Ok(CallToolResult::success(vec![ContentBlock::text(json)])),
            Err(message) => Ok(CallToolResult::error(vec![ContentBlock::text(message)])),
        }
    }

    #[tool(
        description = "Revise an existing active belief in place: amend_claim(claim_id, statement, rests_on, scope?). Use when an edit has partially overtaken a claim you still hold — revise its statement and re-cite the files as they are now, keeping the claim's id and re-anchoring its freshness, instead of superseding it whole and re-narrating what stayed true. Only active claims can be amended (a superseded/retired belief is re-recorded); the prior revision stays in the append-only log. Citation is mandatory and rejections are strict, exactly like record_belief. Returns a compact receipt {id, freshness, supports:[{path, reused}]} by default; pass full:true for the whole audit record."
    )]
    fn workspace_amend_claim(
        &self,
        Parameters(params): Parameters<AmendClaimParams>,
    ) -> Result<CallToolResult, McpError> {
        let scope = match params.scope.as_deref() {
            Some("conservative-siblings") => ClaimScopeStrategy::ConservativeSiblingFiles,
            _ => ClaimScopeStrategy::Declared,
        };
        let rests_on: Vec<PathBuf> = params.rests_on.iter().map(PathBuf::from).collect();
        match self.amend(
            params.claim_id,
            params.statement,
            &rests_on,
            scope,
            params.full,
        ) {
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
        description = "Retire a claim that no longer holds: supersede_claim(claim_id, replacement_claim_id, reason). Record the revised belief first with workspace_record_belief, then retire the old claim citing the replacement. The reason is mandatory; an already-superseded or missing claim id is rejected strictly. Returns a compact receipt {id, lifecycle} (the disposition names the replacement) by default; pass full:true for the whole superseded claim."
    )]
    fn workspace_supersede_claim(
        &self,
        Parameters(params): Parameters<SupersedeClaimParams>,
    ) -> Result<CallToolResult, McpError> {
        match self.supersede(
            params.claim_id,
            params.replacement_claim_id,
            params.reason,
            params.full,
        ) {
            Ok(json) => Ok(CallToolResult::success(vec![ContentBlock::text(json)])),
            Err(message) => Ok(CallToolResult::error(vec![ContentBlock::text(message)])),
        }
    }

    #[tool(
        description = "Retire a claim WITHOUT a replacement: workspace_retire_claim(claim_id, reason). Use when a belief is no longer maintained — a mistaken record, or one whose subject work is simply done — and no successor belief replaces it (that is supersede_claim's job). The claim leaves every active window and reconciliation but stays auditable in the log. Reason mandatory; a missing or already-inactive claim id is rejected strictly. Returns a compact receipt {id, lifecycle} by default; pass full:true for the whole retired claim."
    )]
    fn workspace_retire_claim(
        &self,
        Parameters(params): Parameters<RetireClaimParams>,
    ) -> Result<CallToolResult, McpError> {
        match self.retire(params.claim_id, params.reason, params.full) {
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
    fn tool_result(&self, result: Result<String, String>) -> Result<CallToolResult, McpError> {
        Ok(match result {
            Ok(json) => CallToolResult::success(vec![ContentBlock::text(json)]),
            Err(message) => CallToolResult::error(vec![ContentBlock::text(message)]),
        })
    }

    /// The in-process `open -> lock -> op` path, mirroring CLI dispatch. Runs
    /// `op` under the per-call exclusive lock and returns the kernel's JSON on
    /// success, or its (strict, input-naming) error text. Every tool below is a
    /// thin closure over this, so an MCP call and a CLI call stay identical.
    fn run<T, F>(&self, op: F) -> Result<String, String>
    where
        F: FnOnce(&Workspace) -> Result<T, agent_workspace::WorkspaceError>,
        T: serde::Serialize,
    {
        // Fail with proprioception, not a bare os error. When the client starts
        // the server with a `--repository` that never resolved (e.g. an
        // unexpanded `${CLAUDE_PROJECT_DIR}` literal), every kernel call would
        // otherwise die as "No such file or directory" with no hint at the
        // cause. Name the path and the usual culprit before touching the store.
        if !self.repository.is_dir() {
            return Err(format!(
                "workspace repository path '{}' does not exist or is not a \
                 directory. The MCP server was started with this --repository; \
                 if your client substitutes a variable like ${{CLAUDE_PROJECT_DIR}}, \
                 confirm it expanded to a real path — an unexpanded literal is \
                 the usual cause.",
                self.repository.display()
            ));
        }
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

    fn status(&self, full: bool) -> Result<String, String> {
        self.run(move |workspace| {
            if full {
                serde_json::to_value(workspace.resume_status()?).map_err(Into::into)
            } else {
                serde_json::to_value(workspace.resume_brief_status()?).map_err(Into::into)
            }
        })
    }

    fn delta(&self, full: bool, since: Option<String>) -> Result<String, String> {
        self.run(move |workspace| {
            if full {
                serde_json::to_value(workspace.delta_since(since.as_deref())?).map_err(Into::into)
            } else {
                serde_json::to_value(workspace.delta_brief_since(since.as_deref())?)
                    .map_err(Into::into)
            }
        })
    }

    fn working_set(&self) -> Result<String, String> {
        self.run(Workspace::resume_working_set_view)
    }

    fn findings(&self) -> Result<String, String> {
        self.run(Workspace::resume_findings_view)
    }

    fn transaction_preview(&self, transaction_id: u64) -> Result<String, String> {
        self.run(move |workspace| {
            workspace.resume_transaction_preview(transaction_id)?.ok_or(
                agent_workspace::WorkspaceError::TransactionNotFound(transaction_id),
            )
        })
    }

    fn record(
        &self,
        statement: String,
        rests_on: &[PathBuf],
        scope: ClaimScopeStrategy,
        full: bool,
    ) -> Result<String, String> {
        let rests_on: Vec<PathBuf> = rests_on.to_vec();
        self.run(move |workspace| {
            let belief = workspace.record_belief(statement, &rests_on, scope)?;
            if full {
                serde_json::to_value(&belief).map_err(Into::into)
            } else {
                serde_json::to_value(belief.brief()).map_err(Into::into)
            }
        })
    }

    fn amend(
        &self,
        claim_id: u64,
        statement: String,
        rests_on: &[PathBuf],
        scope: ClaimScopeStrategy,
        full: bool,
    ) -> Result<String, String> {
        let rests_on: Vec<PathBuf> = rests_on.to_vec();
        self.run(move |workspace| {
            let belief = workspace.amend_claim(claim_id, statement, &rests_on, scope)?;
            if full {
                serde_json::to_value(&belief).map_err(Into::into)
            } else {
                serde_json::to_value(belief.brief()).map_err(Into::into)
            }
        })
    }

    fn bind(&self, intent: String, external_reference: Option<String>) -> Result<String, String> {
        self.run(move |workspace| workspace.bind_objective(intent, external_reference))
    }

    fn supersede(
        &self,
        claim_id: u64,
        replacement_claim_id: u64,
        reason: String,
        full: bool,
    ) -> Result<String, String> {
        self.run(move |workspace| {
            let claim = workspace.supersede_claim(claim_id, replacement_claim_id, reason)?;
            if full {
                serde_json::to_value(&claim).map_err(Into::into)
            } else {
                serde_json::to_value(claim.brief()).map_err(Into::into)
            }
        })
    }

    fn retire(&self, claim_id: u64, reason: String, full: bool) -> Result<String, String> {
        self.run(move |workspace| {
            let claim = workspace.retire_claim(claim_id, reason)?;
            if full {
                serde_json::to_value(&claim).map_err(Into::into)
            } else {
                serde_json::to_value(claim.brief()).map_err(Into::into)
            }
        })
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
            "Agent Workspace: orient with workspace_status then workspace_delta; inspect \
             attention, findings, and transaction readiness with workspace_working_set, \
             workspace_findings, and workspace_transaction_preview. Bind the objective, \
             record cited beliefs, retire revised claims, checkpoint coherent slices, and \
             capture native reads through the corresponding workspace_* tools. A claim \
             reported as stale outranks your remembered belief."
                .to_owned(),
        );
        info
    }
}

/// Serve the workspace over stdio. Builds a contained current-thread runtime so
/// the rest of the binary stays synchronous; blocks until the client hangs up.
pub fn serve(repository: PathBuf) -> Result<(), std::io::Error> {
    // Surface a misconfigured repository in the client's MCP server log at
    // startup, not only per-call. Non-fatal: the server still serves so tools
    // list and each call returns the same named error to the agent.
    if !repository.is_dir() {
        eprintln!(
            "agent-workspace mcp: warning — repository path '{}' does not exist \
             or is not a directory; every workspace call will fail until it is \
             corrected (an unexpanded --repository variable is the usual cause).",
            repository.display()
        );
    }
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
