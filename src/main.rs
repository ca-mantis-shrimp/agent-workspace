use agent_workspace::{ClaimScopeStrategy, Workspace, WorkspaceError};
use serde::Serialize;
use std::env;
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

    match command.as_str() {
        "observe" => {
            let path = options
                .path
                .ok_or_else(|| CliError::Usage("observe requires --path".to_owned()))?;
            let provider = options.provider.unwrap_or_else(|| "filesystem".to_owned());
            print_json(&workspace.record_file_observation(path, provider)?)?;
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
        let mut index = 0;

        while index < arguments.len() {
            let flag = &arguments[index];
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
        })
    }
}

#[derive(Debug)]
enum CliError {
    Usage(String),
    Workspace(WorkspaceError),
    Json(serde_json::Error),
}

impl std::fmt::Display for CliError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Usage(message) => write!(formatter, "{message}"),
            Self::Workspace(error) => error.fmt(formatter),
            Self::Json(error) => error.fmt(formatter),
        }
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

fn usage() -> String {
    "usage: agent-workspace <observe|reconcile|claim|reconcile-claim> --repository PATH --workspace PATH [options]".to_owned()
}
