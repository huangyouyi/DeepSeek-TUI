use crate::remote_schema::RemoteToolName;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    Bootstrap,
    Ssh,
    PowerShell,
    Runner,
    RemoteMcp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilitySet {
    pub mode: ExecutionMode,
    pub tools: Vec<RemoteToolName>,
}

impl CapabilitySet {
    #[must_use]
    pub fn bootstrap() -> Self {
        Self {
            mode: ExecutionMode::Bootstrap,
            tools: vec![RemoteToolName::BootstrapGuide],
        }
    }

    #[must_use]
    pub fn ssh() -> Self {
        Self {
            mode: ExecutionMode::Ssh,
            tools: vec![
                RemoteToolName::ShellExec,
                RemoteToolName::DiagnoseSystem,
                RemoteToolName::BootstrapGuide,
            ],
        }
    }

    #[must_use]
    pub fn powershell() -> Self {
        Self {
            mode: ExecutionMode::PowerShell,
            tools: vec![
                RemoteToolName::PowerShellExec,
                RemoteToolName::DiagnoseSystem,
                RemoteToolName::BootstrapGuide,
            ],
        }
    }

    #[must_use]
    pub fn runner() -> Self {
        Self {
            mode: ExecutionMode::Runner,
            tools: vec![
                RemoteToolName::ShellExec,
                RemoteToolName::PowerShellExec,
                RemoteToolName::FileRead,
                RemoteToolName::FileWrite,
                RemoteToolName::DiagnoseSystem,
                RemoteToolName::PackageInstall,
                RemoteToolName::BrowserOpen,
                RemoteToolName::BrowserExtractText,
                RemoteToolName::BrowserClick,
                RemoteToolName::BootstrapGuide,
                RemoteToolName::McpCall,
            ],
        }
    }

    #[must_use]
    pub fn remote_mcp() -> Self {
        Self {
            mode: ExecutionMode::RemoteMcp,
            tools: vec![RemoteToolName::McpCall, RemoteToolName::BootstrapGuide],
        }
    }

    #[must_use]
    pub fn from_runner_reported_tools<I, S>(tool_names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut tools = Vec::new();
        for name in tool_names {
            let Ok(tool) = RemoteToolName::parse(name.as_ref()) else {
                continue;
            };
            if !tools.contains(&tool) {
                tools.push(tool);
            }
        }

        Self {
            mode: ExecutionMode::Runner,
            tools,
        }
    }

    #[must_use]
    pub fn allows(&self, tool: RemoteToolName) -> bool {
        self.tools.contains(&tool)
    }
}
