use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use deepseek_mobile_web_server::model_config::{
    MobileModelConfig, MobileModelConfigSource, MobileModelMode,
};
use serde_json::json;

const SECRET_KEY: &str = "sk-test-secret-token-that-must-not-leak";

#[test]
fn model_config_explicit_path_loads_deepseek_config_read_only() {
    let temp = TempConfig::new(
        r#"
provider = "deepseek"
api_key = "sk-test-secret-token-that-must-not-leak"
model = "deepseek-v4-pro"
base_url = "https://gateway.example/v1"
"#,
    );

    let before = fs::read_to_string(temp.path()).expect("temp config must be readable");
    let config = MobileModelConfig::load_from_path(temp.path(), MobileModelMode::Auto)
        .expect("config with key should load");
    let after = fs::read_to_string(temp.path()).expect("temp config must still be readable");

    assert_eq!(after, before, "loader must not modify the config file");
    assert_eq!(config.provider, "deepseek");
    assert_eq!(config.api_key.as_deref(), Some(SECRET_KEY));
    assert_eq!(config.model, "deepseek-v4-pro");
    assert_eq!(config.base_url, "https://gateway.example/v1");
    assert_eq!(
        config.source,
        MobileModelConfigSource::ExplicitPath(temp.path().to_path_buf())
    );
}

#[test]
fn model_config_explicit_path_uses_deepseek_fallbacks_for_missing_optional_fields() {
    let temp = TempConfig::new(
        r#"
api_key = "sk-test-secret-token-that-must-not-leak"
"#,
    );

    let config = MobileModelConfig::load_from_path(temp.path(), MobileModelMode::Deepseek)
        .expect("config with key should load");

    assert_eq!(config.provider, "deepseek");
    assert_eq!(config.model, "deepseek-v4-flash");
    assert_eq!(config.base_url, "https://api.deepseek.com");
}

#[test]
fn model_config_auto_mode_selects_mock_without_non_empty_key() {
    let temp = TempConfig::new(
        r#"
provider = "deepseek"
api_key = "   "
model = "deepseek-v4-pro"
"#,
    );

    let config = MobileModelConfig::load_from_path(temp.path(), MobileModelMode::Auto)
        .expect("auto mode without key should fall back to mock");

    assert_eq!(config.provider, "mock");
    assert_eq!(config.api_key, None);
    assert_eq!(config.model, "mock");
}

#[test]
fn model_config_deepseek_mode_without_key_returns_clear_redacted_error() {
    let temp = TempConfig::new(
        r#"
provider = "deepseek"
api_key = "sk-test-secret-token-that-must-not-leak"
"#,
    );
    fs::write(temp.path(), "provider = \"deepseek\"\n").expect("rewrite temp config");

    let error = MobileModelConfig::load_from_path(temp.path(), MobileModelMode::Deepseek)
        .expect_err("deepseek mode requires a key")
        .to_string();

    assert!(error.contains("api_key"));
    assert!(error.contains("deepseek"));
    assert!(!error.contains(SECRET_KEY));
}

#[test]
fn model_config_mode_parser_accepts_supported_values_only() {
    assert_eq!(
        "auto".parse::<MobileModelMode>().unwrap(),
        MobileModelMode::Auto
    );
    assert_eq!(
        "MOCK".parse::<MobileModelMode>().unwrap(),
        MobileModelMode::Mock
    );
    assert_eq!(
        " deepseek ".parse::<MobileModelMode>().unwrap(),
        MobileModelMode::Deepseek
    );
    assert!("real".parse::<MobileModelMode>().is_err());
}

#[test]
fn model_config_redacted_debug_and_status_never_expose_api_key() {
    let temp = TempConfig::new(
        r#"
api_key = "sk-test-secret-token-that-must-not-leak"
"#,
    );
    let config = MobileModelConfig::load_from_path(temp.path(), MobileModelMode::Deepseek)
        .expect("config with key should load");

    let debug = format!("{config:?}");
    assert!(debug.contains("api_key_present"));
    assert!(!debug.contains(SECRET_KEY));

    let status = config.redacted_status();
    assert_eq!(
        serde_json::to_value(&status).expect("status must serialize"),
        json!({
            "provider": "deepseek",
            "base_url": "https://api.deepseek.com",
            "model": "deepseek-v4-flash",
            "api_key_present": true,
            "source": {
                "kind": "explicit_path",
                "path": temp.path()
            }
        })
    );
    assert!(!format!("{status:?}").contains(SECRET_KEY));
    assert!(
        !serde_json::to_string(&status)
            .expect("status must serialize")
            .contains(SECRET_KEY)
    );
}

struct TempConfig {
    dir: PathBuf,
    path: PathBuf,
}

impl TempConfig {
    fn new(contents: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock must be after epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "deepseek-mobile-model-config-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).expect("temp dir must be created");
        let path = dir.join("config.toml");
        fs::write(&path, contents).expect("temp config must be written");
        Self { dir, path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempConfig {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}
