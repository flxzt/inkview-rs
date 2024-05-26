use std::path::{Path, PathBuf};
use std::process::ExitStatus;
use tokio::process::{Child, Command};

use crate::error::DevtoolsError;

pub struct DebugSession {
    port: u16,
    executable: PathBuf,
    gdbserver_process: Child,
}

impl DebugSession {
    pub async fn start(executable: PathBuf, port: u16) -> Result<Self, DevtoolsError> {
        let executable =
            executable
                .canonicalize()
                .map_err(|source| DevtoolsError::DebugSessionIo {
                    msg: "Canonicalizing executable path failed".to_string(),
                    source,
                })?;
        let gdbserver_process = Command::new("gdbserver")
            .args([
                &format!("0.0.0.0:{port}"),
                &executable.display().to_string(),
            ])
            .spawn()
            .map_err(|source| DevtoolsError::DebugSessionIo {
                msg: "Spawning gdbserver failed".to_string(),
                source,
            })?;

        Ok(Self {
            port,
            executable,
            gdbserver_process,
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn executable(&self) -> &Path {
        &self.executable
    }

    pub async fn wait(&mut self) -> Result<ExitStatus, DevtoolsError> {
        self.gdbserver_process
            .wait()
            .await
            .map_err(|source| DevtoolsError::DebugSessionIo {
                msg: "Gdbserver process terminated with error".to_string(),
                source,
            })
    }

    pub async fn terminate(&mut self) -> Result<(), DevtoolsError> {
        self.gdbserver_process
            .kill()
            .await
            .map_err(|source| DevtoolsError::DebugSessionIo {
                msg: "Failed to terminate spawned gdbserver process".to_string(),
                source,
            })
    }
}
