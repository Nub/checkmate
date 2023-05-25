use anyhow::{anyhow, bail, Result};
use openssh::{KnownHosts, Session, SessionBuilder};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_dhall::StaticType;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use tokio::runtime::Runtime;
use tokio::sync::watch::{channel, Receiver};

mod command;
use command::CommandRunner;

/// Tasks are always ran in parallel
#[derive(Clone, Debug, Serialize, Deserialize, StaticType, JsonSchema)]
pub struct Job {
    pub name: String,
    pub tasks: Vec<Task>,
}

#[derive(Clone, Debug)]
pub struct JobThread {
    pub task: Task,
    pub runners: Vec<CommandRunner>,
}

#[derive(Clone, Debug)]
pub struct JobRunner {
    pub job: Job,
    pub threads: Vec<JobThread>,
}

impl Job {
    pub fn run(self) -> JobRunner {
        JobRunner {
            threads: self
                .tasks
                .iter()
                .map(|t| JobThread {
                    task: t.clone(),
                    runners: t.clone().into_runners(),
                })
                .collect(),
            job: self,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, StaticType, JsonSchema)]
pub enum Task {
    Script(Script),
    Serial(Vec<Script>),
}

#[derive(Debug)]
pub enum TaskResult {
    Script(Result<Output>),
    Serial(Vec<Result<Output>>),
}

impl Task {
    pub fn name(&self) -> String {
        match self {
            Task::Script(s) => s.name.clone(),
            Task::Serial(ss) => ss
                .iter()
                .map(|s| s.name.clone())
                .collect::<Vec<String>>()
                .join(" => "),
        }
    }

    pub fn type_name(&self) -> String {
        match self {
            Self::Script(s) => {
                match &s.destination {
                    Destination::Local => "Local".to_string(),
                    Destination::Remote(r) => format!("Remote: {}", r)
                }
            },
            Self::Serial(_) => "Serial".to_string()
        }
    }

    fn into_runners(self) -> Vec<CommandRunner> {
        match self {
            Task::Script(s) => vec![s.try_into_runner().expect("Failed to make runner")],
            Task::Serial(ss) => ss
                .into_iter()
                .map(|s| s.try_into_runner().expect("Failed to make runner"))
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, StaticType, JsonSchema)]
pub enum Destination {
    /// Run on the machine making the call
    Local,
    /// Run on a remote machine via ssh
    Remote(String),
}

#[derive(Clone, Debug, Serialize, Deserialize, StaticType, JsonSchema)]
pub enum Environment {
    /// Clear out all env variables
    None,
    /// Use the current env variables
    Current,
}

#[derive(Clone, Debug, Serialize, Deserialize, StaticType, JsonSchema)]
pub enum Shell {
    Bash,
    Custom(String),
}

#[derive(Clone, Debug, Serialize, Deserialize, StaticType, JsonSchema)]
pub struct Script {
    pub name: String,
    pub destination: Destination,
    pub environment: Environment,
    pub shell: Shell,
    pub script: String,
}

impl Default for Script {
    fn default() -> Self {
        Self {
            name: "default".into(),
            destination: Destination::Local,
            environment: Environment::None,
            shell: Shell::Bash,
            script: "bash --version".into(),
        }
    }
}

impl Script {
    /// Write out a bash script to /tmp for execution
    fn write_remote_script(&self) -> Result<PathBuf> {
        let script = self.write_script()?;
        let remote = match &self.destination {
            Destination::Remote(remote) => remote,
            _ => bail!("Not actually a remote call"),
        };
        if Command::new("scp")
            .arg("-C")
            .arg(script.clone().into_os_string())
            .arg(format!("{}:/tmp/", remote))
            .stderr(Stdio::null())
            .stdout(Stdio::null())
            .status()?
            .success()
        {
            let file_name = script.file_name().ok_or(anyhow!("No file_name"))?;
            let mut remote_path = PathBuf::new();
            remote_path.push("/tmp");
            remote_path.push(file_name);
            Ok(remote_path)
        } else {
            Err(anyhow!("Failed to upload script to {remote}"))
        }
    }

    /// Write out a bash script to /tmp for execution
    fn write_script(&self) -> Result<PathBuf> {
        let mut path = std::env::temp_dir();
        path.push(format!("checkmate_{}", self.name));
        path.set_extension("sh");

        let mut file = File::create(&path).expect("Failed to write script");

        file.write_all(self.script.as_bytes())?;
        Ok(path)
    }

    fn try_into_runner(self) -> Result<CommandRunner> {
        match &self.destination {
            Destination::Local => {
                let script = self.write_script()?.into_os_string();
                Ok(CommandRunner::from_command(
                    Command::new(self.environment.with_shell(&self.shell)?)
                        .arg(script)
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped()),
                ))
            }
            Destination::Remote(remote) => {
                let script = self
                    .write_remote_script()?
                    .into_os_string()
                    .into_string()
                    .expect("Failed to stringify path");
                let shell = self
                    .environment
                    .with_shell(&self.shell)
                    .expect("Failed to set env");
                let session = SessionBuilder::default();
                Ok(CommandRunner::from_command_ssh(
                    session,
                    remote.clone(),
                    format!("{} {}", shell.clone(), script.clone()),
                ))
            }
        }
    }
}

impl Environment {
    fn with_shell(&self, shell: &Shell) -> Result<String> {
        match self {
            Environment::None => Ok(shell.path()?),
            _ => Ok(shell.path()?),
        }
    }
}

impl Shell {
    fn path(&self) -> Result<String> {
        match self {
            Shell::Bash => Ok("bash".into()),
            Shell::Custom(x) => Ok(x.clone()),
        }
    }
}

impl std::fmt::Display for Task {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Task::Script(s) => write!(f, "{:?}", s.destination),
            _ => write!(f, "Serial"),
        }
    }
}
