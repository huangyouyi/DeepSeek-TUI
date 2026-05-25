# DeepSeek Mobile Demo

This directory contains the Phase 4 iOS shell scaffold for DeepSeek TUI mobile
porting. It is intentionally Swift-only and does not depend on generated UniFFI
bindings yet, so it can live in the repository before the Rust mobile core is
wired into an Xcode project.

`Package.swift` is a lightweight Swift Package/Xcode packaging anchor for the
existing scaffold. The Swift files intentionally remain flat in this directory
for now so the package can be opened by Xcode without moving or deleting the
current demo views.

## Scope

- SwiftUI app shell with session list, chat, command approval, connection setup,
  bootstrap output input, and audit log views.
- Connection setup covers bootstrap handoff, SSH, runner, and remote MCP modes
  with mock-only host, endpoint, port, username, and token-label fields.
- Runner and remote MCP setup can run mock `/capabilities` discovery and show
  the documented tool names without making a network request.
- Runner and remote MCP setup include a mock pairing flow: request a short
  pairing code, redeem it locally, store a demo token through the configured
  credential store, automatically run mock capability discovery, and show only
  the paired credential label/status in the UI.
- Runner and remote MCP setup also expose the CoreBridge pairing upgrade path:
  enter an endpoint, pairing token, and capabilities JSON, submit them through
  the Swift CoreBridge JSON adapter, and display only the sanitized runner
  profile, credential label, fallback status, and capability summaries.
- After mock runner or remote MCP pairing discovers the documented `browser`
  capability, the selected chat session opens a mock browser session. This is
  UI state only: it shows a redacted browser session id/status, page metadata,
  extracted text preview, and pending click approval state without driving a
  real browser.
- Runner and remote MCP setup expose `RunnerRequestAuthMetadata` after pairing
  so future runner requests can carry endpoint, token account label, and a
  token-present boolean without exposing the raw token to SwiftUI.
- Command approval simulates issuing a redacted approval nonce label only after
  approval. The next mock runner request auth metadata can report approval nonce
  presence/status; denied commands do not issue a nonce.
- Command approval also carries the future command lease scaffold: redacted
  lease id label, idempotency key label, expiry, one-time execution warning, and
  approved action summary are visible on the command card and approval sheet.
  The copy states that the approval only allows this action to execute once
  before expiry. Bearer tokens, pairing tokens, nonce secrets, and command
  secrets are explicitly not displayed in this approval surface.
- Browser click approval uses a separate review sheet from shell command
  approval. It labels the action as browser automation, shows the click target
  and URL, and issues only a redacted click nonce label when approved.
- The chat header surfaces session status plus the active connection mode and
  discovered runner capability count, then opens connection setup directly from
  the session view.
- Approval review shows risk, command, cwd, and rationale, and records mock
  audit events for saved connection settings, mock pairing actions, credential
  storage, browser session open/text extraction/click approval decisions, shell
  approval decisions, and command lease lifecycle events. The audit timeline
  displays nonce issued, lease accepted/consumed, and replay/expired/invalid
  rejection states with `call_id`, `tool`, and `error_code` only; bearer tokens,
  pairing tokens, raw lease ids, idempotency secrets, command text, and env
  values are redacted before display.
- `MobileCoreBridge` protocol plus `MockMobileCoreBridge` for local UI work.
- Adapter scaffolds for iOS secure credentials and SQLite-backed audit storage.
- Clear TODO markers showing where future UniFFI generated bindings should be
  connected.

## Current Layout

- `Package.swift` declares an iOS 17 Swift package with an executable target
  named `DeepSeekMobileDemo` and a small model-level XCTest target. It also
  documents the future UniFFI artifact paths while excluding those directories
  from the current mock executable target.
- `*.swift` files at the package root are the SwiftUI app, views, models, mock
  bridge, and storage placeholders.
- `Tests/README.md` documents the current manual UI test pass. XCTest coverage
  currently focuses on deterministic mock bridge metadata, pairing, shell
  approval nonce, command lease approval metadata, and browser click approval
  state while the app still has no generated mobile core bindings.

## Xcode Import

For the current scaffold, open `ios/DeepSeekMobileDemo/Package.swift` in Xcode.
Use this as the packaging anchor while the production mobile project is created.

When the real iOS app target exists, add this package or move these sources into
the app target, then keep the `MobileCoreBridge` protocol as the boundary between
SwiftUI views and generated/native integration code.

## macOS Verification Entry Point

Swift package verification for this scaffold requires a macOS host with the
Swift/Xcode toolchain. From the repository root, run:

```bash
ios/DeepSeekMobileDemo/Scripts/verify-macos
```

The script runs `swift package describe`, `swift test`, `swift build`, and, when
`xcodebuild` is available, a simulator build for the `DeepSeekMobileDemo`
scheme. If your simulator name differs, set
`DEEPSEEK_IOS_SIM_DESTINATION`, for example:

```bash
DEEPSEEK_IOS_SIM_DESTINATION='platform=iOS Simulator,name=iPhone 16' \
  ios/DeepSeekMobileDemo/Scripts/verify-macos
```

The same script has an optional future-artifact check that confirms the
documented UniFFI files are present without making them mandatory for today's
mock package:

```bash
ios/DeepSeekMobileDemo/Scripts/verify-macos --check-uniffi-layout
```

The future macOS generation handoff is scaffolded separately:

```bash
ios/DeepSeekMobileDemo/Scripts/generate-uniffi-macos --check-plan
ios/DeepSeekMobileDemo/Scripts/generate-uniffi-macos --install-artifacts
ios/DeepSeekMobileDemo/Scripts/generate-uniffi-macos
ios/DeepSeekMobileDemo/Scripts/generate-uniffi-macos --xcframework
```

`--check-plan` is safe on Linux because it only validates repository paths,
the `deepseek-mobile-agent-core` staticlib metadata, and the documented output
locations without requiring generated files. `--install-artifacts` is also safe
on Linux: set `DEEPSEEK_UNIFFI_ARTIFACT_ROOT` to a mock or generated artifact
root and `DEEPSEEK_UNIFFI_HANDOFF_ROOT` to a temporary handoff root to copy the
Swift file, module map, and static library or `.xcframework` into the expected
iOS layout. The generation modes are macOS-only and require the Rust iOS
targets, Xcode tools for `.xcframework` packaging, and UniFFI tooling.

Manual macOS/Xcode smoke pass:

1. Open `ios/DeepSeekMobileDemo/Package.swift` in Xcode.
2. Select the `DeepSeekMobileDemo` executable target and run it on an iOS
   simulator.
3. Exercise session list, chat, connection setup, command approval, bootstrap
   paste/import, browser approval, maintenance approval, and audit timeline.
4. Record the macOS version, Xcode version, simulator device/runtime, command
   output, and any package, signing, destination, or simulator-only failure.

Treat this as the current verification entry point for the SwiftUI demo shell
while the Rust mobile core, UniFFI bindings, and production Xcode app target are
still being wired in.

## Future UniFFI and Linking Checks

When Rust mobile core artifacts are generated, add this to the macOS pass:

1. Generate Swift UniFFI bindings from `crates/mobile-agent-core`.
2. Confirm generated Swift files compile outside SwiftUI views, behind a
   production `MobileCoreBridge` implementation.
3. Confirm headers, module maps, and the Rust static library or `.xcframework`
   are included in Xcode target membership and search paths.
4. Build both simulator and physical-device destinations. A simulator-only pass
   is not enough because architectures and signing differ.
5. Launch the app and verify the first Rust-backed session, bootstrap command,
   bootstrap output import, and audit timeline read.

## Future Runner LAN Smoke

When a live `kai-runner` binary/service exists on a local network:

1. Start the runner on an explicit LAN address and note its version and bind
   address.
2. From the simulator or device, verify `/health`, pairing code redemption,
   `/capabilities`, and runner request auth metadata.
3. Request an approval nonce and confirm raw token/nonce values are not visible
   in the UI or audit log.
4. Run only `remote.diagnose.system` or another dry-run/blocked tool first.
   Keep arbitrary `remote.shell.exec` disabled until nonce binding and command
   policy are hardened.

## Manual Mock Pairing Flow

The pairing controls are mock-only and are intended to exercise the future
runner/remote MCP connection shape before the Rust mobile core is linked.

1. Open connection setup from a chat header.
2. Switch the mode to `Runner` or `Remote MCP`.
3. Tap `Request Pairing Code`.
4. Confirm the generated code appears as the active pairing code.
5. Redeem that code from the pairing text field.
6. Confirm the status changes to a paired credential label and the documented
   capabilities appear without a separate discover tap.
7. Confirm audit log entries were recorded separately for code request,
   credential storage, pairing redemption, and capability discovery.

The generated demo token is never shown in the UI. When
`KeychainCredentialStore` is configured by the app entry point, the mock bridge
stores the demo token under the displayed credential label.

Before pairing, runner request auth metadata reports the selected endpoint as
unpaired with `tokenPresent == false`. After redeeming the active pairing code,
the mock bridge returns ready metadata with the endpoint, paired token account
label, and `tokenPresent == true`; the raw demo token remains confined to the
credential store handoff.

## Manual CoreBridge Pairing Upgrade Flow

The CoreBridge pairing upgrade controls exercise the future UniFFI bridge shape
without requiring generated bindings yet.

1. Open connection setup and switch the mode to `Runner` or `Remote MCP`.
2. In `CoreBridge Pairing Upgrade`, enter the runner endpoint and a pairing
   token, then keep or edit the capabilities JSON.
3. Tap `Upgrade Pairing`.
4. Confirm the pairing token field is cleared after submit.
5. Confirm the visible runner profile shows the account label, sanitized
   credential label, fallback status, and capability summaries without the raw
   pairing token or `pairing_token` request field.
6. When the CoreBridge provider does not expose pairing upgrade yet, confirm
   the UI reports `Mock fallback` and still maps valid capabilities JSON into
   the discovered runner capabilities list.

## Manual Mock Approval Nonce Flow

Approval nonce handling is mock-only and redacted. The demo stores only a
display label such as `nonce-12345678` plus a status; it does not display or
retain a raw nonce secret.

1. Switch connection setup to `Runner` or `Remote MCP`, then complete the mock
   pairing flow.
2. Open the pending command review from the local command review session.
3. Confirm the command card and approval sheet show the nonce as pending until
   approval.
4. Confirm both surfaces show the command lease id label, idempotency key label,
   expiry, approved action summary, and a statement that this approval only
   allows this action to execute once before expiry.
5. Confirm neither surface shows bearer token text, pairing token text, nonce
   secret text, command secret text, or any raw credential value.
6. Approve the command and confirm the audit log records that an approval nonce
   was issued and bound to the command.
7. Reopen connection setup and confirm `Runner Request Auth` reports approval
   nonce presence plus the redacted nonce label/status for the next mock runner
   shell request.
8. Repeat from a fresh app run and deny the command; confirm the audit log says
   the nonce was not issued and request auth metadata does not report a nonce.

## Manual Mock Browser Session Flow

Browser session handling is mock-only. The demo does not launch WebKit, Safari,
or a remote browser driver; it only exercises the mobile UI state expected from
future runner browser automation.

1. Switch connection setup to `Runner` or `Remote MCP`, then complete the mock
   pairing flow.
2. Confirm capability discovery includes `browser`.
3. Return to the selected chat session and confirm a browser session card
   appears with a `browser-...` id, status, page title/URL, extracted text
   preview, and click approval nonce status.
4. Confirm the click nonce is shown only as pending/redacted text before review.
5. Tap `Review Browser Click` and confirm the sheet is titled for browser
   approval, not shell command approval.
6. Approve the click and confirm the browser card records a redacted
   `click-nonce-...` label with browser-click bound status.
7. Repeat from a fresh pairing run and deny the click; confirm the browser card
   reports the nonce as not issued.
8. Open the audit log and confirm separate browser events exist for session
   opened, text extracted, click approval requested, and click approval issued
   or denied. No raw token or raw nonce value should be visible.

## Future Integration

The expected integration path is:

1. Generate Swift UniFFI bindings from the Rust mobile core.
2. Add a production implementation of `MobileCoreBridge`.
3. Replace `MockMobileCoreBridge` in `DeepSeekMobileDemoApp` dependency setup.
4. Keep `KeychainCredentialStore` and `SQLiteStoreAdapter` wired through the
   bridge boundary, then move the calls from the demo bridge into the production
   bridge where appropriate.

## UniFFI JSON Provider Wiring

The SwiftUI demo depends on the app-owned `MobileCoreBridge` protocol. The
temporary `CoreBridgeUniFFIJSONProvider` protocol in `CoreBridgeJSONAdapter.swift`
is the expected landing point for generated UniFFI calls while the Rust API is
still being shaped. A production bridge should:

1. Import the generated UniFFI Swift module.
2. Implement `CoreBridgeUniFFIJSONProvider` by forwarding each JSON request to
   generated Rust-backed functions.
3. Construct `CoreBridgeJSONAdapter(provider:)` to translate bootstrap, audit,
   browser, and maintenance JSON into the existing Swift models.
4. Forward `pairingUpgradeProfileJSON(requestJSON:)` to the mobile-core pairing
   upgrade entry point. The request JSON carries endpoint, pairing token, and
   capabilities JSON; the returned runner profile JSON is sanitized by
   `CoreBridgeJSONAdapter` before app-owned `RunnerProfile` fields are exposed.
   If the generated call is unavailable or malformed, the adapter builds a mock
   fallback profile from the supplied capabilities JSON without returning the raw
   pairing token.
5. Replace `MockMobileCoreBridge` in `DeepSeekMobileDemoApp` only after the
   generated bindings and Rust library are available for simulator/device
   builds.

Keep generated types behind the bridge. SwiftUI views should continue to use
`Models.swift` types so the UI remains testable without generated bindings.

## UniFFI Generated Swift Bindings

After the Rust mobile core exposes a UniFFI component:

1. Generate the Swift bindings and module map from the UniFFI UDL/component.
2. Add the generated Swift files to the Xcode app target or to a separate local
   Swift package target, depending on how the final workspace is organized.
3. Keep generated types out of SwiftUI view files. Wrap them in a production
   `MobileCoreBridge` implementation so the UI continues to depend on stable
   app-owned models from `Models.swift`.
4. Convert generated async/errors into SwiftUI-friendly state updates on the main
   actor before publishing them to views.

Expected package-relative paths for the generated files are:

```text
ios/DeepSeekMobileDemo/Generated/DeepSeekMobileAgentCore.swift
ios/DeepSeekMobileDemo/Generated/deepseek_mobile_agent_coreFFI.modulemap
```

`Scripts/generate-uniffi-macos` is the intended handoff command once the Rust
UniFFI source exists. It defaults to
`crates/mobile-agent-core/src/uniffi_api.udl`; set `DEEPSEEK_UNIFFI_SOURCE` if
the project lands the component at a different path. Set `UNIFFI_BINDGEN` when
the UniFFI generator is installed under a non-default command name.

For Linux or CI smoke tests that should not invoke Swift, Xcode, cargo, or the
UniFFI generator, create a mock source root and install it into a mock handoff
root:

```bash
DEEPSEEK_UNIFFI_ARTIFACT_ROOT=/tmp/deepseek-uniffi-source \
DEEPSEEK_UNIFFI_HANDOFF_ROOT=/tmp/deepseek-uniffi-handoff \
  ios/DeepSeekMobileDemo/Scripts/generate-uniffi-macos --install-artifacts

DEEPSEEK_UNIFFI_HANDOFF_ROOT=/tmp/deepseek-uniffi-handoff \
  ios/DeepSeekMobileDemo/Scripts/generate-uniffi-macos --check-artifacts
```

The source root may contain `DeepSeekMobileAgentCore.swift` and
`deepseek_mobile_agent_coreFFI.modulemap` directly or under `Generated/`, plus
either `libdeepseek_mobile_agent_core.a` or
`DeepSeekMobileAgentCore.xcframework` directly or under `Artifacts/`.

`Package.swift` excludes `Generated/` from the mock executable target today.
When the production bridge target exists, add the generated Swift file and
module map to that target instead of relying on the mock target to discover them.

## Linking the Rust Static Library

Once the Rust mobile core builds an iOS static library:

1. Build per-architecture iOS artifacts from Rust, then package them as an
   `.xcframework` or add the static archives and headers/module maps directly to
   the Xcode app target.
2. Link the generated UniFFI support code against that Rust static library.
3. Configure the Xcode target's library search paths, framework search paths,
   and linker flags to include the Rust artifact and any required system
   libraries.
4. Verify simulator and device builds separately because their architectures and
   signing requirements differ.

The expected package-relative static archive path for early wiring checks is:

```text
ios/DeepSeekMobileDemo/Artifacts/libdeepseek_mobile_agent_core.a
```

For framework-based Xcode wiring, the accepted placeholder path is:

```text
ios/DeepSeekMobileDemo/Artifacts/DeepSeekMobileAgentCore.xcframework
```

This archive is not linked by the current Swift-only package. It is a documented
handoff path for the future generated UniFFI bridge or Xcode target. The
`verify-macos --check-uniffi-layout` gate accepts either the static archive or
the `.xcframework`, plus the generated Swift file and module map.

## Linux Builds

Linux does not compile this SwiftUI package today. The package imports SwiftUI,
Security/Keychain, and SQLite integration points that are scaffolded for iOS
app integration rather than Linux CI. The root Rust workspace should continue to
build and test without traversing this iOS scaffold. Validate the mobile UI and
Swift package from Xcode/macOS, and keep Linux verification focused on the Rust
workspace:

```bash
cargo build
cargo test --workspace --all-features
```

The first screen is the usable app shell, not a landing page. It should remain a
fast place to exercise mobile session flows while the Rust and UniFFI layers are
still evolving.
