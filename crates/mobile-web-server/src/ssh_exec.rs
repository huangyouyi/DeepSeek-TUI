use std::process::Command;
use std::sync::Arc;
use std::time::Instant;

use deepseek_mobile_agent_core::ssh::{SshCommandBuilder, SshCommandRequest, SshConnectionConfig};

use crate::SshTarget;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SshCommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub duration: std::time::Duration,
    pub timed_out: bool,
}

pub trait CommandRunner: Send + Sync + 'static {
    fn run(
        &self,
        target: &SshTarget,
        command: &SshCommandRequest,
    ) -> Result<SshCommandOutput, CommandRunError>;
}

impl<T> CommandRunner for Arc<T>
where
    T: CommandRunner + ?Sized,
{
    fn run(
        &self,
        target: &SshTarget,
        command: &SshCommandRequest,
    ) -> Result<SshCommandOutput, CommandRunError> {
        self.as_ref().run(target, command)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SystemSshCommandRunner;

impl CommandRunner for SystemSshCommandRunner {
    fn run(
        &self,
        target: &SshTarget,
        command: &SshCommandRequest,
    ) -> Result<SshCommandOutput, CommandRunError> {
        let config = SshConnectionConfig {
            host: target.host.clone(),
            user: target.user.clone(),
            port: target.port,
            token_present: false,
            key_present: target.key_present,
        };
        let mut invocation = SshCommandBuilder::build_invocation(&config, command);
        invocation.args.splice(
            0..0,
            [
                "-o".to_string(),
                "BatchMode=yes".to_string(),
                "-o".to_string(),
                "ConnectTimeout=5".to_string(),
            ],
        );
        let started = Instant::now();
        let output = Command::new(&invocation.program)
            .args(&invocation.args)
            .output()
            .map_err(CommandRunError::Spawn)?;

        Ok(SshCommandOutput {
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            exit_code: output.status.code(),
            duration: started.elapsed(),
            timed_out: false,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CommandRunError {
    #[error("failed to spawn ssh: {0}")]
    Spawn(std::io::Error),
}
