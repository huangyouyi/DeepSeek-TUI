use std::{
    fmt, fs,
    path::{Path, PathBuf},
    str::FromStr,
};

use serde::{Deserialize, Serialize, ser::SerializeStruct};
use thiserror::Error;

const DEFAULT_PROVIDER: &str = "deepseek";
const DEFAULT_MODEL: &str = "deepseek-v4-flash";
const DEFAULT_BASE_URL: &str = "https://api.deepseek.com";

#[derive(Clone, PartialEq, Eq)]
pub struct MobileModelConfig {
    pub provider: String,
    pub base_url: String,
    pub model: String,
    pub api_key: Option<String>,
    pub source: MobileModelConfigSource,
}

impl fmt::Debug for MobileModelConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MobileModelConfig")
            .field("provider", &self.provider)
            .field("base_url", &self.base_url)
            .field("model", &self.model)
            .field("api_key_present", &self.api_key_present())
            .field("source", &self.source)
            .finish()
    }
}

impl MobileModelConfig {
    pub fn load_default(mode: MobileModelMode) -> Result<Self, MobileModelConfigError> {
        let path = default_config_path()?;
        Self::load_from_default_path(path, mode)
    }

    pub fn load_from_path(
        path: impl AsRef<Path>,
        mode: MobileModelMode,
    ) -> Result<Self, MobileModelConfigError> {
        Self::load(path.as_ref(), mode, ConfigPathKind::Explicit)
    }

    pub fn redacted_status(&self) -> MobileModelConfigStatus {
        MobileModelConfigStatus {
            provider: self.provider.clone(),
            base_url: self.base_url.clone(),
            model: self.model.clone(),
            api_key_present: self.api_key_present(),
            source: self.source.clone(),
        }
    }

    fn load_from_default_path(
        path: PathBuf,
        mode: MobileModelMode,
    ) -> Result<Self, MobileModelConfigError> {
        Self::load(&path, mode, ConfigPathKind::Default)
    }

    fn load(
        path: &Path,
        mode: MobileModelMode,
        path_kind: ConfigPathKind,
    ) -> Result<Self, MobileModelConfigError> {
        let source = match path_kind {
            ConfigPathKind::Explicit => MobileModelConfigSource::ExplicitPath(path.to_path_buf()),
            ConfigPathKind::Default => MobileModelConfigSource::DefaultPath(path.to_path_buf()),
        };
        let raw = match fs::read_to_string(path) {
            Ok(contents) => toml::from_str::<RawMobileConfig>(&contents).map_err(|source| {
                MobileModelConfigError::Parse {
                    path: path.to_path_buf(),
                    source,
                }
            })?,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => match path_kind {
                ConfigPathKind::Default => {
                    return resolved_from_raw(
                        RawMobileConfig::default(),
                        mode,
                        MobileModelConfigSource::MissingDefaultPath(path.to_path_buf()),
                    );
                }
                ConfigPathKind::Explicit => {
                    return Err(MobileModelConfigError::Read {
                        path: path.to_path_buf(),
                        source,
                    });
                }
            },
            Err(source) => {
                return Err(MobileModelConfigError::Read {
                    path: path.to_path_buf(),
                    source,
                });
            }
        };

        resolved_from_raw(raw, mode, source)
    }

    fn api_key_present(&self) -> bool {
        self.api_key
            .as_deref()
            .is_some_and(|key| !key.trim().is_empty())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MobileModelConfigSource {
    ExplicitPath(PathBuf),
    DefaultPath(PathBuf),
    MissingDefaultPath(PathBuf),
}

impl Serialize for MobileModelConfigSource {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let (kind, path) = match self {
            Self::ExplicitPath(path) => ("explicit_path", path),
            Self::DefaultPath(path) => ("default_path", path),
            Self::MissingDefaultPath(path) => ("missing_default_path", path),
        };
        let mut state = serializer.serialize_struct("MobileModelConfigSource", 2)?;
        state.serialize_field("kind", kind)?;
        state.serialize_field("path", path)?;
        state.end()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct MobileModelConfigStatus {
    pub provider: String,
    pub base_url: String,
    pub model: String,
    pub api_key_present: bool,
    pub source: MobileModelConfigSource,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MobileModelMode {
    Auto,
    Mock,
    Deepseek,
}

impl FromStr for MobileModelMode {
    type Err = MobileModelConfigError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "mock" => Ok(Self::Mock),
            "deepseek" => Ok(Self::Deepseek),
            other => Err(MobileModelConfigError::InvalidMode {
                mode: other.to_string(),
            }),
        }
    }
}

#[derive(Debug, Error)]
pub enum MobileModelConfigError {
    #[error("unable to resolve home directory for default DeepSeek config path")]
    HomeDirectoryUnavailable,
    #[error("failed to read DeepSeek config at {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse DeepSeek config at {path}: {source}")]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    #[error("unsupported mobile model mode '{mode}'; expected auto, mock, or deepseek")]
    InvalidMode { mode: String },
    #[error("deepseek mobile model mode requires a non-empty api_key in DeepSeek config")]
    MissingDeepseekApiKey,
}

#[derive(Clone, Copy)]
enum ConfigPathKind {
    Explicit,
    Default,
}

#[derive(Default, Deserialize)]
struct RawMobileConfig {
    provider: Option<String>,
    api_key: Option<String>,
    base_url: Option<String>,
    model: Option<String>,
    default_text_model: Option<String>,
    #[serde(default)]
    providers: RawProvidersConfig,
}

#[derive(Default, Deserialize)]
struct RawProvidersConfig {
    #[serde(default)]
    deepseek: RawProviderConfig,
}

#[derive(Default, Deserialize)]
struct RawProviderConfig {
    api_key: Option<String>,
    base_url: Option<String>,
    model: Option<String>,
}

fn resolved_from_raw(
    raw: RawMobileConfig,
    mode: MobileModelMode,
    source: MobileModelConfigSource,
) -> Result<MobileModelConfig, MobileModelConfigError> {
    let provider = non_empty(raw.provider).unwrap_or_else(|| DEFAULT_PROVIDER.to_string());
    let api_key = non_empty(raw.providers.deepseek.api_key).or_else(|| non_empty(raw.api_key));
    let base_url = non_empty(raw.providers.deepseek.base_url)
        .or_else(|| non_empty(raw.base_url))
        .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());
    let model = non_empty(raw.providers.deepseek.model)
        .or_else(|| non_empty(raw.model))
        .or_else(|| non_empty(raw.default_text_model))
        .unwrap_or_else(|| DEFAULT_MODEL.to_string());

    match mode {
        MobileModelMode::Mock => Ok(mock_config(source)),
        MobileModelMode::Auto if api_key.is_none() => Ok(mock_config(source)),
        MobileModelMode::Auto | MobileModelMode::Deepseek => {
            let Some(api_key) = api_key else {
                return Err(MobileModelConfigError::MissingDeepseekApiKey);
            };
            Ok(MobileModelConfig {
                provider,
                base_url,
                model,
                api_key: Some(api_key),
                source,
            })
        }
    }
}

fn mock_config(source: MobileModelConfigSource) -> MobileModelConfig {
    MobileModelConfig {
        provider: "mock".to_string(),
        base_url: DEFAULT_BASE_URL.to_string(),
        model: "mock".to_string(),
        api_key: None,
        source,
    }
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn default_config_path() -> Result<PathBuf, MobileModelConfigError> {
    dirs::home_dir()
        .map(|home| home.join(".deepseek").join("config.toml"))
        .ok_or(MobileModelConfigError::HomeDirectoryUnavailable)
}
