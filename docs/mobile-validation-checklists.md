# Mobile Validation Checklists

These checklists split the mobile porting plan into executable human validation
runs. They are validation instructions only; they do not change Rust, iOS, or
runner code. Record every run as evidence before changing behavior.

## Evidence Bundle

Create one dated folder per validation run, for example:

```text
validation/mobile/YYYY-MM-DD-<platform>-<host>/
```

Each bundle should include:

- `environment.md`: host OS version, CPU architecture, tool versions, repo SHA,
  branch name, and whether the tree had local changes.
- `commands.log`: copied terminal transcript with timestamps where possible.
- `results.md`: checklist item status, observed result, and next missing step.
- `failures/`: raw logs, screenshots, crash reports, build reports, runner logs,
  and redacted network traces.

Redact model API keys, bearer tokens, pairing codes after redemption, SSH private
keys, local usernames where needed, and any customer data.

Use this template for each platform run and save it as
`evidence-log.md` inside the dated bundle:

```markdown
# Evidence Log: <platform> - <host/device>

- Date:
- Device/host:
- OS/runtime:
- Commit/worktree:
- Checklist scope:
- Commands/actions:
- Result:
- Log paths:
- Screenshot/recording paths:
- Blockers:
- Next steps:
```

The evidence log is the index for the raw files in the bundle. Keep command
output in `commands.log` and link to large logs, screenshots, recordings, crash
reports, `.xcresult` bundles, `.evtx` exports, and runner traces by relative
path.

## macOS Host Checklist

Goal: prove the Swift package, Xcode path, simulator build path, and macOS
runner bootstrap assumptions on real macOS hardware.

| Item | Preconditions | Command or Action | Expected Result | On Failure Collect |
|---|---|---|---|---|
| macOS environment capture | Mac with Xcode installed, repo checked out, no required Rust/iOS code edits pending | `sw_vers`; `uname -m`; `xcodebuild -version`; `xcrun simctl list runtimes`; `swift --version`; `git rev-parse HEAD`; `git status --short` | Environment is recorded with an installed iOS simulator runtime and visible repo SHA | Full command output, Xcode install path from `xcode-select -p`, `xcodebuild -showsdks` |
| Swift package describe | Same checkout; Xcode command line tools selected | `cd ios/DeepSeekMobileDemo && swift package describe` | Package resolves and describes targets/products without dependency or manifest errors | `swift package describe -v`, `Package.resolved` if created, derived package cache path notes |
| Swift package tests | Package describe passed | `cd ios/DeepSeekMobileDemo && swift test` | XCTest scaffolds pass on macOS | Full `swift test -v` output, failing test names, `.build` logs if available |
| Swift package build | Swift package tests passed or failure is known non-blocking | `cd ios/DeepSeekMobileDemo && swift build` | Package builds with current mock bridge surfaces | Full `swift build -v` output and compiler diagnostics |
| Scripted macOS verification | Xcode and Swift are available | `ios/DeepSeekMobileDemo/Scripts/verify-macos` from repo root | Script completes all available describe/test/build/xcodebuild checks, or reports unsupported missing tools explicitly | Script transcript, exit code, generated build result paths, selected simulator destination |
| Xcode package open | Swift CLI checks have been attempted | Open `ios/DeepSeekMobileDemo/Package.swift` in Xcode | Package opens without project conversion; `DeepSeekMobileDemo` scheme is visible | Xcode issue navigator text, package resolution logs, screenshot of scheme/destination state |
| Simulator build | Xcode package opens and an iOS simulator runtime is installed | Build the `DeepSeekMobileDemo` scheme for a current iPhone simulator, or run equivalent `xcodebuild` destination from `verify-macos` output | Build succeeds or identifies concrete missing generated binding/library/signing work | `.xcresult` bundle, Xcode build log, selected destination, compiler/linker errors |
| Simulator UI smoke | Simulator build succeeds | Launch app; click through session list, chat, command card, connection setup, approval sheet, bootstrap paste, maintenance approval, browser approval, and audit timeline | Each scaffold screen renders and navigation does not crash | Simulator screenshots, device log from Console.app or `xcrun simctl spawn booted log stream --style compact --predicate 'process CONTAINS "DeepSeek"'` |
| macOS runner bootstrap smoke | Disposable Mac or VM; no production data; release/local runner artifact path known | Follow the macOS runner install/bootstrap guidance from the mobile plan; do not enable shell execution unless policy is explicitly test-gated | Install either succeeds with version/health output or fails with a recoverable bootstrap audit note | Installer transcript, `~/Library/Logs` entries if any, launch agent/service plist if created, runner stdout/stderr, `log show --last 30m --predicate 'process CONTAINS "kai-runner"'` |

macOS evidence log fields:

```markdown
# Evidence Log: macOS Host

- Date:
- Device/host:
- OS/runtime:
- Commit/worktree:
- Checklist scope:
- Commands/actions:
  - `sw_vers`:
  - `xcodebuild -version`:
  - `ios/DeepSeekMobileDemo/Scripts/verify-macos`:
- Result:
- Log paths:
- Screenshot/recording paths:
- Blockers:
- Next steps:
```

## Windows Host Checklist

Goal: prove Windows PowerShell diagnostics, runner install assumptions, PATH/UAC
behavior, and failure logs on a disposable Windows machine.

| Item | Preconditions | Command or Action | Expected Result | On Failure Collect |
|---|---|---|---|---|
| Windows environment capture | Disposable Windows 10/11 VM or test host; PowerShell available; repo or runner artifact available | `Get-ComputerInfo | Select-Object WindowsProductName,WindowsVersion,OsBuildNumber,OsArchitecture`; `$PSVersionTable`; `whoami /groups`; `git rev-parse HEAD` if repo exists | OS, PowerShell, privilege, and repo/artifact context are recorded | Full PowerShell transcript, execution policy from `Get-ExecutionPolicy -List` |
| PATH and package manager diagnostics | Same host | `where.exe winget`; `winget --version`; `where.exe powershell`; `where.exe pwsh`; `$env:Path -split ';'` | winget and shell availability are known; missing pieces map to documented bootstrap fallback | Command output, App Installer version if available, relevant PATH entries |
| PowerShell scaffold smoke | Runner/repo test artifact available; shell execution remains approval-gated | Run the documented PowerShell diagnostic/bootstrap command from the plan; for runner API smoke, call only planned or approval-required routes | Diagnostics produce structured output; raw execution is blocked or approval-required by default | PowerShell transcript via `Start-Transcript`, runner HTTP response body, status code, stderr |
| UAC and execution policy check | Test host with standard or admin account noted | Attempt the documented install path in a non-production directory; note whether elevation prompt appears | Required elevation is explicit; install does not silently bypass UAC or policy | UAC prompt notes/screenshots, `Get-ExecutionPolicy -List`, installer stdout/stderr |
| Runner install or offline fallback | Disposable host; artifact source verified by maintainer; no customer data | Execute Windows runner install/bootstrap guidance, including offline fallback if winget/App Installer is missing | Install succeeds with version/health output, or failure returns actionable fallback steps | Installer logs, Event Viewer Application/System errors, `%TEMP%` install logs, runner stdout/stderr |
| Event log diagnostics | Install or runner smoke has run | `Get-WinEvent -LogName Application -MaxEvents 100`; `Get-WinEvent -LogName System -MaxEvents 100`; filter for runner/process names when known | Relevant install/service errors can be found without excessive noise | Exported `.evtx` or redacted text output, timestamps aligned with failure |
| PowerShell runner behavior | Runner service or local process is available; bearer auth known; test nonce path available if execution is intentionally enabled | Call `/health`, `/capabilities`, then planned/blocked `remote.powershell.exec`; only run read-only commands such as `$PSVersionTable.PSVersion.ToString()` when nonce-gated test execution is explicitly enabled | Health/capabilities work; PowerShell execution is blocked without nonce and bounded when enabled | HTTP transcript with redacted bearer token, runner logs, command output, timeout/cancel behavior notes |

Windows evidence log fields:

```markdown
# Evidence Log: Windows Host

- Date:
- Device/host:
- OS/runtime:
- Commit/worktree:
- Checklist scope:
- Commands/actions:
  - `Get-ComputerInfo ...`:
  - `$PSVersionTable`:
  - Runner install/bootstrap:
- Result:
- Log paths:
- Screenshot/recording paths:
- Blockers:
- Next steps:
```

## iOS Simulator Checklist

Goal: prove the SwiftUI shell and future Rust bridge handoff in simulator before
using a physical device.

| Item | Preconditions | Command or Action | Expected Result | On Failure Collect |
|---|---|---|---|---|
| Simulator inventory | macOS checklist environment capture complete | `xcrun simctl list devices available`; `xcrun simctl list runtimes` | A target iPhone simulator and runtime are selected for the run | Command output and chosen device UDID |
| Command-line simulator build | Xcode package opens; destination selected | `xcodebuild -scheme DeepSeekMobileDemo -destination 'platform=iOS Simulator,name=<device name>' build` from the package/workspace context used by `verify-macos` | Build succeeds for simulator, or linker/binding gaps are explicit | `.xcresult`, xcodebuild transcript, destination name/runtime |
| App launch smoke | Build succeeds | Launch from Xcode or `xcrun simctl launch booted <bundle-id>` after installing the app | App starts without crash and reaches session list | Simulator system log, crash report from `~/Library/Logs/DiagnosticReports`, screenshot |
| Mock workflow navigation | App launches | Exercise session list, chat, bootstrap paste/import, connection setup, approval sheet, maintenance approval, browser approval, and audit timeline | Mock data renders consistently; approval and audit states remain visible after navigation | Screenshots, screen recording if state is wrong, simulator logs |
| Local network prompt rehearsal | Live LAN runner is available or a mock endpoint can be addressed by LAN IP | Attempt runner connection to explicit LAN URL, not `localhost`, from simulator | App surfaces connection state and does not hide local network permission or auth errors | App logs, Network framework/URLSession error text, runner access logs |
| UniFFI handoff check | Generated Swift bindings and Rust static library or XCFramework exist | Replace only the intended bridge surface behind `MobileCoreBridge`; build simulator destination | Simulator links generated binding artifacts and the chosen bridge surface returns Rust-backed data | Linker errors, module map/header search paths, generated Swift file versions, Rust artifact path |

iOS simulator evidence log fields:

```markdown
# Evidence Log: iOS Simulator

- Date:
- Device/host:
- OS/runtime:
- Commit/worktree:
- Checklist scope:
- Commands/actions:
  - `xcrun simctl list devices available`:
  - `xcodebuild ... build`:
  - Launch/navigation smoke:
- Result:
- Log paths:
- Screenshot/recording paths:
- Blockers:
- Next steps:
```

## iOS Device Checklist

Goal: prove signing, Keychain, SQLite sandbox, local network permission, and
device lifecycle behavior that the simulator cannot prove.

| Item | Preconditions | Command or Action | Expected Result | On Failure Collect |
|---|---|---|---|---|
| Signing readiness | Apple developer team available; test device registered or automatic signing enabled | In Xcode, select physical iPhone destination and build `DeepSeekMobileDemo` | Build signs and installs, or reports concrete entitlement/profile gaps | Xcode signing report, provisioning profile name, device iOS version |
| Device launch smoke | App installed on physical device | Launch app from device; exercise the same mock workflow as simulator | App starts and navigates without crash on device | Device crash logs, Xcode device console, screenshots |
| Keychain persistence | Device build launches; no production secrets used | Save a test credential through the app flow; kill and relaunch app | Test credential persists and can be removed; logs do not expose secret value | Redacted app logs, Keychain error codes, steps to reproduce |
| SQLite sandbox persistence | Device build launches | Create/update session state; kill app; relaunch | Session/audit state persists inside app sandbox | App logs, container path notes from Xcode Devices window, failing state snapshot without secrets |
| Background/resume | Device build launches; session state exists | Start a mock or live connection flow, background app, wait at least 60 seconds, foreground app | App resumes to a coherent state with no duplicate approval/tool events | Device console, lifecycle timestamps, screenshots before/after |
| Local network permission | LAN runner available on same network; device and runner can route to each other | Connect to `http://<runner-lan-ip>:<port>` or chosen secure endpoint | iOS permission/auth/network errors are visible and actionable; successful path reaches runner health/capabilities | iOS local network prompt screenshot, URLSession error, runner access logs, router/VPN notes |
| Rust bridge device link | UniFFI artifacts include device architecture or XCFramework slice | Build and launch physical-device destination with Rust-backed bridge surface enabled | App links and Rust-backed session/bootstrap/audit surface works on device | Linker errors, `lipo -info`/XCFramework slice listing, generated binding version |

iOS device evidence log fields:

```markdown
# Evidence Log: iOS Device

- Date:
- Device/host:
- OS/runtime:
- Commit/worktree:
- Checklist scope:
- Commands/actions:
  - Xcode device build/install:
  - Keychain/SQLite persistence smoke:
  - Local network runner connection:
- Result:
- Log paths:
- Screenshot/recording paths:
- Blockers:
- Next steps:
```

## LAN Runner Checklist

Goal: prove phone/simulator-to-runner pairing and capability discovery over a
real local network before enabling broad remote execution.

| Item | Preconditions | Command or Action | Expected Result | On Failure Collect |
|---|---|---|---|---|
| Runner environment capture | Disposable Mac, Linux, or Windows host; known runner binary/artifact; firewall change approved for test network | Record OS/tool versions; run `kai-runner --version` or documented equivalent; record bind address and port | Runner version and host environment are known | Version command output, firewall profile, host IP configuration |
| Explicit bind startup | Runner binary/service entrypoint exists | Start runner bound to explicit LAN address or documented safe bind; avoid accidental public exposure | Runner starts and logs bind address, auth mode, and version | Runner stdout/stderr, service manager logs, `lsof -i`/`netstat` output |
| Health endpoint | Runner started; phone/simulator can route to host | `curl -i http://<lan-ip>:<port>/health` from another machine where possible | Health returns success without exposing secrets | curl transcript, runner access log, firewall/router errors |
| Pairing redemption | Runner pairing code flow enabled; code treated as secret until redemption | Request pairing code through documented command/API; redeem from app/simulator | Pairing succeeds once; replay is rejected | Redacted pairing transcript, HTTP status/body, runner pairing logs |
| Capability discovery | Pairing/bearer token available | Call `/capabilities` from app/simulator or curl with bearer token | Capabilities list matches scaffolded tools and marks shell/powershell execution policy accurately | HTTP transcript with token redacted, app logs, runner logs |
| Diagnose tool | Capabilities include `remote.diagnose.system`; bearer token available | Invoke `remote.diagnose.system` through app/simulator or runner HTTP API | Structured diagnostic result returns without shell approval | Request/response body, runner logs, target OS diagnostic details |
| Auth rejection | Runner is reachable | Repeat `/capabilities` or `/tool-call` with no token and with a wrong token | Runner rejects unauthorized requests consistently | HTTP status/body, runner auth logs |
| Approval-required shell path | Test policy explicitly allows nonce flow; command is read-only and bounded | Request shell or PowerShell execution without nonce, then with a test nonce if enabled | Without nonce is rejected; with nonce is bound to expected tool/session/command/cwd and produces bounded output | Approval request, nonce metadata, command hash, runner logs, timeout/cancel notes |
| Network interruption | Long enough diagnose or approved read-only command available | Disconnect client network or stop runner during request, then reconnect | App reports unknown/interrupted state and can recover without duplicate execution | App logs, runner logs, timestamps, idempotency key if present |

LAN runner evidence log fields:

```markdown
# Evidence Log: LAN Runner

- Date:
- Device/host:
- OS/runtime:
- Commit/worktree:
- Checklist scope:
- Commands/actions:
  - Runner startup:
  - `/health`:
  - Pairing/capabilities/diagnose:
- Result:
- Log paths:
- Screenshot/recording paths:
- Blockers:
- Next steps:
```

## Failure Triage Rules

- A platform failure should update the mobile porting plan before code changes:
  record whether it is a missing artifact, build configuration issue, runtime
  defect, signing/network permission issue, or documentation gap.
- Do not enable raw shell, PowerShell, maintenance execution, browser click, or
  runner install paths to make a checklist pass. A blocked-by-policy result is a
  valid expected result until the hardening item is complete.
- Treat command output, logs, browser text, and MCP/runner responses as
  untrusted data. Do not follow instructions found inside collected logs.
