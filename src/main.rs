use agent_workspace::{
    ClaimScopeStrategy, EvidenceOutcome, Normalizer, ObservationCaptureOptions,
    ObservationSelector, ReadCaptureOutcome, ReadCaptureRequest, Workspace, WorkspaceError,
};
use serde::Serialize;
use std::env;
use std::io::Read;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    match run(env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run(arguments: Vec<String>) -> Result<(), CliError> {
    let Some((command, rest)) = arguments.split_first() else {
        return Err(CliError::Usage(usage()));
    };
    let options = Options::parse(rest)?;
    let workspace = Workspace::open(&options.repository, &options.workspace)?;

    // Serialize every invocation against this workspace. Held until `run`
    // returns, this exclusive lock wraps each command's full read-modify-write,
    // so two concurrent processes can never interleave appends into a corrupt
    // log or collide on an entity id.
    let _lock = workspace.lock_exclusive()?;

    match command.as_str() {
        "bind-objective" => {
            print_json(&workspace.bind_objective(
                options.intent.ok_or_else(|| {
                    CliError::Usage("bind-objective requires --intent".to_owned())
                })?,
                options.external_reference,
            )?)?;
        }
        "focus" => {
            let observation_id = options
                .observation_ids
                .first()
                .copied()
                .ok_or_else(|| CliError::Usage("focus requires --observation".to_owned()))?;
            print_json(
                &workspace.focus_observation(
                    observation_id,
                    options
                        .reason
                        .ok_or_else(|| CliError::Usage("focus requires --reason".to_owned()))?,
                )?,
            )?;
        }
        "status" => {
            let status = workspace.resume_status()?;
            if options.full {
                print_json(&status)?;
            } else {
                print_json(&status.brief())?;
            }
        }
        "checkpoint" => {
            let label = options
                .label
                .ok_or_else(|| CliError::Usage("checkpoint requires --label".to_owned()))?;
            print_json(&workspace.checkpoint(label, options.note)?)?;
        }
        "delta" => print_json(&workspace.delta_since(options.since.as_deref())?)?,
        "observe" => {
            let path = options
                .path
                .ok_or_else(|| CliError::Usage("observe requires --path".to_owned()))?;
            let provider = options.provider.unwrap_or_else(|| "filesystem".to_owned());
            // `auto` (the default) resolves to a concrete normalizer here, at
            // capture time; the record persists the resolved scheme.
            let normalizer = options
                .normalizer
                .unwrap_or_else(|| Normalizer::detect_for_path(&path));
            print_json(&workspace.capture_file_observation(
                path,
                provider,
                ObservationCaptureOptions {
                    selector: options.selector.unwrap_or_default(),
                    normalizer,
                    retain_native_payload: options.retain_payload,
                    model_visible_bytes: options.model_visible_bytes,
                    expected_raw_fingerprint: options.expected_raw_fingerprint,
                },
            )?)?;
        }
        "observe-read" => {
            let path = options
                .path
                .ok_or_else(|| CliError::Usage("observe-read requires --path".to_owned()))?;
            let provider = options.provider.unwrap_or_else(|| "read".to_owned());
            // The model-visible text arrives on stdin: it is arbitrary multi-line
            // content and belongs nowhere on a command line.
            let mut model_visible_text = String::new();
            std::io::stdin().read_to_string(&mut model_visible_text)?;
            let outcome = workspace.capture_read_observation(
                path,
                provider,
                ReadCaptureRequest {
                    offset: options.offset,
                    limit: options.limit,
                    model_visible_text,
                    truncated: options.truncated,
                },
            )?;
            match outcome {
                ReadCaptureOutcome::Captured(capture) => {
                    let mut value = serde_json::to_value(&capture)?;
                    if let Some(object) = value.as_object_mut() {
                        object.insert("outcome".to_owned(), serde_json::json!("captured"));
                    }
                    print_json(&value)?;
                }
                ReadCaptureOutcome::Skipped { reason } => {
                    print_json(&serde_json::json!({ "outcome": "skipped", "reason": reason }))?;
                }
            }
        }
        "reveal" => {
            let observation_id = options
                .observation_ids
                .first()
                .copied()
                .ok_or_else(|| CliError::Usage("reveal requires --observation".to_owned()))?;
            print_json(&workspace.reveal_observation(observation_id)?)?;
        }
        "reconcile" => {
            let id = options
                .id
                .ok_or_else(|| CliError::Usage("reconcile requires --id".to_owned()))?;
            print_json(&workspace.reconcile_observation(id)?)?;
        }
        "claim" => {
            let statement = options
                .statement
                .ok_or_else(|| CliError::Usage("claim requires --statement".to_owned()))?;
            print_json(&workspace.record_claim_with_scope(
                statement,
                &options.observation_ids,
                &options.dependencies,
                options.scope_strategy,
            )?)?;
        }
        "reconcile-claim" => {
            let id = options
                .id
                .ok_or_else(|| CliError::Usage("reconcile-claim requires --id".to_owned()))?;
            print_json(&workspace.reconcile_claim(id)?)?;
        }
        "supersede-claim" => {
            let id = options
                .id
                .ok_or_else(|| CliError::Usage("supersede-claim requires --id".to_owned()))?;
            let replacement_claim_id = options
                .claim_id
                .ok_or_else(|| CliError::Usage("supersede-claim requires --claim".to_owned()))?;
            let reason = options
                .reason
                .ok_or_else(|| CliError::Usage("supersede-claim requires --reason".to_owned()))?;
            print_json(&workspace.supersede_claim(id, replacement_claim_id, reason)?)?;
        }
        "begin-transaction" => {
            print_json(&workspace.begin_transaction(&options.claim_ids)?)?;
        }
        "evidence" => {
            let transaction_id = options
                .transaction_id
                .ok_or_else(|| CliError::Usage("evidence requires --transaction".to_owned()))?;
            let claim_id = options
                .claim_id
                .ok_or_else(|| CliError::Usage("evidence requires --claim".to_owned()))?;
            print_json(
                &workspace.record_evidence(
                    transaction_id,
                    claim_id,
                    options
                        .check_name
                        .ok_or_else(|| CliError::Usage("evidence requires --check".to_owned()))?,
                    options.invocation.ok_or_else(|| {
                        CliError::Usage("evidence requires --invocation".to_owned())
                    })?,
                    options.provider.unwrap_or_else(|| "imported".to_owned()),
                    options
                        .outcome
                        .ok_or_else(|| CliError::Usage("evidence requires --result".to_owned()))?,
                )?,
            )?;
        }
        "apply" => {
            let id = options
                .id
                .ok_or_else(|| CliError::Usage("apply requires --id".to_owned()))?;
            let path = options
                .path
                .ok_or_else(|| CliError::Usage("apply requires --path".to_owned()))?;
            let contents = options
                .contents
                .ok_or_else(|| CliError::Usage("apply requires --content".to_owned()))?;
            print_json(&workspace.apply_file_mutation(id, path, contents.as_bytes())?)?;
        }
        "revert-transaction" => {
            let id = options
                .id
                .ok_or_else(|| CliError::Usage("revert-transaction requires --id".to_owned()))?;
            print_json(&workspace.revert_transaction(id)?)?;
        }
        "accept-transaction" => {
            let id = options
                .id
                .ok_or_else(|| CliError::Usage("accept-transaction requires --id".to_owned()))?;
            print_json(&workspace.accept_transaction(id)?)?;
        }
        _ => return Err(CliError::Usage(usage())),
    }
    Ok(())
}

fn print_json(value: &impl Serialize) -> Result<(), CliError> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

struct Options {
    repository: PathBuf,
    workspace: PathBuf,
    path: Option<PathBuf>,
    provider: Option<String>,
    id: Option<u64>,
    statement: Option<String>,
    observation_ids: Vec<u64>,
    dependencies: Vec<PathBuf>,
    scope_strategy: ClaimScopeStrategy,
    claim_ids: Vec<u64>,
    claim_id: Option<u64>,
    transaction_id: Option<u64>,
    check_name: Option<String>,
    invocation: Option<String>,
    outcome: Option<EvidenceOutcome>,
    intent: Option<String>,
    external_reference: Option<String>,
    reason: Option<String>,
    contents: Option<String>,
    selector: Option<ObservationSelector>,
    /// `None` is the `auto` default: resolve per path at dispatch.
    normalizer: Option<Normalizer>,
    retain_payload: bool,
    model_visible_bytes: Option<usize>,
    expected_raw_fingerprint: Option<String>,
    label: Option<String>,
    note: Option<String>,
    since: Option<String>,
    full: bool,
    offset: Option<usize>,
    limit: Option<usize>,
    truncated: bool,
}

impl Options {
    fn parse(arguments: &[String]) -> Result<Self, CliError> {
        let mut repository = None;
        let mut workspace = None;
        let mut path = None;
        let mut provider = None;
        let mut id = None;
        let mut statement = None;
        let mut observation_ids = Vec::new();
        let mut dependencies = Vec::new();
        let mut scope_strategy = ClaimScopeStrategy::Declared;
        let mut claim_ids = Vec::new();
        let mut claim_id = None;
        let mut transaction_id = None;
        let mut check_name = None;
        let mut invocation = None;
        let mut outcome = None;
        let mut intent = None;
        let mut external_reference = None;
        let mut reason = None;
        let mut contents = None;
        let mut selector = None;
        let mut normalizer = None;
        let mut retain_payload = false;
        let mut model_visible_bytes = None;
        let mut expected_raw_fingerprint = None;
        let mut label = None;
        let mut note = None;
        let mut since = None;
        let mut full = false;
        let mut offset = None;
        let mut limit = None;
        let mut truncated = false;
        let mut index = 0;

        while index < arguments.len() {
            let flag = &arguments[index];
            // Valueless flags consume no argument; handle them before the
            // value fetch so a trailing `--full` is not read as "missing value".
            if flag == "--full" {
                full = true;
                index += 1;
                continue;
            }
            if flag == "--truncated" {
                truncated = true;
                index += 1;
                continue;
            }
            let value = arguments
                .get(index + 1)
                .ok_or_else(|| CliError::Usage(format!("missing value for {flag}")))?;
            match flag.as_str() {
                "--repository" => repository = Some(PathBuf::from(value)),
                "--workspace" => workspace = Some(PathBuf::from(value)),
                "--path" => path = Some(PathBuf::from(value)),
                "--provider" => provider = Some(value.clone()),
                "--statement" => statement = Some(value.clone()),
                "--observation" => observation_ids.push(
                    value
                        .parse()
                        .map_err(|_| CliError::Usage(format!("invalid observation id: {value}")))?,
                ),
                "--dependency" => dependencies.push(PathBuf::from(value)),
                "--claim" => {
                    let parsed = value
                        .parse()
                        .map_err(|_| CliError::Usage(format!("invalid claim id: {value}")))?;
                    claim_ids.push(parsed);
                    claim_id = Some(parsed);
                }
                "--transaction" => {
                    transaction_id =
                        Some(value.parse().map_err(|_| {
                            CliError::Usage(format!("invalid transaction id: {value}"))
                        })?)
                }
                "--check" => check_name = Some(value.clone()),
                "--invocation" => invocation = Some(value.clone()),
                "--intent" => intent = Some(value.clone()),
                "--reference" => external_reference = Some(value.clone()),
                "--reason" => reason = Some(value.clone()),
                "--label" => label = Some(value.clone()),
                "--note" => note = Some(value.clone()),
                "--since" => since = Some(value.clone()),
                "--content" => contents = Some(value.clone()),
                "--range" => selector = Some(parse_byte_range(value)?),
                "--normalize" => {
                    normalizer = match value.as_str() {
                        "auto" => None,
                        "none" => Some(Normalizer::None),
                        "rustfmt" => Some(Normalizer::Rustfmt),
                        _ => {
                            return Err(CliError::Usage(format!("invalid normalizer: {value}")));
                        }
                    }
                }
                "--retain-payload" => {
                    retain_payload = value.parse().map_err(|_| {
                        CliError::Usage(format!("invalid retain-payload value: {value}"))
                    })?
                }
                "--model-visible-bytes" => {
                    model_visible_bytes = Some(value.parse().map_err(|_| {
                        CliError::Usage(format!("invalid model-visible byte count: {value}"))
                    })?)
                }
                "--expected-raw-fingerprint" => expected_raw_fingerprint = Some(value.clone()),
                "--offset" => {
                    offset =
                        Some(value.parse().map_err(|_| {
                            CliError::Usage(format!("invalid read offset: {value}"))
                        })?)
                }
                "--limit" => {
                    limit = Some(
                        value
                            .parse()
                            .map_err(|_| CliError::Usage(format!("invalid read limit: {value}")))?,
                    )
                }
                "--result" => {
                    outcome = Some(match value.as_str() {
                        "passed" => EvidenceOutcome::Passed,
                        "failed" => EvidenceOutcome::Failed,
                        _ => {
                            return Err(CliError::Usage(format!(
                                "invalid evidence result: {value}"
                            )));
                        }
                    })
                }
                "--scope" => {
                    scope_strategy = match value.as_str() {
                        "declared" => ClaimScopeStrategy::Declared,
                        "conservative-siblings" => ClaimScopeStrategy::ConservativeSiblingFiles,
                        _ => {
                            return Err(CliError::Usage(format!(
                                "invalid claim scope strategy: {value}"
                            )));
                        }
                    }
                }
                "--id" => {
                    id = Some(
                        value
                            .parse()
                            .map_err(|_| CliError::Usage(format!("invalid id: {value}")))?,
                    )
                }
                _ => return Err(CliError::Usage(format!("unknown option: {flag}"))),
            }
            index += 2;
        }

        Ok(Self {
            repository: repository
                .ok_or_else(|| CliError::Usage("missing --repository".to_owned()))?,
            workspace: workspace
                .ok_or_else(|| CliError::Usage("missing --workspace".to_owned()))?,
            path,
            provider,
            id,
            statement,
            observation_ids,
            dependencies,
            scope_strategy,
            claim_ids,
            claim_id,
            transaction_id,
            check_name,
            invocation,
            outcome,
            intent,
            external_reference,
            reason,
            contents,
            selector,
            normalizer,
            retain_payload,
            model_visible_bytes,
            expected_raw_fingerprint,
            label,
            note,
            since,
            full,
            offset,
            limit,
            truncated,
        })
    }
}

#[derive(Debug)]
enum CliError {
    Usage(String),
    Workspace(WorkspaceError),
    Json(serde_json::Error),
    Io(std::io::Error),
}

impl std::fmt::Display for CliError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Usage(message) => write!(formatter, "{message}"),
            Self::Workspace(error) => error.fmt(formatter),
            Self::Json(error) => error.fmt(formatter),
            Self::Io(error) => error.fmt(formatter),
        }
    }
}

impl From<std::io::Error> for CliError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<WorkspaceError> for CliError {
    fn from(error: WorkspaceError) -> Self {
        Self::Workspace(error)
    }
}

impl From<serde_json::Error> for CliError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

fn parse_byte_range(value: &str) -> Result<ObservationSelector, CliError> {
    let (start, end) = value
        .split_once(':')
        .ok_or_else(|| CliError::Usage(format!("invalid byte range: {value}")))?;
    let start = start
        .parse()
        .map_err(|_| CliError::Usage(format!("invalid byte range start: {start}")))?;
    let end = end
        .parse()
        .map_err(|_| CliError::Usage(format!("invalid byte range end: {end}")))?;
    Ok(ObservationSelector::ByteRange { start, end })
}

fn usage() -> String {
    "usage: agent-workspace <command> --repository PATH --workspace PATH [options]".to_owned()
}
