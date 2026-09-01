use agent_workspace::{Workspace, WorkspaceError};
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
}

impl Options {
    fn parse(arguments: &[String]) -> Result<Self, CliError> {
        let mut repository = None;
        let mut workspace = None;
        let mut path = None;
        let mut provider = None;
        let mut id = None;
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
                "--id" => {
                    id =
                        Some(value.parse().map_err(|_| {
                            CliError::Usage(format!("invalid observation id: {value}"))
                        })?)
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
    "usage: agent-workspace <observe|reconcile> --repository PATH --workspace PATH [--path PATH] [--provider ID] [--id NUMBER]".to_owned()
}
