use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteToolName {
    ShellExec,
    PowerShellExec,
    FileRead,
    FileWrite,
    DiagnoseSystem,
    PackageInstall,
    BrowserOpen,
    BrowserExtractText,
    BrowserClick,
    BootstrapGuide,
    McpCall,
}

impl RemoteToolName {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ShellExec => "remote.shell.exec",
            Self::PowerShellExec => "remote.powershell.exec",
            Self::FileRead => "remote.file.read",
            Self::FileWrite => "remote.file.write",
            Self::DiagnoseSystem => "remote.diagnose.system",
            Self::PackageInstall => "remote.package.install",
            Self::BrowserOpen => "remote.browser.open",
            Self::BrowserExtractText => "remote.browser.extract_text",
            Self::BrowserClick => "remote.browser.click",
            Self::BootstrapGuide => "remote.bootstrap.guide",
            Self::McpCall => "remote.mcp.call",
        }
    }

    pub fn parse(name: &str) -> Result<Self, UnknownRemoteToolName> {
        Self::from_str(name)
    }
}

impl fmt::Display for RemoteToolName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownRemoteToolName {
    name: String,
}

impl UnknownRemoteToolName {
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl fmt::Display for UnknownRemoteToolName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown remote tool name: {}", self.name)
    }
}

impl std::error::Error for UnknownRemoteToolName {}

impl FromStr for RemoteToolName {
    type Err = UnknownRemoteToolName;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        match name {
            "remote.shell.exec" => Ok(Self::ShellExec),
            "remote.powershell.exec" => Ok(Self::PowerShellExec),
            "remote.file.read" => Ok(Self::FileRead),
            "remote.file.write" => Ok(Self::FileWrite),
            "remote.diagnose.system" => Ok(Self::DiagnoseSystem),
            "remote.package.install" => Ok(Self::PackageInstall),
            "remote.browser.open" => Ok(Self::BrowserOpen),
            "remote.browser.extract_text" => Ok(Self::BrowserExtractText),
            "remote.browser.click" => Ok(Self::BrowserClick),
            "remote.bootstrap.guide" => Ok(Self::BootstrapGuide),
            "remote.mcp.call" => Ok(Self::McpCall),
            _ => Err(UnknownRemoteToolName {
                name: name.to_string(),
            }),
        }
    }
}

impl Serialize for RemoteToolName {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for RemoteToolName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct RemoteToolNameVisitor;

        impl Visitor<'_> for RemoteToolNameVisitor {
            type Value = RemoteToolName;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a documented remote tool name")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                RemoteToolName::from_str(value).map_err(E::custom)
            }
        }

        deserializer.deserialize_str(RemoteToolNameVisitor)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteToolCall {
    pub call_id: String,
    pub name: RemoteToolName,
    pub arguments: Value,
}

impl RemoteToolCall {
    #[must_use]
    pub fn with_command_lease(mut self, lease: CommandLease) -> Self {
        let idempotency_key = lease.idempotency_key.clone();
        let lease_value =
            serde_json::to_value(lease).expect("command lease must be JSON serializable");

        match &mut self.arguments {
            Value::Object(arguments) => {
                arguments.insert(
                    "idempotency_key".to_string(),
                    Value::String(idempotency_key),
                );
                arguments.insert("lease".to_string(), lease_value);
            }
            arguments => {
                let original_arguments = std::mem::take(arguments);
                *arguments = serde_json::json!({
                    "arguments": original_arguments,
                    "idempotency_key": idempotency_key,
                    "lease": lease_value,
                });
            }
        }

        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandLease {
    pub id: String,
    pub idempotency_key: String,
    pub approved_action: CommandLeaseAction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at_unix_ms: Option<u64>,
}

impl CommandLease {
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        idempotency_key: impl Into<String>,
        approved_action: CommandLeaseAction,
    ) -> Self {
        Self {
            id: id.into(),
            idempotency_key: idempotency_key.into(),
            approved_action,
            expires_at_unix_ms: None,
        }
    }

    #[must_use]
    pub const fn with_expires_at_unix_ms(mut self, expires_at_unix_ms: u64) -> Self {
        self.expires_at_unix_ms = Some(expires_at_unix_ms);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandLeaseAction {
    pub tool: String,
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

impl CommandLeaseAction {
    #[must_use]
    pub fn new(
        tool: impl Into<String>,
        command: impl Into<String>,
        cwd: Option<impl Into<String>>,
    ) -> Self {
        Self {
            tool: tool.into(),
            command: command.into(),
            cwd: cwd.map(Into::into),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteToolOutput {
    pub call_id: String,
    pub success: bool,
    pub result: Value,
}
