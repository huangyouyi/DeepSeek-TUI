use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=src/uniffi_api.udl");
    println!("cargo:rerun-if-changed=src/uniffi_api.rs");
    println!("cargo:rerun-if-env-changed=DEEPSEEK_UNIFFI_GENERATE_SCAFFOLDING");

    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let udl_path = manifest_dir.join("src/uniffi_api.udl");
    let udl = fs::read_to_string(&udl_path).unwrap_or_else(|error| {
        panic!(
            "UniFFI scaffold file {} could not be read: {error}",
            udl_path.display()
        )
    });

    for required_anchor in [
        "namespace deepseek_mobile_agent_core",
        "dictionary IosConnectionProfile",
        "dictionary IosBootstrapStep",
        "dictionary IosMobileEvent",
        "interface IosMobileAgentCore",
    ] {
        assert!(
            udl.contains(required_anchor),
            "UniFFI scaffold {} is missing `{required_anchor}`",
            udl_path.display()
        );
    }

    if env::var_os("DEEPSEEK_UNIFFI_GENERATE_SCAFFOLDING").is_some() {
        uniffi::generate_scaffolding("src/uniffi_api.udl")
            .expect("UniFFI Rust scaffolding generation should succeed");
    }
}
