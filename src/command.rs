use anyhow::{anyhow, Result};
use std::process;
use std::process::{Command, Output, ExitStatus};
use tokio::sync::watch::{channel, Receiver};
use std::io::Read;
use std::sync::{Arc, Mutex};

use super::Destination;

struct AsyncCommand {
    stdout: Arc<Mutex<Vec<u8>>>,
    stderr: Arc<Mutex<Vec<u8>>>,
    status: Arc<Mutex<Option<ExitStatus>>>,
    task: Receiver<Result<Output>>,
}

impl AsyncCommand {
    fn from_command(mut cmd: Command) -> Self {
        let stdout = Arc::new(Mutex::new(vec![]));
        let stderr = Arc::new(Mutex::new(vec![]));
        let status = Arc::new(Mutex::new(None));
        let (tx, rx) = channel(Err(anyhow!("No data")));

        let stdout_bg = stdout.clone();
        let stderr_bg = stderr.clone();
        let status_bg = status.clone();

        std::thread::spawn(move || {
            let mut child = cmd.spawn().expect("Failed to spawn command");
            let mut stdout = child.stdout.take().unwrap();
            let mut stderr = child.stderr.take().unwrap();

            loop {
                match child.try_wait() {
                    Ok(Some(status)) => return (),
                    Ok(None) => {
                        *status_bg.lock().expect("Failed to lock status") = child.wait().ok();
                        stdout
                            .read(&mut *stdout_bg.lock().expect("Failed to lock stdout"))
                            .expect("Failed to read stdout");
                        stderr
                            .read(&mut *stdout_bg.lock().expect("Failed to lock stdout"))
                            .expect("Failed to read stdout");
                    }
                    Err(e) => panic!("Failed running child {:?}", e),
                }

                std::thread::sleep(std::time::Duration::from_millis(10))
            }
        });

        Self {
            stdout,
            stderr,
            status,
            task: rx,
        }
    }
}
