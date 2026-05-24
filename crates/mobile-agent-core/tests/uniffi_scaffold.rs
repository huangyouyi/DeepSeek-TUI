use std::{collections::BTreeSet, fs, path::Path};

const CRATE_ROOT: &str = env!("CARGO_MANIFEST_DIR");

#[test]
fn uniffi_udl_file_defines_mobile_agent_boundary() {
    let udl = fs::read_to_string(Path::new(CRATE_ROOT).join("src/uniffi_api.udl"))
        .expect("src/uniffi_api.udl should exist");

    assert!(udl.contains("namespace deepseek_mobile_agent_core"));
    assert!(udl.contains("dictionary IosConnectionProfile"));
    assert!(udl.contains("dictionary IosBootstrapStep"));
    assert!(udl.contains("dictionary IosMobileEvent"));
    assert!(udl.contains("interface IosMobileAgentCore"));
    assert!(udl.contains("[Throws=IosMobileCoreError]"));
}

#[test]
fn uniffi_udl_methods_match_rust_json_facade() {
    let rust = fs::read_to_string(Path::new(CRATE_ROOT).join("src/uniffi_api.rs"))
        .expect("src/uniffi_api.rs should exist");
    let udl = fs::read_to_string(Path::new(CRATE_ROOT).join("src/uniffi_api.udl"))
        .expect("src/uniffi_api.udl should exist");

    let rust_methods = rust_facade_methods(&rust);
    let udl_methods = udl_interface_methods(&udl);

    assert_eq!(udl_methods, rust_methods);
}

#[test]
fn cargo_manifest_declares_uniffi_static_library_shape() {
    let manifest = fs::read_to_string(Path::new(CRATE_ROOT).join("Cargo.toml"))
        .expect("Cargo.toml should exist");

    assert!(manifest.contains("crate-type = [\"lib\", \"staticlib\", \"cdylib\"]"));
    let dependencies = manifest_section(&manifest, "dependencies");
    let build_dependencies = manifest_section(&manifest, "build-dependencies");
    assert!(dependencies.contains("uniffi = "));
    assert!(build_dependencies.contains("uniffi = "));
    assert!(build_dependencies.contains("features = [\"build\"]"));
}

#[test]
fn build_script_has_real_uniffi_generation_entrypoint() {
    let build_rs =
        fs::read_to_string(Path::new(CRATE_ROOT).join("build.rs")).expect("build.rs should exist");

    assert!(build_rs.contains("uniffi::generate_scaffolding(\"src/uniffi_api.udl\")"));
    assert!(build_rs.contains("DEEPSEEK_UNIFFI_GENERATE_SCAFFOLDING"));
    assert!(build_rs.contains("cargo:rerun-if-env-changed=DEEPSEEK_UNIFFI_GENERATE_SCAFFOLDING"));
}

#[test]
fn linux_dry_run_script_documents_generation_command_plan() {
    let script = fs::read_to_string(Path::new(CRATE_ROOT).join("scripts/uniffi-dry-run"))
        .expect("scripts/uniffi-dry-run should exist");

    assert!(script.contains("uniffi-bindgen generate"));
    assert!(script.contains("src/uniffi_api.udl"));
    assert!(script.contains("--language swift"));
    assert!(script.contains("cargo build -p deepseek-mobile-agent-core --release"));
    assert!(script.contains("libdeepseek_mobile_agent_core.a"));
    assert!(script.contains("check-plan"));
}

fn rust_facade_methods(source: &str) -> BTreeSet<String> {
    source
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim_start();
            let signature = trimmed.strip_prefix("pub fn ")?;
            signature.split_once('(').map(|(name, _)| name.to_string())
        })
        .collect()
}

fn manifest_section(source: &str, section: &str) -> String {
    let header = format!("[{section}]");
    let mut in_section = false;
    let mut body = String::new();

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            if in_section {
                break;
            }
            in_section = trimmed == header;
            continue;
        }
        if in_section {
            body.push_str(line);
            body.push('\n');
        }
    }

    body
}

fn udl_interface_methods(source: &str) -> BTreeSet<String> {
    let mut in_interface = false;
    let mut methods = BTreeSet::new();

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed == "interface IosMobileAgentCore {" {
            in_interface = true;
            continue;
        }
        if in_interface && trimmed == "};" {
            break;
        }
        if !in_interface || trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }

        let declaration = trimmed
            .strip_prefix("[Throws=IosMobileCoreError] ")
            .unwrap_or(trimmed);
        if declaration == "constructor();" {
            methods.insert("new".to_string());
            continue;
        }

        let Some(prefix) = declaration.split_once('(').map(|(prefix, _)| prefix) else {
            continue;
        };
        let Some(name) = prefix.split_whitespace().last() else {
            continue;
        };
        methods.insert(name.to_string());
    }

    methods
}
