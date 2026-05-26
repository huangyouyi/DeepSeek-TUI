# Linux-First Mobile Agent Task Tree

This task tree is the priority track for work that can be built and tested on
the current Linux development machine before real macOS, Xcode, iOS simulator,
iOS device, or Windows host validation is available.

It intentionally does not claim real iOS execution. It validates the mobile
agent core, Swift-facing JSON/UniFFI contracts, remote runner protocol, approval
policy, audit redaction, and phone-to-computer rescue flow using Linux tests and
loopback runner execution.

## Scope Boundary

Linux-first includes:

- `deepseek-mobile-agent-core` agent loop, session state, bootstrap flow,
  approval gate, remote tool schema, persistence, runner transport, audit model,
  and Swift-facing JSON/UniFFI facade tests.
- `kai-runner` HTTP server, pairing, bearer auth, capabilities, approval nonce,
  command lease, shell policy, file tools, package dry-run, browser scaffold,
  MCP scaffold, maintenance dry-run, and audit redaction.
- Linux loopback simulation of the phone-to-computer flow.
- Evidence bundle tooling and plan-update draft generation.
- Linux/LAN Mobile Web AI Agent chat flow: a phone-shaped browser UI talks to a
  Rust HTTP/SSE server, the server loads model configuration read-only, routes
  model-proposed tools through server-side SSH policy, gates high-risk commands
  behind approval, and returns assistant-visible command summaries.
- Linux/LAN Mobile Web server message/tool part contract: the Rust server owns
  typed chat message parts for tool execution and approval continuation while
  preserving the existing HTTP/SSE compatibility surface for test tooling.
- Linux/LAN Mobile Web common-diagnostics chat coverage: fake smoke and Linux
  tests cover natural-language DNS, Docker, OpenWrt/router, and log-summary
  diagnostic requests through the server-owned agent/tool contract.

Linux-first excludes:

- Real SwiftUI rendering.
- Real iOS Keychain behavior.
- Real iOS SQLite sandbox and lifecycle behavior.
- iOS local network permission prompts.
- Xcode build, Swift compilation, simulator launch, signing, and TestFlight.
- Real macOS Homebrew/Xcode CLT host behavior.
- Real Windows PowerShell/UAC/PATH/Event Log behavior.
- Real browser automation engine behavior.
- Real package install/update/uninstall execution.
- Real Mobile Web browser automation evidence.

## Current Execution Wave

This wave is focused on LF-D, LF-E, and LF-I: the Linux iOS execution simulator
contract, the Linux evidence loop, and the pre-real-device completion gate.

The current evidence is Linux-only. Cargo tests validate the mobile core,
runner, Swift-facing JSON/UniFFI contract shapes, approval metadata, audit
redaction, and evidence-parser integration points. This is enough to mark the
LF-D simulator contract items as partial, but it is not real iOS execution and
does not prove SwiftUI, Keychain, SQLite lifecycle, local-network prompts,
Xcode, signing, simulator launch, or device behavior.

LF-E evidence-loop work is also partial: the evidence templates, parser, and
Linux validation entrypoint are integrated enough to review Linux harness output,
but a real dated simulator bundle and hardware-backed iOS evidence are still
unfinished.

LF-I remains a gate, not a completed release claim. LF-I1 is marked done for the
Linux dry-run contract evidence covered by cargo tests; LF-I2 remains open for
the loopback runner execution path; LF-I3 is partial until the full completion
gate, including cargo checks and real-device evidence handoff, has been run and
reviewed.

## Linux-to-Real-Device Blocker Mapping

This mapping closes the Linux documentation blocker only. It does not close any
real macOS, iOS, Windows, or LAN-runner validation claim. A row is closed only
when the named evidence bundle contains the completed `evidence-log.md`,
`results.md`, command logs, screenshots or recordings where applicable, and any
failure artifacts needed to review the platform behavior.

| Blocker | Linux covered content | Real platform behavior Linux cannot prove | Evidence bundle | Tasks closed by accepted evidence |
|---|---|---|---|---|
| macOS host build and bootstrap | Homebrew/Xcode command-card text, shell-plan syntax, runner request shapes, audit redaction, and Linux-safe UniFFI handoff plan checks | `swift test`, Xcode package resolution, generated UniFFI artifact linking, `.xcresult` output, notarization/signing path, Homebrew/Xcode CLT behavior, macOS runner bootstrap on real hardware | `macos-host` | LF-H3, S6, T6, U6, V6, S11, M13 |
| iOS simulator execution | Swift-facing JSON/UniFFI contract shapes, mock bridge payloads, approval/audit JSON, SQLite persistence model tests in Rust | SwiftUI rendering, XCTest execution in simulator, generated Swift bindings linked into the app, simulator networking, app lifecycle, and SQLite adapter behavior inside the iOS simulator sandbox | `ios-simulator` | LF-H4 simulator portion, S7 simulator portion, T7 simulator portion, U7 simulator portion, V7 simulator portion, M0, M3, M13, M14 |
| Physical iOS device execution | Approval, pairing, bootstrap, audit, and runner transport contracts covered by Linux tests and mock Swift-facing payloads | Code signing/provisioning, device install, Keychain behavior, local-network permission prompt, background/resume behavior, real phone-to-runner connectivity, TestFlight/App Store review issues | `ios-device` | LF-H4 device portion, S7 device portion, T7 device portion, U7 device portion, V7 device portion, M1, M2, M5, M6 |
| LAN runner phone-to-computer flow | Loopback runner pairing/auth/capability/diagnose shapes, bearer redaction, approval nonce and command lease lifecycle, local HTTP smoke coverage | Real LAN discovery/reachability, firewall behavior, host bind address correctness, phone-to-host pairing, auth rejection over the network, diagnose execution against a disposable runner host | `lan-runner` | LF-C loopback-to-real handoff, LF-H6 runner blocker row, S8, T8, U8, V8, M6, M9 |
| Windows runner and PowerShell host | PowerShell request/response schema, blocked default route behavior, approval-required scaffold, Windows bootstrap text, and bearer-auth route behavior | Real PowerShell execution, UAC behavior, PATH/App Installer/winget behavior, execution policy, Event Log capture, Windows runner service lifecycle, Windows firewall behavior | `windows-host` | LF-H5, S9, T9, U9, V9, M12, M16 |
| Package installer and runner maintenance execution | Dry-run package/maintenance plans, nonce/audit metadata, rollback guidance text, and redacted maintenance audit parsing | Real install/update/uninstall execution, signed artifact availability, rollback from failure, host package-manager side effects, notarized macOS artifact and Windows package behavior | `macos-host`, `windows-host`, `lan-runner` | LF-C7, LF-F5, S6, S8, S9, T6, T8, T9, M8, M10 |
| Browser automation engine | Browser open/extract/click schema, fake/scaffold engine behavior, click approval shape, and browser session metadata | Real Playwright/WebDriver/Safari/Chrome engine binding, hardened browser profile isolation, page interaction reliability, screenshot/text extraction fidelity, approval-bound click behavior on a real host | `macos-host`, `windows-host`, `lan-runner` | LF-C8, LF-F6, S12, M7, M11 |
| Production OS sandbox and secret boundary | Approval gate, nonce replay rejection, command lease metadata, workspace path checks, Linux audit redaction, and bearer-token redaction | macOS Seatbelt policy, Linux runner hardening beyond local tests, Windows sandbox/UAC mediation, cwd allowlist behavior, environment secret containment, real-network replay resistance | `macos-host`, `windows-host`, `lan-runner`, `ios-device` | LF-G5, LF-H6 security blocker row, S8, S9, U8, U9, M12, M15 |
| Timeout/cancel/status semantics | Linux tests cover timeout/cancel terminal events, runner request timeout/cancel/status metadata, status-like HTTP failures, and disconnected runner transport errors | Cross-transport cancellation, long-running runner status polling, network interruption recovery, process cleanup, and mobile UI state reconciliation | `lan-runner`, `macos-host`, `windows-host`, `ios-device` | LF-F8, LF-I completion gate follow-up, S8, S9, T8, T9 |
| Mutating tool idempotency | Linux tests cover replay-safe binding for shell command leases, file write idempotency keys, package install dry-run idempotency, browser click approval metadata, and maintenance self-update/uninstall idempotency | Replay-safe protocol behavior across real network retries, duplicated mobile submissions, package/browser/maintenance side effects, and host restart recovery | `lan-runner`, `macos-host`, `windows-host`, `ios-device` | LF-F9, LF-I completion gate follow-up, S8, S9, T8, T9 |
| Linux evidence bundle and plan draft | Simulator can write a dated Linux loopback evidence bundle into an explicit output root, and the parser consumes it into a reviewable plan draft | Manual maintainer acceptance before updating platform support claims; real platform claims still require their own evidence bundles | Linux loopback bundle under an explicit output root; downstream `macos-host`, `ios-simulator`, `ios-device`, `windows-host`, `lan-runner` bundles for real claims | LF-E5, LF-E6, LF-I6, S10, T10, U10, V10, W2 |
| Linux mobile Web SSH simulator | LAN-accessible Rust HTTP/SSE server, phone-shaped Web UI scaffold, SSH target reachability check, preset SSH diagnostics, approval-gated advanced commands, scriptable smoke/flow checks, optional access-token redaction in helper/evidence output, and one-command evidence generation with `--write-plan-draft` | Real iOS WebView/Safari behavior, iOS local network permission prompt, generated Swift binding link, real runner pairing, production auth hardening, and production OS sandboxing | Linux/LAN Web simulator command output and generated plan draft; not a real-platform evidence bundle | Linux-only follow-up to LF-D/LF-F/LF-G; real claims still require `ios-device`, `ios-simulator`, `lan-runner`, `macos-host`, or `windows-host` bundles |
| Linux mobile Web AI Agent chat | Natural-language chat from a phone-shaped Web UI, server-side DeepSeek/mock model routing, read-only `~/.deepseek/config.toml` loading, server-side SSH tool policy, low-risk diagnostic execution, high-risk approval creation, approved command execution, final assistant summary, copyable report output, and fake-runner/API tests for the approval continuation path | Real iOS Safari/WebView quirks, iOS local network permission prompt, native iOS app integration, production auth hardening, real model quality guarantees, and real LAN runner/device evidence | Linux/LAN Web AI chat manual output, Rust/React/Python tests, and optional Linux Web evidence bundle; not a real-platform evidence bundle | Linux-only AI control-chain follow-up to LF-D/LF-F/LF-G; real claims still require platform evidence bundles |
| Linux mobile Web server message/tool parts | Compatible server-owned typed message parts for text, remote shell tool lifecycle, pending approvals, approved/rejected tool results, final assistant summaries, and `message.part.updated` SSE events | Real iOS Safari/WebView rendering behavior, native iOS app integration, production auth hardening, real package install/update/uninstall, real browser automation, and real macOS/Windows host evidence | Linux Web contract tests, React reducer/API tests, Python smoke tests, and optional LAN Web evidence output; not a real-platform evidence bundle | Linux-only productization follow-up after LF-J; real claims still require platform evidence bundles |
| Linux mobile Web Chat product UI | Phone-friendly Chat-first UI with final answer prominence, copy final answer, Tool Activity rows, collapsed stdout/stderr, secondary diagnostics/advanced command/timeline/audit panels, and preserved server-owned approval/tool policy | Real iOS Safari/WebView rendering behavior, native iOS app integration, production auth hardening, real browser automation, real package install/update/uninstall, and real macOS/Windows host evidence | Linux Web UI component/reducer tests, Vite build/typecheck, Python smoke compatibility tests, and optional LAN Web manual output; not a real-platform evidence bundle | Linux-only product UI follow-up after LF-K; real claims still require platform evidence bundles |
| Linux mobile Web `/web` product and `/debug` split | `/web` serves a product Chat UI aligned with the reference project, `/debug` preserves the testing console, product settings expose SSH target/server status, and static deep links serve the SPA | Real iOS Safari/WebView rendering behavior, native iOS app integration, production auth hardening, real browser automation, real package install/update/uninstall, and real macOS/Windows host evidence | Linux Web route/component tests, Rust static serving tests, Vite build/typecheck, Python smoke compatibility tests, and optional LAN Web manual output; not a real-platform evidence bundle | Linux-only product split follow-up after LF-L; real claims still require platform evidence bundles |
| Linux mobile Web safe approval experience | Approval v2 contract planning, server-side exact command/cwd/session allow policy, `reject_stop` turn-stop semantics, enriched approval metadata/audit, `/web` three-option authorization card requirements, `/debug` compatibility, and fake-server Python smoke compatibility for response aliases | Real iOS Safari/WebView rendering behavior, native iOS app integration, production auth hardening, real package install/update/uninstall, real browser automation, real macOS/Windows host evidence, and manual phone/LAN validation until user-run evidence is accepted | Linux Web contract/unit/smoke tests and task-tree documentation; fake-server smoke only, not a real-platform evidence bundle | Linux-only safe authorization follow-up after LF-M; high-risk commands still require approval unless an exact server-side session allow entry matches |
| Linux mobile Web comfortable Chat + Tool Activity | Tool Activity product polish for advanced users: per-row status/title/exit/duration/approval metadata, Chinese status labels with raw status titles, collapsed stdout/stderr/output summaries, bounded long-output containers, and clearer empty state | Real iOS Safari/WebView rendering behavior, native iOS app integration, production auth hardening, real browser automation, real package install/update/uninstall, real macOS/Windows host evidence, and manual phone/LAN validation until user-run evidence is accepted | Linux Web component tests and task-tree documentation; not a real-platform evidence bundle | Linux-only Phase 3 UI comfort follow-up after LF-N; manual LAN/phone checks remain pending/manual |
| Linux mobile Web reliable multi-turn Agent | Phase 4 server-owned multi-turn contract planning and fake smoke coverage for `AgentTurnRequest.mode`, `retry_turn_id`, same-session continuation, retry body acceptance, and deterministic stopped summaries | Real iOS Safari/WebView rendering behavior, native iOS app integration, production auth hardening, real DeepSeek quality guarantees, real browser automation, real package install/update/uninstall, real macOS/Windows host evidence, and manual phone/LAN validation until user-run evidence is accepted | Linux fake-server Python smoke tests and task-tree documentation; not a real-platform evidence bundle | Linux-only Phase 4 reliability follow-up after LF-O; manual LAN/phone checks remain pending/manual |
| Linux mobile Web common diagnostics | Phase 5/LF-Q fake smoke and Linux-only docs cover natural-language DNS, Docker, OpenWrt/router, and log-summary diagnostic turns, with helper-side assertions for expected command and assistant summary text | Real iOS Safari/WebView rendering behavior, native iOS app integration, production auth hardening, real DeepSeek quality guarantees, real SSH execution, real browser automation, real package install/update/uninstall, real macOS/Windows host evidence, and manual LAN/phone validation until user-run evidence is accepted | Linux fake-server Python smoke tests and task-tree documentation; not a real-platform evidence bundle | Linux-only Phase 5 common-diagnostics follow-up after LF-P; manual LAN/phone checks remain pending/manual |

## Reserved Status Notes

- LF-E5/LF-E6 are covered by simulator evidence bundle generation into an
  explicit output root and parser consumption of that generated bundle.
- The mobile Web SSH simulator under `crates/mobile-web-server`, `mobile-web/`,
  and `scripts/mobile_web_*` extends Linux/LAN development coverage. It proves
  the phone-shaped Web API, SSH diagnostic, approval, audit, and redaction flow
  on Linux only; it does not close any real iOS/macOS/Windows evidence item.
  The AI chat extension additionally proves that natural-language requests can
  travel through server-side model/tool routing, approval, SSH execution, and a
  final assistant-visible result without giving the browser direct SSH,
  process, local-file, or API-key access.
  Its final repeatable verification path is
  `scripts/mobile_web_ssh_evidence.py --auto-approve --write-plan-draft`
  against an already-running Rust server. The smoke path includes
  `POST /api/ssh/check` for target reachability before diagnostic and approval
  execution.
- LF-G6 is covered by `scripts/mobile_secret_scan.py` and its smoke test, which
  are included in Linux validation without scanning the whole repository.
- LF-F8 is covered by Linux tests for timeout/cancel terminal events, runner
  request metadata, explicit status-like HTTP errors, and disconnected
  transports.
- LF-F9 is covered by Linux tests for shell, file write, package install,
  browser click, and runner maintenance idempotency/replay-safe metadata.

## Plan Agreement Check

`docs/mobile-porting-plan.md` Stage W already treats this task tree as the
priority Linux execution track and keeps real macOS, iOS, LAN runner, and
Windows evidence blocked until hardware-backed bundles are collected. The
mapping above keeps this task tree aligned with the plan by routing each
remaining hardware claim to `macos-host`, `ios-simulator`, `ios-device`,
`windows-host`, or `lan-runner` evidence before any platform support claim can
be closed.

## Linux-First Milestones

| Milestone | Goal | Human-Testable Result on Linux | Status |
|---|---|---|---|
| LF-M0 | Baseline contracts pass | Rust core/runner tests, fmt, clippy pass | done |
| LF-M1 | Bootstrap rescue flow | Homebrew/Xcode and Windows bootstrap cards can be generated and audited | done |
| LF-M2 | Runner pairing loopback | Local runner pairing, bearer auth, capabilities, and diagnose smoke pass | done |
| LF-M3 | Approval safety | High-risk shell/PowerShell flows require nonce and command lease | done |
| LF-M4 | iOS-facing contract simulation | UniFFI/JSON facade returns Swift-friendly session, connection, approval, audit payloads | partial |
| LF-M5 | Linux iOS execution simulator | One command simulates phone flow from bootstrap to runner audit | done |
| LF-M6 | Evidence bundle loop | Linux simulator writes evidence bundle and parser emits plan update draft | done |
| LF-M7 | Remote tool hardening | Idempotency, cancellation/timeout, streaming event model, and replay rejection are covered | done |
| LF-M8 | Browser/installer scaffold proof | Browser open/extract/click approval and package install dry-run are covered by Linux smoke | partial |
| LF-M9 | Ready-for-real-device handoff | Linux produces a checklist/evidence pack for macOS/iOS/Windows validation | partial |
| LF-M10 | Mobile Web AI control chain | Phone-shaped Web chat drives server-side model/tool routing, approval-gated SSH execution, and final assistant summaries against a Linux host | done |
| LF-M11 | Mobile Web product contract | Server-owned typed message/tool parts make session messages the source of truth for chat text, SSH tool lifecycle, approvals, approval results, and final assistant summaries while preserving legacy HTTP/SSE compatibility | done |
| LF-M12 | Mobile Web Chat product UI | Chat is the first-class mobile experience, Tool Activity replaces raw timeline as the main execution view, final answers are prominent and copyable, and debug/testing panels remain available but secondary | done |
| LF-M13 | Mobile Web `/web` product and `/debug` split | `/web` is the product Chat entry, `/debug` preserves the test console, product settings configure SSH target/server state, and static deep links work on the Rust server; phone/LAN validation remains manual | partial/manual |
| LF-M14 | Mobile Web safe approval experience | Approval v2 is implemented around server-side exact command/cwd/session allow scope, high-risk allow misses still require approval, `reject_stop` stops the active turn, `/web` gets a three-option authorization card, `/debug` remains compatible, and manual LAN/phone validation remains pending | partial/manual |
| LF-M15 | Mobile Web comfortable Chat + Tool Activity | Tool Activity is an advanced detail area with readable metadata, collapsed summarized output, bounded long-output containers, product status labels, and clear empty state; manual LAN/phone validation remains pending | partial/manual |
| LF-M16 | Mobile Web reliable multi-turn Agent | Phase 4 covers server-owned continuation/retry/stop contracts, deterministic fallback expectations, fake smoke coverage, and explicit Linux-only evidence boundaries; manual LAN/phone validation remains pending | partial/manual |
| LF-M17 | Mobile Web common diagnostics | Phase 5 covers common natural-language troubleshooting entries for DNS, Docker, OpenWrt/router, and logs through fake smoke and Linux-only docs; manual LAN/phone validation remains pending | partial/manual |

## Task Tree

```text
Linux-First Mobile Agent Task Tree
├─ LF-A. Baseline Linux validation
│  ├─ [done] LF-A1. `cargo fmt --all --check`
│  ├─ [done] LF-A2. `cargo test -p deepseek-mobile-agent-core -p kai-runner`
│  ├─ [done] LF-A3. `cargo clippy -p deepseek-mobile-agent-core -p kai-runner --all-targets --all-features`
│  ├─ [done] LF-A4. `python3 scripts/mobile_linux_validation.py`
│  └─ [done] LF-A5. `git diff --check -- docs ios/DeepSeekMobileDemo crates/kai-runner crates/mobile-agent-core scripts Cargo.toml Cargo.lock`
│
├─ LF-B. Mobile core contract coverage
│  ├─ [done] LF-B1. Agent loop handles text-only, low-risk tools, and high-risk pending approvals
│  ├─ [done] LF-B2. Bootstrap sessions generate command cards and ingest user-submitted output
│  ├─ [done] LF-B3. Cloud model request/response and streaming shapes are tested without real API keys
│  ├─ [done] LF-B4. Session snapshots and pending turns round-trip through memory and SQLite stores
│  ├─ [done] LF-B5. Runner transport request shapes include bearer metadata without leaking tokens
│  ├─ [done] LF-B6. Command lease metadata is generated for high-risk shell and PowerShell approvals
│  ├─ [done] LF-B7. Runner audit entries are parsed into redacted mobile audit events
│  └─ [partial] LF-B8. Swift-facing JSON facade is tested on Linux; generated Swift binding link remains real macOS/iOS work
│
├─ LF-C. Runner loopback coverage
│  ├─ [done] LF-C1. Runner health, capabilities, and diagnose system routes pass local HTTP tests
│  ├─ [done] LF-C2. Pairing code can be redeemed once and rejected after replay or expiration
│  ├─ [done] LF-C3. Bearer-protected routes reject missing or wrong tokens
│  ├─ [done] LF-C4. Approval nonce is required before shell execution paths
│  ├─ [done] LF-C5. Command lease accepted/consumed/replay-rejected lifecycle is audited without leaking secrets
│  ├─ [done] LF-C6. File read/write is confined to runner workspace with backups
│  ├─ [partial] LF-C7. Package install is dry-run/preview only
│  ├─ [partial] LF-C8. Browser engine is fake/scaffold only
│  └─ [partial] LF-C9. MCP proxy shape exists; real MCP server hardening remains open
│
├─ LF-D. Linux iOS execution simulator
│  ├─ [done] LF-D1. Add `scripts/mobile_ios_flow_simulator.py`; Linux cargo tests cover the simulator contract, not real iOS execution
│  ├─ [done] LF-D2. Simulator creates a mobile session through the Swift-facing JSON facade contract
│  ├─ [done] LF-D3. Simulator runs bootstrap Homebrew/Xcode diagnostic command-card flow
│  ├─ [done] LF-D4. Simulator imports pasted terminal output and records an audit event
│  ├─ [done] LF-D5. Simulator pairs with loopback `kai-runner`
│  ├─ [done] LF-D6. Simulator reads runner capabilities and executes low-risk diagnose flow
│  ├─ [done] LF-D7. Simulator requests high-risk shell approval and verifies nonce/command lease metadata
│  ├─ [done] LF-D8. Simulator reads `/audit/recent` and verifies iOS timeline-safe redaction
│  └─ [done] LF-D9. Simulator exits nonzero if expected contract tests fail or secret scanner smoke detects known sentinels
│
├─ LF-E. Evidence loop on Linux
│  ├─ [done] LF-E1. Evidence bundle templates exist for macOS, Windows, iOS simulator/device, and LAN runner
│  ├─ [done] LF-E2. `scripts/mobile_evidence_bundle.py --list` lists tracked templates
│  ├─ [done] LF-E3. `scripts/mobile_lan_runner_evidence.py --dry-run` prepares LAN-runner evidence paths
│  ├─ [done] LF-E4. `scripts/mobile_evidence_plan_draft.py` reads evidence files and bundle directories
│  ├─ [done] LF-E5. Simulator writes a dated Linux loopback evidence bundle under an explicit output root
│  ├─ [done] LF-E6. Evidence parser consumes the simulator bundle and emits a reviewable plan draft
│  ├─ [done] LF-E7. Validation entrypoint includes simulator dry-run, simulator smoke, and secret-scan smoke checks
│  └─ [done] LF-E8. Evidence bundle remains untracked unless explicitly selected for review because simulator writes only to an explicit output root
│
├─ LF-F. Remote tool behavior that can be proven on Linux
│  ├─ [done] LF-F1. `remote.shell.exec` is blocked by default and gated by approval nonce/lease when enabled
│  ├─ [done] LF-F2. `remote.powershell.exec` request/response shape is tested without Windows execution
│  ├─ [done] LF-F3. `remote.file.read` and `remote.file.write` enforce workspace boundaries
│  ├─ [done] LF-F4. `remote.diagnose.system` returns structured runner capability/system output
│  ├─ [partial] LF-F5. `remote.package.install` supports dry-run preview; real install execution remains disabled
│  ├─ [partial] LF-F6. `remote.browser.open`, `extract_text`, and `click` are scaffold/fake-engine tested
│  ├─ [partial] LF-F7. `remote.mcp.call` supports registered fake tools; real server hardening remains open
│  ├─ [done] LF-F8. Tool timeout/cancel/status polling semantics are covered by Linux tests
│  └─ [done] LF-F9. Mutating tool idempotency is uniformly bound across shell, file write, package, browser, and maintenance flows
│
├─ LF-G. Security and redaction gates
│  ├─ [done] LF-G1. Bearer tokens are redacted from runner request metadata and audit output
│  ├─ [done] LF-G2. Approval nonces are consumed once and replay is rejected
│  ├─ [done] LF-G3. Command lease ids, idempotency keys, commands, and env secrets are not exposed in mobile audit output
│  ├─ [done] LF-G4. High-risk shell/PowerShell commands become pending approvals before transport execution
│  ├─ [partial] LF-G5. Production OS sandbox policy remains outside Linux simulator scope
│  └─ [done] LF-G6. Add a single Linux simulator assertion that fails on known secret-like strings in user-visible output
│
├─ LF-H. Handoff to real device validation
│  ├─ [done] LF-H1. macOS/iOS/Windows/LAN validation checklists exist
│  ├─ [done] LF-H2. UniFFI generation scripts provide Linux-safe plan checks and mock artifact handoff checks
│  ├─ [partial] LF-H3. Real macOS/Xcode commands are documented but not executed
│  ├─ [partial] LF-H4. Real iOS simulator/device commands are documented but not executed
│  ├─ [partial] LF-H5. Real Windows host commands are documented but not executed
│  └─ [done] LF-H6. Linux-to-real-device blocker mapping names each evidence bundle that must close the remaining hardware-blocked claims
│
├─ LF-J. Linux Mobile Web AI Agent chat
│  ├─ [done] LF-J1. Add stable `POST /api/sessions/{id}/agent-turn` contract in Rust and TypeScript
│  ├─ [done] LF-J2. Load DeepSeek model config read-only from server-side config without exposing API keys to the browser
│  ├─ [done] LF-J3. Provide mock and DeepSeek-compatible model providers with fake-transport tests
│  ├─ [done] LF-J4. Route natural-language chat through server-side tool policy so raw user text is not executed as shell
│  ├─ [done] LF-J5. Execute low-risk diagnostic tool calls over SSH without approval and create approvals for high-risk commands
│  ├─ [done] LF-J6. Continue approved Agent-originated commands into assistant-visible final summaries
│  ├─ [done] LF-J7. Make AI chat the primary Web workflow while keeping Diagnostics and Advanced Command secondary
│  ├─ [done] LF-J8. Add copyable report output for manual evidence sharing from mobile browsers
│  ├─ [done] LF-J9. Add fake-runner/API and Web tests for chat, approval, final summary, redaction-safe status, and copyable report behavior
│  └─ [done] LF-J10. Manually validate Web chat against `root@192.168.30.244` through approval and final assistant response; this is Linux/LAN Web evidence only
│
├─ LF-K. Linux Mobile Web server message/tool part contract
│  ├─ [done] LF-K0. Make the Rust server message-contract ready for a product Chat UI: session messages expose text answers, remote shell tool lifecycle, pending approvals, approved/rejected results, stdout/stderr, exit metadata, and final summaries through compatible typed parts; `message.part.updated` drives incremental Web updates; legacy HTTP/SSE fields remain compatible
│  ├─ [done] LF-K1. Freeze a compatible typed-part schema that preserves `MessagePart { id, kind, text, data }`
│  ├─ [done] LF-K2. Add `typed_message_parts` health capability and Rust/TypeScript contract tests
│  ├─ [done] LF-K3. Persist low-risk remote shell tool lifecycle as assistant message `kind="tool"` parts
│  ├─ [done] LF-K4. Persist pending approval, approved tool result, rejected tool result, and final summary parts
│  ├─ [done] LF-K5. Emit `message.part.updated` SSE events while keeping legacy tool and approval events
│  ├─ [done] LF-K6. Update Web reducer/API tests to consume typed tool parts without breaking existing timeline behavior
│  ├─ [done] LF-K7. Update Python smoke helpers to accept typed parts while preserving legacy `assistant_text` fallback
│  └─ [done] LF-K8. Run targeted Rust, React, Python, fmt, clippy, and diff-check validation; this remains Linux Web contract evidence only
│
├─ LF-L. Linux Mobile Web Chat product UI
│  ├─ [done] LF-L0. Keep this product UI task tree and docs scoped to Linux/LAN Web evidence only
│  ├─ [done] LF-L1. Add state selectors for Chat messages, latest final answer, Tool Activity rows, copy final answer report, and legacy timeline compatibility
│  ├─ [done] LF-L2. Add ChatView and ToolActivity React components with collapsed stdout/stderr and no browser-side SSH/process/file/API-key behavior
│  ├─ [done] LF-L3. Integrate the product layout into App: Chat first, pending approvals near chat, Tool Activity next, diagnostics/advanced command/timeline/audit as secondary panels
│  ├─ [done] LF-L4. Add copy final answer while preserving copy full report and manual-copy textarea fallback
│  ├─ [done] LF-L5. Add focused React/state tests for final answer, collapsed tool output, dedupe, and secondary timeline behavior
│  └─ [done] LF-L6. Run Web tests, typecheck, build, Python smoke compatibility, fmt, and diff checks; manual `make` LAN testing remains user-run
│
├─ LF-M. Linux Mobile Web `/web` product and `/debug` split
│  ├─ [done] M1.0. Entry route contract: `/web` is the formal product entry, `/debug` is the retained test/debug entry, `/` remains compatible, `/web` and `/debug` static deep links serve the SPA, and `/health`/`/event`/`/api/*` are preserved as server routes
│  ├─ [done] M1.1. Product `/web` Chat shell: product chat is the first-use surface, showing user/assistant text, final answer, tool activity, and approvals from server-owned state
│  ├─ [done] M1.2. Debug `/debug` preservation: Check SSH, Diagnostics, Advanced Command, raw Timeline, recent Audit, Copy report, and smoke-test compatibility remain available under `/debug`
│  ├─ [done] M1.3. SSH target configuration: product settings expose SSH host/user/port, key/server status, and Check SSH through server APIs without exposing API keys or moving SSH policy client-side
│  ├─ [done] M1.4. First remote diagnosis main path: chat supports natural-language diagnosis, server-side low-risk SSH diagnostics, approval-gated high-risk commands, assistant-visible command results, and copyable final report output
│  ├─ [done] M1.5. Browser thin-client boundary: browser renders server state and sends intents only; the server owns model config, API keys, SSH execution, tool policy, approvals, audit, and message/tool persistence
│  ├─ [done] M1.6. Static serving and API safety: Rust tests cover `/web`, `/web/...`, `/debug`, `/debug/...`, static assets, `/health`, `/api/sessions`, and missing API routes not falling back to `index.html`
│  ├─ [done] M1.7. Explicit non-goals: approval v2, multi-round context expansion, real iOS/macOS/Windows evidence, real package install/update/uninstall, and real browser automation are outside this stage
│  └─ [pending/manual] M1.8. Manual phone/LAN handoff: start the LAN server, open `/web` and `/debug` from a phone browser, configure/check SSH target, run first diagnosis, approve explicit high-risk work if created, and confirm final answer/report/debug fallback; this is not real platform evidence unless backed by a separate accepted bundle
│
├─ LF-N. Linux Mobile Web safe approval experience
│  ├─ [done] S2.0. Goal and boundary: this phase is Linux/LAN Web approval contract and UX hardening only
│  ├─ [done] S2.1. Backend approval v2 contract supports approve once, allow this session, reject, and reject/stop response semantics while preserving old aliases
│  ├─ [done] S2.2. Server-side session allow policy stores exact command/cwd/session scope on the server; it is not a frontend local cache
│  ├─ [done] S2.3. `reject_stop` marks the current agent turn stopped and prevents further tool execution for that turn
│  ├─ [done] S2.4. Enriched approval metadata/audit includes risk reason, command, cwd, session/turn ids, response type, allow scope, and redacted result data
│  ├─ [done] S2.5. `/web` renders a three-option authorization card for approve once, allow for this session, and reject/stop
│  ├─ [done] S2.6. `/debug` keeps existing test entry compatibility for approve/reject flows and exposes new aliases without breaking scripts
│  ├─ [done] S2.7. Contract/unit/smoke tests cover response aliases, exact session allow hit/miss, high-risk miss approval, reject/stop, `/web` card behavior, `/debug` compatibility, and Python fake-server smoke
│  └─ [pending/manual] S2.8. Manual LAN/phone handoff remains user-run and must not be marked done without accepted evidence
│
├─ LF-O. Linux Mobile Web comfortable Chat + Tool Activity
│  ├─ [done] S3.0. Goal and boundary: this phase is Linux/LAN Web Chat + Tool Activity comfort only, excluding real iOS/macOS/Windows evidence, real browser automation evidence, and real package install/update/uninstall evidence
│  ├─ [done] S3.1. Tool Activity rows show status, command title/command, exit code, duration, and approval marker when present
│  ├─ [done] S3.2. stdout, stderr, and output are collapsed by default with line-count and character-count summaries
│  ├─ [done] S3.3. Long output renders inside bounded scrollable `pre.tool-activity__stream` containers
│  ├─ [done] S3.4. Empty state explains that tool execution details appear in Tool Activity
│  ├─ [done] S3.5. Component tests cover collapsed stdout/stderr, long output containment, status labels, and empty state
│  ├─ [done] S3.6. Targeted Linux component verification is recorded in the Phase 3 task tree; broader fmt/diff checks remain command-gated by the worker
│  ├─ [pending/manual] S3.7. Manual LAN/phone validation remains user-run and must not be marked done without accepted evidence
│  └─ [pending/manual] S3.8. Acceptance boundary remains Linux Web only until separate real-platform evidence bundles are accepted
│
├─ LF-P. Linux Mobile Web reliable multi-turn Agent
│  ├─ [done] S4.0. Goal and boundary: Phase 4 is Linux/LAN Web Agent reliability contract work only, excluding real iOS/macOS/Windows evidence, real browser automation evidence, and real package install/update/uninstall evidence
│  ├─ [done] S4.1. `AgentTurnRequest` compatibility includes optional `mode` and `retry_turn_id` while legacy requests without those fields remain accepted
│  ├─ [done] S4.2. Fake smoke coverage proves a same-session second turn with `mode=continue` can depend on the previous network-diagnosis turn without calling DeepSeek
│  ├─ [done] S4.3. Fake smoke coverage accepts a `mode=retry` body with `retry_turn_id`
│  ├─ [done] S4.4. Fake stop endpoint returns a deterministic stopped summary for the requested turn id
│  ├─ [pending/manual] S4.5. Manual LAN/phone validation remains user-run and must not be marked done without accepted evidence
│  └─ [pending/manual] S4.6. Acceptance boundary remains Linux Web only until separate real-platform evidence bundles are accepted
│
├─ LF-Q. Linux Mobile Web common diagnostics
│  ├─ [done] S5.0. Goal and boundary: Phase 5 is Linux/LAN Web common troubleshooting diagnostics only, excluding real iOS/macOS/Windows evidence, real browser automation evidence, and real package install/update/uninstall evidence
│  ├─ [done] S5.1. Fake smoke covers natural-language DNS diagnostics without calling DeepSeek or opening a real SSH connection
│  ├─ [done] S5.2. Fake smoke covers natural-language Docker diagnostics without calling DeepSeek or opening a real SSH connection
│  ├─ [done] S5.3. Fake smoke covers natural-language OpenWrt/router diagnostics without calling DeepSeek or opening a real SSH connection
│  ├─ [done] S5.4. Fake smoke covers natural-language log-summary diagnostics without calling DeepSeek or opening a real SSH connection
│  ├─ [done] S5.5. Smoke helper assertions can require exact expected command text and an assistant-summary substring for common diagnostic turns
│  ├─ [pending/manual] S5.6. Manual LAN/phone validation remains user-run and must not be marked done without accepted evidence
│  └─ [pending/manual] S5.7. Acceptance boundary remains Linux Web only until separate real-platform evidence bundles are accepted
│
└─ LF-I. Completion gate before real-device phase
   ├─ [done] LF-I1. `python3 scripts/mobile_ios_flow_simulator.py --dry-run` contract evidence is covered by Linux cargo tests; this is not real iOS execution
   ├─ [done] LF-I2. `python3 scripts/mobile_ios_flow_simulator.py --use-loopback-runner --json` passes with targeted loopback cargo tests
   ├─ [done] LF-I3. `python3 scripts/mobile_linux_validation.py`
   ├─ [done] LF-I4. `cargo test -p deepseek-mobile-agent-core -p kai-runner`
   ├─ [done] LF-I5. `cargo clippy -p deepseek-mobile-agent-core -p kai-runner --all-targets --all-features`
   ├─ [done] LF-I6. Evidence draft generated from the Linux simulator bundle
   └─ [done] LF-I7. `docs/mobile-porting-plan.md` Stage W and this task tree agree that remaining hardware-blocked work must close through platform evidence bundles
```

## Priority Execution Order

1. Complete LF-D first. This gives the team a repeatable Linux simulation of the
   phone-to-computer rescue flow.
2. Complete LF-E next. This turns simulation output into evidence that can be
   reviewed and backfilled into the plan.
3. Complete LF-F and LF-G hardening gaps. These reduce risk before a real phone
   sends commands to a real computer.
4. Complete LF-H and LF-I. These define the exact handoff point from Linux-only
   confidence to hardware-backed validation.

## Linux Completion Definition

The Linux-first track is complete when all LF-D, LF-E, LF-F, LF-G, LF-H, and
LF-I open items are closed and these commands pass:

```bash
python3 scripts/mobile_ios_flow_simulator.py --dry-run
python3 scripts/mobile_ios_flow_simulator.py --use-loopback-runner
python3 scripts/mobile_linux_validation.py
cargo fmt --all --check
cargo test -p deepseek-mobile-agent-core -p kai-runner
cargo test -p deepseek-mobile-web-server
cargo clippy -p deepseek-mobile-agent-core -p kai-runner --all-targets --all-features
cargo clippy -p deepseek-mobile-web-server --all-targets --all-features
cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm test
cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm run typecheck
cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm run build
python3 scripts/mobile_web_ai_chat_smoke_test.py
git diff --check -- docs ios/DeepSeekMobileDemo crates/kai-runner crates/mobile-agent-core crates/mobile-web-server mobile-web scripts Makefile Cargo.toml Cargo.lock
```

After this gate passes, remaining claims must move to real macOS, iOS
simulator/device, LAN runner hardware, and Windows evidence bundles.
