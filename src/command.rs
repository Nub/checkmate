use anyhow::{anyhow, Result};
use openssh::{Command as CommandSsh, KnownHosts, Session};
use std::io::Read;
use std::process;
use std::process::{Command, ExitStatus, Output};
use std::sync::{Arc, Mutex};
use tokio::io::AsyncReadExt;
use tokio::runtime::Runtime;
use tokio::sync::watch::{channel, Receiver};

use super::Destination;

#[derive(Debug, Clone)]
struct CommandRunner {
    stdout: Arc<Mutex<Vec<u8>>>,
    stderr: Arc<Mutex<Vec<u8>>>,
    status: Arc<Mutex<Option<ExitStatus>>>,
    complete: Arc<Mutex<Result<bool>>>,
}

impl CommandRunner {
    pub fn from_command(mut cmd: Command) -> Self {
        let stdout = Arc::new(Mutex::new(vec![]));
        let stderr = Arc::new(Mutex::new(vec![]));
        let status = Arc::new(Mutex::new(None));
        let complete = Arc::new(Mutex::new(Ok(false)));

        let stdout_bg = stdout.clone();
        let stderr_bg = stderr.clone();
        let status_bg = status.clone();
        let complete_bg = complete.clone();

        std::thread::spawn(move || {
            let mut child = cmd.spawn().expect("Failed to spawn command");
            let mut stdout = child.stdout.take().unwrap();
            let mut stderr = child.stderr.take().unwrap();

            loop {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        *status_bg.lock().expect("Failed to lock status") = Some(status);
                        *complete_bg.lock().expect("Failed to lock complete") = Ok(true);
                    }
                    Ok(None) => {
                        stdout
                            .read(&mut *stdout_bg.lock().expect("Failed to lock stdout"))
                            .expect("Failed to read stdout");
                        stderr
                            .read(&mut *stderr_bg.lock().expect("Failed to lock stdout"))
                            .expect("Failed to read stderr");
                    }
                    Err(e) => {
                        *complete_bg.lock().expect("Failed to lock complete") =
                            Err(anyhow!("Failed to complete async command"));
                    }
                }

                //Rate limit polling to 10hz
                std::thread::sleep(std::time::Duration::from_millis(100))
            }
        });

        Self {
            stdout,
            stderr,
            status,
            complete,
        }
    }

    pub fn from_command_ssh(cmd: &'static mut CommandSsh) -> Self {
        let stdout = Arc::new(Mutex::new(vec![]));
        let stderr = Arc::new(Mutex::new(vec![]));
        let status = Arc::new(Mutex::new(None));
        let complete = Arc::new(Mutex::new(Ok(false)));

        let stdout_bg = stdout.clone();
        let stderr_bg = stderr.clone();
        let status_bg = status.clone();
        let complete_bg = complete.clone();


        std::thread::spawn(move || {
            let runtime = Runtime::new().expect("Failed to spawn runtime");
            let cmd = cmd;
            let cmd_spawn = cmd.spawn();

            runtime.block_on(async move {
                let mut child = cmd_spawn.await.expect("Failed to spawn child");
                let mut stdout = child.stdout().take().unwrap();
                let mut stderr = child.stderr().take().unwrap();

                let stdout_task = tokio::spawn(async move {
                    let mut buffer = [0; 1024];
                    stdout.read(&mut buffer[..]).await.expect("Failed to read stdout");
                    stdout_bg.lock().expect("Failed to lock stderr").extend_from_slice(&buffer);
                });
                let stderr_task = tokio::spawn(async move {
                    let mut buffer = [0; 1024];
                    stderr.read(&mut buffer[..]).await.expect("Failed to read stderr");
                    stderr_bg.lock().expect("Failed to lock stderr").extend_from_slice(&buffer);
                });

                match child.wait().await {
                    Ok(status) => {
                        *status_bg.lock().expect("Failed to lock status") = Some(status);
                        *complete_bg.lock().expect("Failed to lock complete") = Ok(true);
                        stdout_task.abort();
                        stderr_task.abort();
                    }
                    Err(e) => {
                        *complete_bg.lock().expect("Failed to lock complete") =
                            Err(anyhow!("Failed to complete async command {:?}", e));
                    }
                }
            });
        });

        Self {
            stdout,
            stderr,
            status,
            complete,
        }
    }

    pub fn complete(&self) -> bool {
        match & *self.complete.lock().expect("Failed to lock stdout") {
            Ok(x) => *x,
            Err(e) => false,
        }
    }

    pub fn status(&self) -> Option<ExitStatus> {
        self.status.lock().expect("Failed to lock stdout").clone()
    }

    pub fn stdout(&self) -> Vec<u8> {
        self.stdout.lock().expect("Failed to lock stdout").clone()
    }

    pub fn stderr(&self) -> Vec<u8> {
        self.stderr.lock().expect("Failed to lock stdout").clone()
    }
}
