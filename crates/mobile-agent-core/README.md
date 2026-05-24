# DeepSeek Mobile Agent Core

This crate owns the Rust side of the phone-facing agent core. Its UniFFI
surface is intentionally narrow and JSON-oriented so Swift can link the core
without importing desktop TUI, shell, PTY, LSP, or local MCP process support.

## UniFFI Dry-Run

Linux CI should check the generation plan without invoking Xcode, Swift, or the
UniFFI generator:

```sh
crates/mobile-agent-core/scripts/uniffi-dry-run --check-plan
crates/mobile-agent-core/scripts/uniffi-dry-run --check-artifacts
crates/mobile-agent-core/scripts/uniffi-dry-run --install-artifacts
crates/mobile-agent-core/scripts/uniffi-dry-run --print-commands
```

The planned macOS binding commands are:

```sh
uniffi-bindgen generate crates/mobile-agent-core/src/uniffi_api.udl --language swift --out-dir crates/mobile-agent-core/target/uniffi-swift
cargo build -p deepseek-mobile-agent-core --release
```

The Rust crate is configured to produce `lib`, `staticlib`, and `cdylib`
artifacts. The generated handoff is expected to use these iOS package paths:

```text
ios/DeepSeekMobileDemo/Generated/DeepSeekMobileAgentCore.swift
ios/DeepSeekMobileDemo/Generated/deepseek_mobile_agent_coreFFI.modulemap
ios/DeepSeekMobileDemo/Artifacts/libdeepseek_mobile_agent_core.a
ios/DeepSeekMobileDemo/Artifacts/DeepSeekMobileAgentCore.xcframework
```

`--check-plan` validates the UDL anchors, crate metadata, and documented paths
without requiring generated files. `--check-artifacts` validates the same plan
and then fails clearly until the generated Swift file, module map, and either
the static archive or `.xcframework` are present. `--install-artifacts` copies
mock or generated output from `DEEPSEEK_UNIFFI_ARTIFACT_ROOT` into the iOS
handoff layout and then runs the artifact check.

Linux tests can exercise the success path without Swift, Xcode, or real UniFFI
output by pointing the install and artifact checks at temporary roots:

```sh
DEEPSEEK_UNIFFI_ARTIFACT_ROOT=/tmp/deepseek-uniffi-source \
DEEPSEEK_UNIFFI_HANDOFF_ROOT=/tmp/deepseek-uniffi-handoff \
  crates/mobile-agent-core/scripts/uniffi-dry-run --install-artifacts

DEEPSEEK_UNIFFI_HANDOFF_ROOT=/tmp/deepseek-uniffi-handoff \
  crates/mobile-agent-core/scripts/uniffi-dry-run --check-artifacts
```

The mock source root may contain the Swift and modulemap files directly or under
`Generated/`. The Rust artifact may be
`Artifacts/libdeepseek_mobile_agent_core.a` or
`Artifacts/DeepSeekMobileAgentCore.xcframework`, with direct files at the
source root also accepted for small shell smoke tests. The install step copies
them to `Generated/DeepSeekMobileAgentCore.swift`,
`Generated/deepseek_mobile_agent_coreFFI.modulemap`, and the matching
`Artifacts/` path under `DEEPSEEK_UNIFFI_HANDOFF_ROOT`.

By default, `build.rs` validates that `src/uniffi_api.udl` still contains the
expected mobile boundary anchors. To exercise UniFFI Rust scaffolding generation
locally, set:

```sh
DEEPSEEK_UNIFFI_GENERATE_SCAFFOLDING=1 cargo build -p deepseek-mobile-agent-core
```
