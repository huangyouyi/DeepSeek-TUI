# Mobile Porting Plan

This document evaluates whether DeepSeek TUI can become a phone-side Agent Core
for computer rescue workflows. It is intentionally implementation-oriented: the
goal is not to move the terminal UI to iOS, but to split planning, approval,
state, and tool dispatch from local desktop execution.

## Executive Summary

DeepSeek TUI is a useful starting point, but not as a direct iOS port. The live
runtime is still centered in `crates/tui`, where agent turns, tool execution,
local shell, filesystem, MCP stdio, LSP, task state, and terminal UI concerns are
intertwined. The newer workspace crates already define useful seams:
`crates/protocol`, `crates/tools`, `crates/execpolicy`, `crates/state`,
`crates/agent`, and `crates/app-server`.

Recommended strategy:

1. Create a new `mobile-agent-core` crate that owns the phone-side agent loop,
   session state, event stream, approval gate, model client trait, persistence
   trait, and remote tool dispatcher.
2. Keep all local shell, file mutation, PTY, local LSP, desktop sandboxing, and
   MCP stdio spawning out of iOS.
3. Define remote tools as the only executable tool surface for phone workflows.
4. Support capability escalation from manual bootstrap, to SSH/PowerShell, to a
   lightweight `kai-runner`, to remote MCP/browser automation.

Final fit:

| Role | Fit | Reason |
|---|---:|---|
| DeepSeek TUI as phone-side Agent Core | Medium | Good protocol, approval, session, and tool ideas, but the current live agent loop is still too desktop-bound. |
| DeepSeek TUI as computer-side runner | Medium-High | Existing shell/file/git/MCP/runtime API code is useful, but must be slimmed and hardened into runner mode. |
| DeepSeek TUI as architecture reference | High | The event model, approval flow, tool registry, and session persistence are directly relevant. |

Recommendation: continue with a one-week technical spike. The spike should not
attempt a full product. It should prove that an iOS demo can run the agent loop,
persist a session, request approval, and execute one remote diagnostic command
through bootstrap text and SSH transport.

## Original Product Requirements

The product is a phone-side AI Agent that helps users repair a computer when the
computer is not ready to run a full agent.

Core value:

- The phone remains lightweight, reliable, and independent.
- The phone performs conversation, planning, approval, risk assessment, state,
  and tool dispatch.
- The computer starts from zero or degraded capability and gradually upgrades.
- Tool execution happens on the target computer, not locally on the phone.
- LLM inference happens through a cloud API, not a local mobile model.
- The core workflow is text, commands, logs, and structured tool results.
- RDP, VNC, screenshots, and video streams are not the core control plane.

The lowest fallback must work when the computer has no working agent, no Node,
no Bun, no Go, no Rust, no Python, no package manager, and no remote connection.
In that case the phone Agent gives the user one or two commands, receives pasted
or OCR text output, and iterates until the computer can enable SSH or install a
runner.

## Current Architecture

Current high-level structure:

```text
deepseek CLI dispatcher
  ├─ crates/cli
  │    └─ starts deepseek-tui, app-server, mcp-server, doctor, config commands
  │
  ├─ crates/tui
  │    ├─ crates/tui/src/main.rs
  │    ├─ core/engine.rs, core/turn.rs, core/session.rs, core/events.rs
  │    │    └─ live interactive agent loop, turns, cancellation, event emission
  │    ├─ client.rs, llm_client/
  │    │    └─ DeepSeek/OpenAI-compatible streaming chat completions
  │    ├─ tools/
  │    │    └─ shell, file, git, browser/web, plan, task, subagent, RLM, OCR
  │    ├─ mcp.rs, mcp_server.rs
  │    │    └─ local MCP client/server, stdio spawn, HTTP MCP support
  │    ├─ lsp/
  │    │    └─ stdio LSP process pool after file edits
  │    ├─ sandbox/
  │    │    └─ macOS Seatbelt, Linux Landlock, Windows helper contracts
  │    ├─ runtime_api.rs, runtime_threads.rs, task_manager.rs
  │    │    └─ local HTTP/SSE runtime API and durable task timeline
  │    └─ tui/
  │         └─ ratatui UI, approval modal, command palette, history, rendering
  │
  ├─ crates/protocol
  │    └─ ThreadRequest/Response, ToolPayload, ToolOutput, EventFrame,
  │       approval events, MCP startup events
  │
  ├─ crates/tools
  │    └─ generic ToolRegistry, ToolHandler, ToolCall, ToolSpec, ToolResult
  │
  ├─ crates/execpolicy
  │    └─ command approval policy, trusted/denied prefixes, approval decisions
  │
  ├─ crates/state
  │    └─ SQLite thread/session/message/checkpoint/job persistence
  │
  ├─ crates/core
  │    └─ newer runtime boundary over protocol/state/tools; not yet the full
  │       live TUI engine source of truth
  │
  ├─ crates/app-server
  │    └─ HTTP/JSON-RPC wrapper around crates/core Runtime
  │
  ├─ crates/mcp
  │    └─ lightweight MCP lifecycle compatibility types
  │
  └─ crates/secrets, crates/config, crates/hooks, crates/agent, crates/tui-core
       └─ support crates for config, secrets, hooks, model registry, UI state
```

Important current reality:

- `docs/ARCHITECTURE.md` says `crates/tui` remains the live end-user runtime.
- `crates/core` has useful thread, state, job, tool, approval, and event
  boundaries, but `handle_prompt` is still a scaffold-like runtime response and
  does not contain the full streaming LLM/tool loop from `crates/tui/src/core`.
- `crates/tui/src/tools/spec.rs` shows why direct iOS porting is unsafe: the
  tool context includes workspace paths, shell manager, sandbox backend,
  network policy, memory file path, local LSP manager, and large-output routing.
- `crates/tui/src/tools/shell.rs` uses local process spawning, PTY support,
  process groups, stdin, cancellation, sandboxing, and platform-specific code.
- `crates/tui/src/lsp` lazily spawns local language servers over stdio.
- `crates/tui/src/mcp.rs` supports MCP stdio process spawning and HTTP MCP.
  HTTP MCP is relevant to runner mode; stdio spawn is not phone-portable.

## iOS Portability Classification

| Module/File Area | Class | Reason |
|---|---|---|
| `crates/protocol/src/lib.rs` | Portable | Pure serde protocol types. Needs mobile-specific remote tool payloads and event variants. |
| `crates/tools/src/lib.rs` | Portable | Generic async registry and tool result abstractions. Needs no local executor assumptions. |
| `crates/execpolicy/src/lib.rs` | Needs Adapter | Good command approval core, but risk model must expand beyond shell prefixes to remote action classes and OS-specific hazards. |
| `crates/agent` | Portable | Model/provider registry is small and config-driven. |
| `crates/config` | Needs Adapter | Uses desktop paths and config files. iOS should inject config from Swift/Keychain/app storage. |
| `crates/secrets` | Needs Adapter | Desktop keyring backends are not enough. iOS must use Keychain via Swift or iOS-specific Rust binding. |
| `crates/state` | Needs Adapter | SQLite schema is relevant, but path discovery via `dirs` and desktop session fields need mobile storage abstraction. |
| `crates/hooks` | Needs Adapter | Hook model is useful for audit/event sinks, but webhook/stdout hooks are not phone-core defaults. |
| `crates/core` | Needs Adapter | Good thread/state/tool shell, but not yet the complete live agent loop. Also depends on config, MCP manager, state, hooks. |
| `crates/app-server` | Desktop/Runner Only | Phone should consume remote transports, not host a local Axum server as the core integration surface. Useful for runner prototype. |
| `crates/mcp` | Needs Adapter | HTTP/SSE MCP concepts are relevant. Stdio MCP spawning is desktop/runner only. |
| `crates/tui/src/core/engine.rs` | Needs Adapter | Contains the real agent loop but depends on TUI app modes, local tool context, local snapshots, LSP, subagents, shell manager, workspace, and tokio process ecosystem. |
| `crates/tui/src/core/session.rs`, `core/turn.rs`, `core/events.rs` | Needs Adapter | Turn/session ideas are portable, but filesystem snapshot and TUI event assumptions need removal. |
| `crates/tui/src/client.rs`, `llm_client/`, `models.rs` | Needs Adapter | Cloud model client is needed. Must remove blocking APIs and desktop config assumptions; expose Swift-friendly async boundary. |
| `crates/tui/src/tools/plan.rs`, `todo.rs`, `handle.rs` | Portable/Needs Adapter | Planning/checklist/handle ideas are useful. Must detach from TUI-specific state and large-output store paths. |
| `crates/tui/src/tools/shell.rs` | Desktop Only | Uses local child process spawn, PTY, stdin, process groups, sandbox hints. Must become runner-side only. |
| `crates/tui/src/tools/file.rs`, `apply_patch.rs`, `git.rs` | Desktop/Runner Only | They operate on local desktop filesystem and git state. Phone should call `remote.file.*` and runner tools. |
| `crates/tui/src/lsp/*` | Desktop/Runner Only | Requires local LSP server binaries and stdio transports. |
| `crates/tui/src/sandbox/*` | Desktop/Runner Only | macOS/Linux/Windows sandbox primitives are target-computer concerns. |
| `crates/tui/src/mcp.rs`, `mcp_server.rs` | Needs Adapter/Desktop Only | HTTP MCP can inform remote transport. MCP stdio server/client process spawning remains runner-side. |
| `crates/tui/src/runtime_api.rs`, `runtime_threads.rs`, `task_manager.rs` | Needs Adapter | Runtime event/timeline model is useful. Local server and desktop task execution are not iOS core. |
| `crates/tui/src/tui/*` | Desktop Only | ratatui, keyboard, clipboard, terminal rendering, approval modal. iOS needs native Swift UI. |
| `crates/tui/src/repl/*`, `rlm/*`, `subagent/*` | Desktop/Cloud Only for MVP | Python REPL/RLM/subagents are heavy and not needed for the first phone rescue core. Revisit after core split. |
| `crates/cli` | Desktop Only | CLI dispatcher and install shape. Not part of iOS app. |

## Recommended Target Architecture

```text
iOS App (Swift/SwiftUI)
  ├─ chat, command cards, approval sheets, connection setup, OCR paste/import
  ├─ Keychain credentials, SQLite app storage, Network.framework/URLSession
  └─ Rust FFI boundary
        │
        ▼
mobile-agent-core (Rust)
  ├─ Agent loop
  ├─ session state + message history
  ├─ planner/checklist state
  ├─ model client trait
  ├─ approval gate + risk classifier
  ├─ remote tool dispatcher
  ├─ event stream model
  └─ persistence abstraction
        │
        ├──────────────► Cloud model API
        │
        ▼
Remote Tool Transport trait
  ├─ ManualBootstrapTransport
  ├─ SshTransport
  ├─ PowerShellTransport
  ├─ RunnerWebSocketTransport
  └─ RemoteMcpTransport
        │
        ▼
Target computer
  ├─ no agent: user manually runs commands and returns text/OCR output
  ├─ SSH/PowerShell: bounded command execution and log collection
  ├─ kai-runner: tools, browser automation, files, logs, capability discovery
  └─ remote MCP servers: optional higher-level tools exposed by runner
```

Layer responsibilities:

- iOS App: native UI, connection onboarding, approval UX, QR/paste/OCR import,
  local notifications, Keychain, SQLite location, and background-resume UX.
- `mobile-agent-core`: deterministic agent state machine. It must not assume a
  shell, child process, terminal, PTY, desktop filesystem, or local MCP server.
- Model client: cloud chat completions streaming with DeepSeek V4 thinking
  support. It must preserve reasoning content when tool calls require it.
- Approval gate: evaluates every proposed remote tool call before execution.
  Produces a structured approval request for Swift UI.
- Remote tool transport: turns approved tool calls into manual instructions,
  SSH commands, PowerShell commands, runner RPC calls, or remote MCP calls.
- Target computer: executes tools at the currently available capability level.

## mobile-agent-core Boundary

`mobile-agent-core` should include:

- Agent loop and turn state.
- Session state and message history.
- Planner/checklist state.
- Tool call parser and dispatcher.
- Approval gate.
- Remote tool schema and capability registry.
- Event stream model for text, reasoning, tool start, tool delta, tool result,
  approval request, connection status, and errors.
- Persistence abstraction, with an iOS SQLite implementation outside the core
  crate or behind a feature.
- Model client trait, with a reqwest implementation that can be replaced by a
  Swift URLSession bridge if iOS build constraints require it.
- Bootstrap transcript model: generated command, user execution status,
  pasted/OCR output, and next-step recommendation.

`mobile-agent-core` should not include:

- Local shell execution.
- Local file editing.
- PTY.
- Terminal TUI.
- Local LSP.
- Local MCP stdio server spawning.
- Desktop sandbox implementation.
- Desktop clipboard, keyboard, terminal rendering, or OS-specific process
  management.
- Git snapshots of the phone filesystem.

## Phone-to-Computer Control Model

The phone controls the computer through a progressive capability model:

```text
Level 0: Manual bootstrap
  user copies one command from phone to computer
  user pastes text output or OCRs terminal output back to phone

Level 1: Remote shell
  phone connects over SSH, WinRM, or PowerShell Remoting
  low-risk diagnostics can run automatically
  medium/high-risk commands require approval

Level 2: Lightweight runner
  phone installs or guides installation of kai-runner
  runner exposes structured tools and capabilities
  runner does not run local LLM inference

Level 3: Enhanced tools
  runner exposes browser automation, installer logs, file operations,
  remote MCP calls, package repair, and system diagnostics
```

The agent must always know the active capability level and choose only available
tools. If a tool is unavailable, the agent should generate a bootstrap step that
moves the user toward the next level.

## Execution Modes

### Mode 1: Bootstrap Manual Guidance

Use when the computer cannot be reached remotely.

Flow:

1. User describes problem on phone.
2. Agent creates a short diagnostic plan.
3. Agent emits one safe command or manual step.
4. User runs it on the computer.
5. User pastes output or uses OCR to return text.
6. Agent parses the output and proposes the next command.
7. Repeat until SSH, runner, or target software can be installed.

Required product behavior:

- One command per step by default.
- Commands must include OS and shell assumptions.
- Dangerous fixes must be explained and approved even in manual mode.
- The app stores manual command/output pairs as audit log entries.

### Mode 2: SSH / PowerShell Remote Execution

Use when the computer accepts remote commands.

Flow:

1. User creates or selects a computer connection.
2. Agent discovers OS, shell, package manager, network state, and permissions.
3. Agent runs low-risk commands automatically.
4. Agent asks approval for medium/high-risk commands.
5. Output streams back as structured events.
6. Agent decides next step based on output.

Notes:

- macOS/Linux primary transport: SSH.
- Windows first transport should be PowerShell over SSH when available, then
  WinRM/PowerShell Remoting later if product requirements justify it.
- Every command result should carry command, cwd, exit code, stdout, stderr,
  duration, truncation metadata, and idempotency key.

### Mode 3: Lightweight Runner Enhanced Mode

Use after the computer is stable enough to install `kai-runner`.

Runner responsibilities:

- Execute tools.
- Collect logs.
- Read/write files with policy.
- Run browser automation.
- Report capabilities.
- Proxy remote MCP tools.
- Stream structured events.

Runner non-goals:

- It does not run a local LLM.
- It does not make autonomous decisions.
- It does not bypass phone-side approval policy.

### Mode 4: Complex Software Installation and Browser Assist

Use when installation requires web downloads, login consoles, forms, browser
clicks, or installer pages.

Behavior:

- Without runner: phone gives manual browser instructions and asks the user to
  paste visible text, error text, downloaded filename, or OCR result.
- With runner: phone calls `remote.browser.*` tools.
- Browser tools return DOM text, URL, selected element summaries, network
  status, download status, and screenshots only as optional evidence, not as
  the core control surface.
- Login, submit, upload, payment, deletion, privilege elevation, or account
  change actions always require approval.

## User Journey

Example journey for "Homebrew install failed":

1. User opens iOS app and writes: "帮我检查 Mac 上 Homebrew 为什么安装失败".
2. Agent asks whether the Mac is reachable by SSH. If not, it enters bootstrap.
3. Bootstrap command 1: `uname -a; sw_vers; command -v brew; xcode-select -p`.
4. User pastes output.
5. Agent detects missing CLT, broken PATH, proxy/DNS issue, or partial install.
6. If SSH is available, Agent connects and runs bounded diagnostics.
7. If repair requires install or sudo, Agent asks approval with risk summary.
8. Once base tools are healthy, Agent offers runner installation.
9. Runner reports capabilities; Agent switches from shell-only diagnostics to
   structured package/log/browser tools.
10. Session can be resumed later with complete command/output audit history.

## Remote Tool Interface Design

Risk levels:

- Low: read-only diagnostics, no credential exposure expected.
- Medium: writes user files, changes config, installs packages without admin,
  opens browser pages, or reveals sensitive logs.
- High: sudo/admin, deletes data, changes network/security settings, modifies
  shell profiles, installs services, submits forms, uploads files, payment,
  account changes.

### `remote.shell.exec`

Input:

```json
{
  "connection_id": "macbook-1",
  "command": "brew doctor",
  "cwd": "/Users/alice",
  "timeout_ms": 60000,
  "env": {"LANG": "en_US.UTF-8"},
  "stream": true,
  "idempotency_key": "diag-brew-doctor-1",
  "risk_hint": "low"
}
```

Output:

```json
{
  "status": "completed",
  "exit_code": 0,
  "stdout": "...",
  "stderr": "",
  "duration_ms": 1234,
  "stdout_truncated": false,
  "stderr_truncated": false,
  "runner": {"transport": "ssh", "os": "macos"}
}
```

Risk: Low to High, command-dependent. Approval: low-risk read-only commands may
auto-run; anything mutating, privileged, destructive, credential-related, or
network/security-changing requires approval.

### `remote.powershell.exec`

Input:

```json
{
  "connection_id": "win-laptop-1",
  "script": "Get-Command winget; $PSVersionTable.PSVersion",
  "working_directory": "C:\\Users\\Alice",
  "timeout_ms": 60000,
  "execution_policy": "bypass_not_allowed",
  "idempotency_key": "diag-winget-1"
}
```

Output:

```json
{
  "status": "completed",
  "exit_code": 0,
  "stdout": "...",
  "stderr": "",
  "objects_json": null,
  "duration_ms": 1200
}
```

Risk: Low to High. Approval: same as shell, with extra high-risk classification
for registry edits, UAC elevation, service changes, script downloads, and
execution policy changes.

### `remote.file.read`

Input:

```json
{
  "connection_id": "macbook-1",
  "path": "/tmp/install.log",
  "start_line": 1,
  "max_lines": 200,
  "encoding": "utf-8"
}
```

Output:

```json
{
  "path": "/tmp/install.log",
  "content": "...",
  "total_lines": 480,
  "shown_lines": "1-200",
  "truncated": true,
  "next_start_line": 201
}
```

Risk: Low to Medium. Approval: auto for known logs and user-approved paths;
approval required for secrets, browser profiles, SSH keys, keychains, private
documents, or broad directory reads.

### `remote.file.write`

Input:

```json
{
  "connection_id": "macbook-1",
  "path": "/Users/alice/.zshrc",
  "content": "export PATH=\"/opt/homebrew/bin:$PATH\"\n",
  "mode": "append",
  "backup": true,
  "idempotency_key": "fix-zshrc-path-1"
}
```

Output:

```json
{
  "status": "written",
  "path": "/Users/alice/.zshrc",
  "backup_path": "/Users/alice/.zshrc.kai-backup-20260523",
  "bytes_written": 36
}
```

Risk: Medium to High. Approval: always required for MVP.

### `remote.diagnose.system`

Input:

```json
{
  "connection_id": "macbook-1",
  "profile": "macos_dev_environment",
  "include_network": true,
  "include_package_managers": true,
  "include_permissions": true
}
```

Output:

```json
{
  "os": {"name": "macos", "version": "15.5", "arch": "arm64"},
  "shells": [{"name": "zsh", "path": "/bin/zsh"}],
  "package_managers": [{"name": "brew", "present": false, "detail": "not found"}],
  "developer_tools": {"xcode_select": "missing"},
  "network": {"dns_ok": true, "https_ok": false},
  "findings": [{"severity": "error", "code": "xcode_clt_missing", "message": "..."}]
}
```

Risk: Low. Approval: auto after the user authorizes the connection.

### `remote.package.install`

Input:

```json
{
  "connection_id": "macbook-1",
  "manager": "brew",
  "package": "git",
  "version": null,
  "allow_sudo": false,
  "dry_run": true,
  "idempotency_key": "install-git-brew-1"
}
```

Output:

```json
{
  "status": "planned",
  "commands": ["brew install git"],
  "requires_sudo": false,
  "log": "...",
  "next_approval_required": true
}
```

Risk: Medium to High. Approval: always required before actual install; dry-run
can be auto.

### `remote.browser.open`

Input:

```json
{
  "connection_id": "macbook-1",
  "url": "https://brew.sh",
  "profile": "default",
  "headless": false
}
```

Output:

```json
{
  "status": "opened",
  "page_id": "page-123",
  "url": "https://brew.sh/",
  "title": "Homebrew"
}
```

Risk: Medium. Approval: required for opening external sites in MVP unless the
user explicitly requested the site.

### `remote.browser.extract_text`

Input:

```json
{
  "connection_id": "macbook-1",
  "page_id": "page-123",
  "selector": "body",
  "max_chars": 8000
}
```

Output:

```json
{
  "url": "https://brew.sh/",
  "title": "Homebrew",
  "text": "...",
  "links": [{"text": "Installation", "href": "https://brew.sh/#install"}],
  "truncated": false
}
```

Risk: Low to Medium. Approval: auto for current page text; approval required for
sensitive pages such as email, banking, admin consoles, or private dashboards.

### `remote.browser.click`

Input:

```json
{
  "connection_id": "macbook-1",
  "page_id": "page-123",
  "selector": "button.download",
  "intent": "download installer",
  "requires_submit": false
}
```

Output:

```json
{
  "status": "clicked",
  "url_before": "...",
  "url_after": "...",
  "download": {"started": true, "filename": "installer.pkg"}
}
```

Risk: Medium to High. Approval: always required for form submit, login, upload,
payment, deletion, account changes, or anything that leaves the current site.

### `remote.bootstrap.guide`

Input:

```json
{
  "target_os": "macos",
  "shell": "zsh",
  "goal": "diagnose Homebrew install failure",
  "previous_output": null,
  "constraints": {"one_command": true, "no_sudo_without_approval": true}
}
```

Output:

```json
{
  "instruction": "Open Terminal on the Mac and run this command:",
  "command": "uname -a; sw_vers; command -v brew; xcode-select -p",
  "expected_output": "OS version, brew path if installed, and Xcode CLT path",
  "risk": "low",
  "approval_required": false,
  "how_to_return_output": "Paste the terminal output into the phone app."
}
```

Risk: Low to High. Approval: same risk rules as shell, but the user executes
manually.

### `remote.mcp.call`

Input:

```json
{
  "connection_id": "macbook-1",
  "server": "filesystem",
  "tool": "read_text_file",
  "arguments": {"path": "/tmp/install.log"},
  "timeout_ms": 60000
}
```

Output:

```json
{
  "status": "completed",
  "result": {"content": "..."},
  "server": "filesystem",
  "tool": "read_text_file"
}
```

Risk: Server/tool-dependent. Approval: use runner-reported metadata where
available, but default to approval-required for mutating or unknown MCP tools.

## Communication Options

| Transport | Best Use | Pros | Cons |
|---|---|---|---|
| Manual bootstrap | Computer unreachable or broken | Always available, no install required, works offline except model API | Slow, user error prone, no streaming, output copy/OCR quality varies |
| SSH | macOS/Linux and Windows with OpenSSH | Standard, debuggable, no custom runner first, works early | Credential UX, firewall/NAT, command quoting, limited browser/file semantics |
| PowerShell Remoting / WinRM | Managed Windows environments | Native Windows management semantics | Harder consumer setup, UAC/policy complexity, certificate/trust issues |
| HTTPS/WebSocket runner | Stable enhanced mode | Structured tools, streaming, capabilities, browser automation, logs, upgrades | Requires installation, trust boundary, update/security burden |
| MCP over HTTP/SSE | Tool ecosystem interop | Standardizing, good for tool discovery and structured calls | MCP security metadata is uneven; stdio MCP is not phone-portable |

Recommended sequence:

1. MVP: manual bootstrap + SSH.
2. Early beta: PowerShell over SSH + runner WebSocket.
3. Later beta: remote MCP over HTTP/SSE behind runner capability discovery.
4. Avoid exposing raw MCP stdio to the phone. Let runner own local MCP stdio
   process spawning on the computer.

## iOS Integration

Options:

| Option | Assessment |
|---|---|
| Rust `staticlib` + Swift FFI | Lowest-level and controllable, but high manual binding cost. Good only if API surface stays tiny. |
| UniFFI | Recommended for MVP and beta. Generates Swift bindings, handles async-ish object APIs more cleanly, keeps Rust core testable. |
| Flutter/React Native native module | Useful if the whole app is cross-platform. Adds framework overhead and still needs Rust binding layer. |

Recommendation:

- Use UniFFI for `mobile-agent-core`.
- Keep the Swift-facing API small:
  - `create_session(config) -> SessionHandle`
  - `send_user_message(session_id, text) -> stream/events`
  - `submit_approval(approval_id, decision)`
  - `submit_bootstrap_output(step_id, text)`
  - `connect_target(connection_config)`
  - `list_sessions()`, `resume_session(id)`
- Store model API keys and computer credentials in iOS Keychain.
- Store session database in app container SQLite.
- Treat network permission failures and background suspension as normal events.
- Avoid hosting HTTP servers inside the iOS app. iOS should be a client.

## Product Capability Mapping

| Capability | Current Similar Capability | Reuse | New Module Needed | iOS Core? | Runner? | MVP? |
|---|---|---|---|---:|---:|---:|
| User intent understanding | TUI agent loop in `crates/tui/src/core/engine.rs` | Prompt/model loop patterns | `mobile-agent-core::agent_loop` | Yes | No | Yes |
| Diagnostic plan generation | `tools/plan.rs`, `AppMode::Plan` | Planner/checklist concepts | Mobile planner prompt + plan state | Yes | No | Yes |
| Bootstrap manual guidance | None as first-class mode | Approval/event concepts | `BootstrapTransport`, bootstrap transcript | Yes | No | Yes |
| SSH remote execution | Local `exec_shell` only | Shell result schema ideas | `SshTransport`, remote shell schema | Yes dispatch only | Yes execution | Yes |
| PowerShell remote execution | Windows shell handling in local shell | Quoting lessons | `PowerShellTransport` | Yes dispatch only | Yes execution | No for first spike |
| Remote file read | `read_file` | Bounded output, truncation | `remote.file.read` | Yes schema | Yes execution | No for first spike |
| Installer log collection | `read_file`, diagnostics | Truncation, summaries | Known log locator tools | Yes orchestration | Yes execution | Phase 2 |
| System diagnosis | `diagnostics`, `doctor --json` | Health/capability patterns | `remote.diagnose.system` | Yes schema | Yes execution | Yes minimal |
| Package manager repair | Shell + docs | Command policy ideas | Package repair playbooks | Yes planning/approval | Yes execution | Phase 2 |
| Browser automation | `web_run`, fetch/search, OCR | Some schemas and event ideas | `remote.browser.*` | Yes schema/approval | Yes execution | Phase 3 |
| Runner install/upgrade | Install scripts/docs | CLI install knowledge | Runner bootstrap installer | Yes guidance | Yes self-update | Phase 3 |
| Capability discovery | MCP manager, doctor JSON | Capability endpoint ideas | `remote.capabilities` | Yes | Yes | Yes |
| Tool risk assessment | `crates/execpolicy`, `command_safety.rs` | Prefix policy, approval decisions | Remote risk classifier | Yes | No final decision | Yes |
| User approval | TUI approval modal, protocol approval events | `ExecApprovalRequestEvent`, `ReviewDecision` | Swift approval UI + mobile gate | Yes | Enforces nonce only | Yes |
| Audit log | `audit.rs`, hooks, runtime timelines | Event/audit patterns | Mobile audit store | Yes | Yes emits | Yes |
| Rollback suggestions | snapshots/revert_turn | Concept only | Remote backup/rollback metadata | Yes planning | Yes backups | Phase 2 |
| Session recovery | `session_manager`, `crates/state` | SQLite/message schema | Mobile persistence adapter | Yes | No | Yes |

## Scenario Coverage

| Scenario | Fit With Proposed Architecture | MVP Handling |
|---|---|---|
| macOS Homebrew install failed | High | Bootstrap or SSH runs `sw_vers`, `uname`, `command -v brew`, `brew doctor`, `xcode-select -p`, DNS/HTTPS checks. |
| macOS Xcode Command Line Tools missing | High | Detect with `xcode-select -p`, guide `xcode-select --install`, explain GUI installer. |
| macOS certificates/proxy/DNS/permissions | Medium-High | Diagnostics can inspect `scutil --dns`, `networksetup`, curl TLS output; fixes require approval. |
| Windows Python/Node/VS Code install failed | Medium | Bootstrap PowerShell commands first; runner later improves installer log discovery. |
| Windows winget/PowerShell/PATH/UAC | Medium | PowerShell transport needed after MVP spike; bootstrap can still guide commands. |
| Windows Event Log/install logs | Medium | Runner should implement structured event log query; bootstrap can guide `Get-WinEvent`. |
| User cannot read CLI errors | High | Agent summarizes pasted/OCR output and explains next step in plain language. |
| Computer offline | Medium | Phone can provide offline repair steps from model context, but cloud model requires phone network. |
| Install stronger runner | High | Bootstrap and SSH modes should converge on runner install once system is stable. |

## Fit Assessment for Our Product

DeepSeek TUI is suitable as a reference and partial code donor, not as a direct
mobile core.

Answers to the key product questions:

- Is it suitable as phone-side Agent Core? Medium. The abstractions exist, but
  the complete live loop remains in `crates/tui` and must be extracted.
- Are local shell/file/git/LSP tools easy to replace with remote tools? Medium.
  The tool registry makes replacement conceptually clean, but current TUI tool
  context carries many local assumptions.
- Can skills/MCP/approval/session mechanisms serve remote computer rescue?
  Yes, with adapters. Approval and session are strong; MCP stdio must move to
  runner; skills should become guidance/playbooks first, executable plugins
  later.
- Does it support bootstrap -> SSH -> runner upgrade today? No as a product
  flow. It has pieces: runtime API, MCP, shell tools, and approval. The explicit
  capability ladder must be added.
- Can it separate computer execution from phone planning? Not yet. This is the
  central refactor: all executable tools must go through a transport trait and
  remote schema when running in mobile mode.
- Biggest modification: extract a real agent loop from `crates/tui/src/core`
  into a platform-neutral core while replacing `ToolContext` local execution
  with `RemoteToolTransport`.

## Current Implementation Status

This task tree is no longer only a proposal. The repository now contains
scaffolds for the phone core, iOS demo shell, and runner path, but the current
state is still a Linux-verified scaffold rather than a proven macOS/iOS product.

Completed or scaffolded in this branch:

- `crates/mobile-agent-core` exists with session, event, approval, risk,
  audit, bootstrap, capability, connection, model, persistence, SSH, remote
  schema, transport, and `uniffi_api` modules.
- Phase 3 core shape is represented by `src/session.rs`,
  `src/agent_loop.rs`, `src/model.rs`, `src/persistence.rs`,
  `src/transport.rs`, `src/ssh.rs`, `src/capabilities.rs`, and
  `src/uniffi_api.rs`, with deterministic tests under
  `crates/mobile-agent-core/tests/`.
- Bootstrap and rescue flows now include Homebrew/Xcode CLT diagnostics,
  macOS runner install guidance, Windows runner install guidance, winget/App
  Installer/PATH/execution-policy fallback text, and bootstrap-to-runner
  upgrade tests.
- Runner HTTP/WebSocket-facing transport scaffolds exist in
  `crates/mobile-agent-core/src/transport.rs`, including capability discovery,
  auth metadata preparation, remote MCP call request shaping, and maintenance
  plan request/response parsing.
- `crates/kai-runner` exists with capability reporting, runner API handlers,
  bearer-token auth, pairing code redemption, approval nonce issuance and
  redemption, dry-run maintenance planning, MCP call dispatch, workspace file
  read/write helpers with path escape checks and backups, browser session
  metadata scaffolding, and shell/powershell execution scaffolding behind
  explicit approval nonce policy.
- Runner shell execution is intentionally not enabled through the default
  `KaiRunner` scaffold; the default handler returns a blocked result for
  `remote.shell.exec` and `remote.powershell.exec`. Shell execution exists only
  through `ShellEnabledKaiRunner<E>` and requires an approval nonce.
- Browser support is scaffolded as `remote.browser.open`,
  `remote.browser.extract_text`, and `remote.browser.click` plus browser
  session state. Without an injected `BrowserEngine`, extract/click remain
  preview/approval flows and do not drive a real browser.
- Maintenance support is dry-run and audit-oriented. `self_update` and
  `uninstall` produce preflight, maintenance, rollback, and safety guidance,
  but execution is deliberately disabled in the scaffold even after nonce
  consumption.
- `ios/DeepSeekMobileDemo` exists as a Swift Package demo shell with SwiftUI
  session/chat/command/approval/connection/bootstrap/audit views, mock
  runner/remote MCP pairing, mock capability discovery, redacted shell and
  browser approval nonce presentation, `KeychainCredentialStore`, and
  `SQLiteStoreAdapter`.
- Audit timeline work is represented in `crates/mobile-agent-core/src/audit.rs`,
  transport maintenance audit parsing, iOS `AuditLogView.swift`, and XCTest
  scaffolds such as `AuditTimelinePresentationTests.swift` and
  `RunnerRequestAuthMetadataTests.swift`.
- Recent P/Q scaffold work is represented by live local runner pairing smoke
  tests in `crates/kai-runner/tests/runner_pairing_live_auth.rs` and
  `crates/kai-runner/tests/runner_mobile_core_live_e2e.rs`, maintenance
  approval request tests in
  `crates/mobile-agent-core/tests/maintenance_approval_flow.rs`, browser
  adapter seams around `BrowserEngine` and `BrowserSessionRegistry` in
  `crates/kai-runner/src/lib.rs`, shell approval nonce policy through
  `ShellEnabledKaiRunner<E>` and `ShellApprovalNonceManager`, PowerShell runner
  scaffolding in `crates/kai-runner/tests/runner_windows_powershell.rs`, and
  iOS UniFFI handoff scripts under `ios/DeepSeekMobileDemo/Scripts/`.

Still not complete:

- No macOS/Xcode build has been proven from this Linux workspace. The Swift
  package and `Scripts/verify-macos` document the path, but real `swift test`,
  simulator launch, Xcode target integration, signing, and device/TestFlight
  validation remain macOS-only work.
- The iOS demo still uses a mock bridge. It is not linked to the generated Rust
  UniFFI bindings, does not stream a real cloud model from the phone, and does
  not call a live runner.
- Real runner installation/upgrade has not been executed. Bootstrap scripts
  refer to release artifacts, but artifact production, signing/notarization on
  macOS, Windows packaging, and upgrade/uninstall execution are not proven.
- The runner has HTTP route scaffolds and direct handler tests, but no shipped
  standalone runner binary entrypoint is documented here as complete, and no
  long-lived TLS/WebSocket service lifecycle has been validated.
- Real browser automation is not implemented. The code models browser sessions,
  extract requests, and click approvals; it does not yet bind Playwright,
  WebDriver, Safari automation, Chrome DevTools, or a hardened profile model.
- The S-stage validation split is documentation-only: real Mac/Xcode builds,
  real UniFFI generation/linking, iOS simulator/device runs, LAN runner pairing,
  a real browser engine behind `BrowserEngine`, and real Windows
  PowerShell/runner host tests remain open until the platform checklists in
  `docs/mobile-validation-checklists.md` are executed and evidence is recorded.
- The remote shell security boundary is incomplete. Approval nonces, path
  checks, timeouts, and policy hooks exist, but there is no production-grade
  OS sandbox, per-command capability lease, cwd allowlist policy, secret
  redaction boundary, sudo/UAC mediation, or replay-resistant protocol envelope
  proven against a real networked runner.
- Remote MCP is a proxy shape, not a hardened trust boundary. Unknown mutating
  MCP tools, server identity, schema attestation, and prompt-injection handling
  still need policy and tests against real MCP servers.
- App Store risk remains open: remote computer management, runner pairing,
  local network access, credential handling, background behavior, and browser
  automation all need product copy, privacy disclosures, review notes, and
  possibly enterprise/TestFlight-first distribution.

Linux-verifiable today:

- Rust crate compilation and unit/integration tests for
  `crates/mobile-agent-core` and `crates/kai-runner`.
- JSON schema, auth metadata, pairing, nonce, maintenance dry-run, fake runner,
  fake SSH, bootstrap, recovery, persistence, file path, and browser preview
  behavior.
- Static review of Swift sources and README/XCTest scaffolds.

Not Linux-verifiable today:

- Xcode project/package resolution, Swift XCTest execution, simulator/device UI,
  Keychain behavior, SQLite adapter behavior inside an iOS sandbox, local
  network permission prompts, signing, notarization, App Store review behavior,
  and real macOS/Windows runner installation.

## MVP Refactor Plan

### Phase 1: Abstract Tool Execution

Goal: make tool dispatch independent from local shell/file execution.

Files to modify or create:

- Create `crates/mobile-agent-core/Cargo.toml`.
- Create `crates/mobile-agent-core/src/lib.rs`.
- Create `crates/mobile-agent-core/src/tool.rs`.
- Create `crates/mobile-agent-core/src/event.rs`.
- Create `crates/mobile-agent-core/src/approval.rs`.
- Extend `crates/protocol/src/lib.rs` with remote tool payload/event types, or
  create mobile-local types first and upstream later.
- Reuse `crates/tools/src/lib.rs` for generic registry concepts.
- Reuse `crates/execpolicy/src/lib.rs` as the first approval engine input.

Deliverable:

- A Rust unit test can submit a synthetic `remote.shell.exec` call.
- The core emits `approval_required` for risky commands.
- The same call can be approved and dispatched to a fake transport.

Phone-runnable milestone:

- iOS command-line or Swift unit-test harness links the Rust static library.
- User can type a prompt, receive a planned bootstrap command, and persist the
  session locally. No real SSH yet.

### Phase 2: Replace Local Tools With Remote Tools in Mobile Mode

Goal: mobile mode exposes only remote tools and bootstrap tools.

Files to modify or create:

- Create `crates/mobile-agent-core/src/remote_schema.rs`.
- Create `crates/mobile-agent-core/src/bootstrap.rs`.
- Create `crates/mobile-agent-core/src/risk.rs`.
- Add remote tool catalog equivalent to `crates/tui/src/tools/registry.rs`, but
  without local `ToolContext`.
- Keep `crates/tui/src/tools/shell.rs`, `file.rs`, `git.rs`, `lsp/*`,
  `sandbox/*`, and `tui/*` out of `mobile-agent-core`.

Deliverable:

- Mobile core can run a complete bootstrap transcript:
  prompt -> plan -> command card -> user output -> analysis -> next command.
- Audit log includes every generated command and returned output.

Phone-runnable milestone:

- iOS demo app screen with chat, command card, paste/OCR text input, and
  approval sheet.
- A real user can test Homebrew/Xcode CLT diagnosis manually on a Mac.

### Phase 3: Implement `mobile-agent-core` - Scaffold Complete, Product Wiring Open

Goal: a stable phone-side Rust crate with model streaming, persistence, and
transport interfaces.

Files to create:

- `crates/mobile-agent-core/src/session.rs`
- `crates/mobile-agent-core/src/agent_loop.rs`
- `crates/mobile-agent-core/src/model.rs`
- `crates/mobile-agent-core/src/persistence.rs`
- `crates/mobile-agent-core/src/transport/mod.rs`
- `crates/mobile-agent-core/src/transport/manual.rs`
- `crates/mobile-agent-core/src/transport/ssh.rs`
- `crates/mobile-agent-core/src/capabilities.rs`
- `crates/mobile-agent-core/src/uniffi.rs`

Files to reference or mine:

- `crates/tui/src/core/engine.rs` for loop shape, max steps, cancellation, tool
  call iteration, reasoning/text streaming.
- `crates/tui/src/core/turn.rs` for turn tracking.
- `crates/tui/src/core/events.rs` and `crates/protocol/src/lib.rs` for event
  schemas.
- `crates/state/src/lib.rs` for SQLite schema patterns.
- `crates/tui/src/client.rs`, `crates/tui/src/llm_client/*`, and
  `crates/tui/src/models.rs` for Chat Completions models and streaming.

Status:

- Scaffold complete in `crates/mobile-agent-core`.
- Covered by tests for agent loop, event stream, model/fake model, remote
  schema, approval risk/continue, bootstrap session, SSH command mapping,
  runner transport, runner HTTP transport, persistence, SQLite persistence,
  audit, recovery, Homebrew rescue, Windows rescue, bootstrap-to-runner, browser
  session, and maintenance audit flows.
- Real phone model streaming and generated Swift UniFFI integration are still
  open.

Original deliverable:

- Rust integration test with fake model and fake transport completes a
  multi-step diagnosis.
- Core survives network interruption by persisting pending turn state.

Phone-runnable milestone:

- iOS demo talks to the real cloud model.
- User can resume a previous bootstrap diagnosis after app restart.

### Phase 4: iOS Demo Shell - Mock Scaffold Complete, Xcode Validation Open

Goal: a usable native shell around the core.

Swift-side modules:

- `SessionListView`
- `ChatView`
- `CommandCardView`
- `ApprovalSheet`
- `ConnectionSetupView`
- `BootstrapOutputInputView`
- `AuditLogView`
- `KeychainCredentialStore`
- `SQLiteStoreAdapter` or Rust-managed SQLite wrapper

Status:

- Scaffold complete under `ios/DeepSeekMobileDemo` as a Swift Package demo.
- Implemented views include session list, chat, command cards, approval sheets,
  connection setup, bootstrap output input, audit log, mock credential storage,
  and SQLite adapter scaffolding.
- Mock bridge covers runner/remote MCP pairing, mock capability discovery,
  approval nonce labels, browser click approval state, maintenance approval
  dry-run state, bootstrap output import, and audit timeline presentation.
- Not yet linked to real Rust UniFFI bindings or a real runner; not built in
  Xcode from this Linux workspace.

Original deliverable:

- TestFlight/internal build can run manual bootstrap end to end.
- App handles background/foreground transitions without losing pending
  approval or pasted output.

Phone-runnable milestone:

- Real user tests on a Mac with broken Homebrew.
- The phone app visibly produces command cards and explanations.

### Phase 5: Connect `kai-runner` / Remote MCP - Runner Scaffold Complete, Real Deployment Open

Goal: upgrade from manual/SSH execution to structured runner tools.

Files/modules to create:

- Runner crate or binary: `crates/kai-runner`.
- Runner tool modules: shell, powershell, file, diagnose, package, browser,
  mcp_proxy, capabilities.
- Runner transport: HTTPS/WebSocket with auth token, pairing code, or local
  network pairing.
- Mobile transport: `RunnerWebSocketTransport`.

DeepSeek TUI code to reuse:

- `crates/tui/src/tools/shell.rs` result schema and background execution ideas.
- `crates/tui/src/tools/file.rs` truncation and path safety ideas.
- `crates/tui/src/mcp.rs` HTTP MCP concepts.
- `crates/tui/src/runtime_api.rs` and `runtime_threads.rs` event/timeline ideas.
- `crates/app-server` as a reference, not as the direct mobile protocol.

Status:

- `crates/kai-runner` exists with direct API and Axum route scaffolds in
  `src/lib.rs`, `src/server.rs`, and `src/pairing.rs`.
- Implemented/tested scaffold surfaces include `/health`, `/capabilities`,
  `/tool-call`, `/mcp/call`, `/pairing/code`, `/pairing/redeem`,
  `/approval/nonce`, `/maintenance/plan`, bearer-token auth, pairing tokens,
  approval nonce replay/expiry handling, dry-run maintenance planning, file
  read/write workspace confinement, browser open/extract/click preview paths,
  MCP call dispatch, and shell/powershell execution through an explicit
  `ShellEnabledKaiRunner<E>` path.
- Default runner still blocks raw shell execution. Maintenance execution and
  package install execution are intentionally disabled. Browser extraction
  needs a real `BrowserEngine`.

Phone-runnable milestone:

- iOS app connects to runner, discovers capabilities, runs
  `remote.diagnose.system`, and then executes `remote.shell.exec`.
- User can see live command output on the phone.

## Next Stage Recommendations

1. Execute the real-platform validation checklists in
   `docs/mobile-validation-checklists.md` before changing platform behavior.
   Capture `evidence-log.md` bundles for macOS, Windows, iOS simulator/device,
   and LAN runner runs, then update this plan with pass/fail status and exact
   blockers.
2. Prove the macOS toolchain path on real hardware: run the Swift package tests,
   open `ios/DeepSeekMobileDemo/Package.swift` in Xcode, launch the simulator,
   and record the exact missing project/signing/binding work.
3. Generate and link the real UniFFI bridge from
   `crates/mobile-agent-core/src/uniffi_api.rs` into the Swift demo, replacing
   mock-only session and audit data one surface at a time.
4. Add a real `kai-runner` binary/service entrypoint with explicit local bind
   address, TLS or local-network pairing posture, token storage, logs, lifecycle
   commands, and version reporting.
5. Execute runner install/upgrade on a sacrificial Mac and Windows VM. Keep
   bootstrap fallback as the recovery path and capture audit entries for failed
   installs as first-class outcomes.
6. Harden remote shell before broad use: define allowed cwd roots, command
   lease metadata, nonce binding to command hash/session/tool/cwd, output
   redaction, timeout/cancel semantics, sudo/UAC behavior, and OS sandbox
   expectations for macOS, Linux, and Windows.
7. Pick the first real browser engine behind `BrowserEngine` and keep
   `remote.browser.click` approval-bound. Do not expose profile cookies,
   downloads, or form submission until audit/redaction policy is tested.
8. Turn maintenance from dry-run plan to staged executor only after binary
   signing, rollback, idempotency, and local approval UX are proven.
9. Prepare App Store risk review early: local network use, remote control
   framing, credential storage, browser automation, background limits, privacy
   labels, and reviewer demo credentials.

## Post-Scaffold Execution Plan

### Stage 1: macOS/Xcode Proof

- Run `ios/DeepSeekMobileDemo/Scripts/verify-macos` on macOS with Xcode and
  Swift installed. The script should run `swift package describe`,
  `swift test`, `swift build`, and an `xcodebuild` simulator build when
  `xcodebuild` is available.
- Open `ios/DeepSeekMobileDemo/Package.swift` in Xcode and run the
  `DeepSeekMobileDemo` package tests if the command-line test run did not
  already cover them.
- Launch the demo in the simulator and verify session list, chat, command card,
  connection setup, approval sheet, bootstrap paste, maintenance approval,
  browser approval, and audit timeline screens.
- Record the macOS version, Xcode version, simulator destination/runtime,
  command output, compile errors, package layout gaps, signing requirements, and
  simulator/device-only failures in this plan before touching Rust behavior.
- Keep Linux verification limited to the Rust workspace and shell syntax/docs
  checks. Linux cannot prove SwiftUI, Keychain, SQLite sandbox behavior, Xcode
  package resolution, simulator launch, signing, local network permission, or
  physical-device behavior.

Exit criteria:

- A macOS log shows whether Swift package tests and simulator launch pass.
- The document separates Linux-known scaffold status from macOS/Xcode-known
  status.

S5 real-platform validation checklist pointer:

The executable macOS checklist is now split out in
`docs/mobile-validation-checklists.md`. It covers prerequisites, commands,
expected results, and failure logs for macOS environment capture, Swift package
describe/test/build, `verify-macos`, Xcode package opening, simulator build/UI
smoke, and macOS runner bootstrap smoke.

### Stage S5: Real Platform Validation Checklist Split - Complete

Scope: documentation-only decomposition of the manual validation work needed
before claiming real platform support.

Checklist families now defined in `docs/mobile-validation-checklists.md`:

- macOS host validation: Xcode/Swift package, simulator build/UI smoke, and
  macOS runner bootstrap evidence.
- Windows host validation: PowerShell diagnostics, PATH/winget/UAC/execution
  policy, runner install/offline fallback, Event Log, and PowerShell runner
  behavior.
- iOS simulator validation: simulator inventory, command-line build, app launch,
  mock workflow navigation, local network rehearsal, and UniFFI handoff.
- iOS device validation: signing, launch, Keychain, SQLite sandbox persistence,
  background/resume, local network permission, and Rust bridge device link.
- LAN runner validation: explicit bind, health, pairing redemption,
  capabilities, diagnose, auth rejection, approval-required shell path, and
  interruption recovery.

S5 exit status:

- Done: each checklist item names preconditions, command/action, expected
  result, and failure logs/evidence to collect.
- Done: validation evidence bundle expectations and failure triage rules are
  documented.
- Open: the checklists have not been executed on real macOS, Windows, iOS
  simulator/device, or LAN runner environments from this Linux workspace.
- Open: pass/fail evidence still needs to be fed back into this plan before any
  platform support claim changes.

Next S-stage open items:

1. S6: Run the macOS host checklist on real hardware and record the evidence
   bundle.
2. S7: Run iOS simulator and physical-device checklists, including signing,
   Keychain, SQLite sandbox, local network permission, and UniFFI link status.
3. S8: Run the LAN runner checklist against a disposable runner host and capture
   pairing/auth/diagnose evidence before enabling broader execution.
4. S9: Run the Windows host checklist on a disposable Windows VM, including
   PowerShell, UAC, PATH, execution policy, Event Log, and runner install paths.
5. S10: Update this plan with concrete platform results and convert verified
   blockers into implementation tasks.

### Stage T5: Evidence Log Template - Complete

Scope: documentation-only addition of fillable evidence logs for real platform
validation. This does not claim that any macOS, Windows, iOS simulator/device,
or LAN runner checklist has passed.

T5 exit status:

- Done: `docs/mobile-validation-checklists.md` now defines a shared
  `evidence-log.md` template with date, device/host, OS/runtime,
  commit/worktree, checklist scope, commands/actions, result, log paths,
  screenshot/recording paths, blockers, and next steps.
- Done: platform-specific evidence log templates are present for macOS host,
  Windows host, iOS simulator, iOS device, and LAN runner validation runs.
- Done: this plan points the next validation stage at the fillable evidence
  logs before any platform support claim changes.
- Open: no real-platform checklist execution was performed as part of T5.
- Open: pass/fail evidence still needs to be copied back into this plan after
  each dated validation bundle is collected.

Next T-stage open items:

1. T6: Run the macOS host evidence checklist on real hardware and attach the
   completed `evidence-log.md` plus command logs, screenshots, and `.xcresult`
   paths.
2. T7: Run iOS simulator and physical-device evidence checklists, including
   signing, Keychain, SQLite sandbox, local network permission, and UniFFI link
   status.
3. T8: Run LAN runner evidence validation against a disposable runner host and
   record bind, health, pairing, capabilities, auth rejection, diagnose, and
   approval-required shell-path results.
4. T9: Run the Windows host evidence checklist on a disposable Windows VM,
   including PowerShell, UAC, PATH, execution policy, Event Log, and runner host
   behavior.
5. T10: Feed completed evidence logs back into this plan and convert verified
   blockers into implementation tasks.

### Stage 2: Real Rust Bridge

- Generate Swift bindings from `crates/mobile-agent-core/src/uniffi_api.rs`.
- Replace one mock bridge surface at a time in
  `ios/DeepSeekMobileDemo/MobileCoreBridge.swift`, starting with session
  creation, bootstrap step generation, bootstrap output import, and audit
  timeline reads.
- Keep mock runner/browser/maintenance paths available until live transports
  have the same UI coverage.

Exit criteria:

- iOS creates a real Rust-backed mobile session.
- Homebrew/Xcode CLT bootstrap command cards and audit entries come from
  `crates/mobile-agent-core`, not Swift mock data.

### Stage 3: Live Runner on Local Network

- Add or document the concrete `kai-runner` binary/service entrypoint.
- Bind the runner to an explicit local address and expose `/health`,
  `/pairing/code`, `/pairing/redeem`, `/capabilities`, `/approval/nonce`, and
  `/tool-call` only behind the chosen auth posture.
- Connect `RunnerHttpTransport` or the future WebSocket transport from the
  phone demo to a live runner and validate pairing, capability discovery, and
  `remote.diagnose.system`.
- Keep `remote.shell.exec` blocked until nonce binding and command policy are
  hardened beyond the current scaffold.

Exit criteria:

- A phone or simulator pairs with a live local runner and reads capabilities.
- Live diagnosis works without enabling arbitrary shell execution.

### Stage 4: Controlled Execution and Install/Upgrade

- Bind approval nonces to session id, tool name, command hash, cwd, expiry, and
  idempotency key before enabling shell/powershell execution in a live runner.
- Execute bootstrap runner install on a disposable Mac and Windows VM, including
  offline fallback and failure audit.
- Convert maintenance dry-run into a staged executor only after signed artifacts,
  rollback, and local approval UX are verified.
- Add a real `BrowserEngine` only after the remote shell and runner install
  boundary is understood.

Exit criteria:

- Live runner execution can run a narrow read-only diagnostic allowlist.
- Install/upgrade failures produce usable audit records and bootstrap fallback
  steps.

## Task Tree

```text
Mobile Porting Program
├─ A. Architecture split
│  ├─ [done] A1. Inventory current runtime dependencies
│  ├─ [done] A2. Freeze phone-core non-goals: no shell, PTY, TUI, LSP, MCP stdio
│  ├─ [done] A3. Define mobile-agent-core public API
│  ├─ [done] A4. Define remote tool protocol and event protocol
│  └─ [done] A5. Add cargo workspace member behind no default app behavior change
│
├─ B. Mobile agent core
│  ├─ [done] B1. Session model (`src/session.rs`)
│  ├─ [done] B2. Message/session snapshots (`src/persistence.rs`)
│  ├─ [done] B3. Turn loop with max-step guard (`src/agent_loop.rs`)
│  ├─ [done] B4. Model client trait and fake model (`src/model.rs`)
│  ├─ [done] B5. Tool dispatcher trait and fake/runner transports (`src/transport.rs`)
│  ├─ [done] B6. Approval gate and risk classifier (`src/approval.rs`, `src/risk.rs`)
│  ├─ [done] B7. Event stream (`src/event.rs`)
│  ├─ [done] B8. Persistence abstraction plus SQLite scaffold (`src/persistence.rs`)
│  └─ [partial] B9. UniFFI binding surface exists; Swift demo is not linked to generated bindings
│
├─ C. Bootstrap mode
│  ├─ [done] C1. Bootstrap tool schema (`src/bootstrap.rs`)
│  ├─ [done] C2. Command card generation
│  ├─ [done] C3. User output ingestion
│  ├─ [partial] C4. Paste normalization scaffold; real OCR is iOS-side future work
│  ├─ [done] C5. Manual audit log (`src/audit.rs`)
│  ├─ [done] C6. Homebrew/Xcode CLT diagnostic playbook prompt
│  ├─ [done] C7. macOS runner install/bootstrap fallback guidance
│  └─ [done] C8. Windows runner rescue guidance for winget, PATH, execution policy, and offline install
│
├─ D. Remote execution
│  ├─ [partial] D1. SSH connection config exists; production credential storage is iOS/Keychain work
│  ├─ [partial] D2. `remote.shell.exec` schema and fake/runner paths exist; live runner execution is gated/incomplete
│  ├─ [partial] D3. Output event shapes exist; real network streaming remains open
│  ├─ [partial] D4. Timeout fields and runner helpers exist; cancel semantics need real transport validation
│  ├─ [partial] D5. Idempotency keys exist for maintenance planning; broader mutating tools still need binding
│  ├─ [partial] D6. PowerShell schema/runner support exists; Windows real host smoke is open
│  └─ [done] D7. Windows diagnostics/bootstrap playbook scaffold
│
├─ E. Runner
│  ├─ [partial] E1. `kai-runner` crate and pairing API exist; standalone install/service path is open
│  ├─ [done] E2. capabilities endpoint and report schema
│  ├─ [partial] E3. shell/powershell tool exists only behind explicit approval nonce policy
│  ├─ [done] E4. file read/write with workspace confinement and backups
│  ├─ [done] E5. diagnose system scaffold
│  ├─ [partial] E6. package install dry-run/preview only; execution disabled
│  ├─ [partial] E7. browser session/approval scaffold only; no real browser engine
│  ├─ [partial] E8. remote MCP proxy shape exists; hardening and real servers open
│  └─ [partial] E9. self-update and uninstall plans/audit exist; execution disabled
│
├─ F. iOS app
│  ├─ [done] F1. SwiftUI session list scaffold
│  ├─ [done] F2. Chat and event rendering scaffold
│  ├─ [done] F3. Command cards
│  ├─ [done] F4. Approval sheet with shell/browser nonce presentation
│  ├─ [done] F5. Connection setup for bootstrap, SSH, runner, remote MCP
│  ├─ [partial] F6. Keychain adapter scaffold; real device validation open
│  ├─ [partial] F7. SQLite persistence adapter scaffold; iOS sandbox validation open
│  ├─ [partial] F8. Background/resume UI model exists; real lifecycle validation open
│  └─ [open] F9. Internal testing build/TestFlight build
│
├─ G. Verification
│  ├─ [done] G1. Fake model deterministic tests
│  ├─ [done] G2. Fake transport approval tests
│  ├─ [partial] G3. Manual bootstrap Homebrew test scaffold; real Mac execution open
│  ├─ [partial] G4. SSH Mac Homebrew test scaffold; real Mac execution open
│  ├─ [partial] G5. Windows PowerShell smoke scaffold; real Windows host open
│  ├─ [done] G6. Runner capability/auth/pairing tests
│  ├─ [done] G7. Audit log and session recovery tests
│  ├─ [done] G8. Runner maintenance dry-run/audit tests
│  ├─ [done] G9. Browser session/approval scaffold tests
│  └─ [open] G10. macOS Xcode/Swift/iOS simulator verification
│
├─ P. Runner live pairing and maintenance approval
│  ├─ [done] P1. Runner pairing live smoke over local HTTP in `crates/kai-runner/tests/runner_pairing_live_auth.rs`
│  ├─ [done] P2. Mobile-core-to-live-runner smoke with bearer auth in `crates/kai-runner/tests/runner_mobile_core_live_e2e.rs`
│  ├─ [done] P3. Maintenance approval request E2E without network in `crates/mobile-agent-core/tests/maintenance_approval_flow.rs`
│  └─ [partial] P4. Maintenance execution remains dry-run; signed artifacts and staged executor are still open
│
├─ Q. Runner adapters and platform scaffolds
│  ├─ [done] Q1. Browser adapter seam exists through `BrowserEngine`, `BrowserSessionRegistry`, and `crates/kai-runner/tests/runner_browser.rs`
│  ├─ [partial] Q2. Real browser engine is not connected; Playwright/WebDriver/Safari/CDP selection remains open
│  ├─ [done] Q3. Shell policy requires approval nonce through `ShellEnabledKaiRunner<E>` and `ShellApprovalNonceManager`
│  ├─ [partial] Q4. Shell policy now has command lease scaffolding, but still lacks production OS sandbox and real-network hardening
│  ├─ [done] Q5. PowerShell scaffold exists for `remote.powershell.exec` and Windows runner request shaping
│  ├─ [partial] Q6. Real Windows host smoke remains open for PowerShell, UAC, PATH, execution policy, and runner install paths
│  └─ [done] Q7. iOS UniFFI handoff scripts exist in `ios/DeepSeekMobileDemo/Scripts/generate-uniffi-macos` and `verify-macos`
│
├─ S. Real platform validation
│  ├─ [done] S1. UniFFI UDL scaffold exists in `crates/mobile-agent-core/src/uniffi_api.udl` and is checked by `build.rs`
│  ├─ [done] S2. iOS pairing-upgrade bridge exists in `CoreBridgeJSONAdapter` with redacted fallback profile shaping
│  ├─ [done] S3. Gated `SystemShellExecutor` exists but is disabled by default and only test-enabled after nonce and policy checks
│  ├─ [done] S4. PowerShell HTTP/live smoke covers planned output, approval-required output, bearer auth, and `windows_rescue`
│  ├─ [done] S5. Split real-platform manual validation into `docs/mobile-validation-checklists.md`
│  ├─ [open] S6. Real Mac/Xcode build: run `ios/DeepSeekMobileDemo/Scripts/verify-macos`, Swift tests, simulator, and macOS runner bootstrap checklist
│  ├─ [open] S7. Real iOS simulator/device validation: signing, Keychain, SQLite sandbox, local network permission, and UniFFI link
│  ├─ [open] S8. Real LAN runner validation: bind, health, pairing, capabilities, auth rejection, diagnose, and approval-required shell path
│  ├─ [open] S9. Real Windows test against PowerShell, UAC, PATH, execution policy, Event Log, and runner host behavior
│  ├─ [open] S10. Feed platform evidence back into this plan and convert verified blockers into implementation tasks
│  ├─ [open] S11. Real UniFFI generation and Swift link from `crates/mobile-agent-core/src/uniffi_api.udl`
│  └─ [open] S12. Real browser engine behind `BrowserEngine` with approval-bound click/extract flows
│
└─ T. Evidence-backed platform validation
   ├─ [done] T5. Add fillable evidence log templates for macOS, Windows, iOS simulator/device, and LAN runner validation
   ├─ [open] T6. Real Mac evidence run: completed `evidence-log.md`, command logs, screenshots, `.xcresult`, and runner bootstrap notes
   ├─ [open] T7. Real iOS simulator/device evidence run: signing, Keychain, SQLite sandbox, local network permission, and UniFFI link status
   ├─ [open] T8. Real LAN runner evidence run: bind, health, pairing, capabilities, auth rejection, diagnose, and approval-required shell path
   ├─ [open] T9. Real Windows evidence run: PowerShell, UAC, PATH, execution policy, Event Log, and runner host behavior
   └─ [open] T10. Feed completed evidence logs back into this plan and convert verified blockers into implementation tasks
```

## Mobile-Runnable Milestones

| Milestone | What Runs on Phone | What Can Be Tested by a Human | Required Scope |
|---|---|---|---|
| M0: Rust core linked | Not yet on phone; Rust `uniffi_api` scaffold exists | Linux can test Rust API; Xcode binding link remains open | Core API, fake model, generated Swift bindings |
| M1: Bootstrap command cards | Mock Swift UI and Rust bootstrap steps exist | Linux can test command generation; real Mac paste flow remains open | Bootstrap mode, fake or real model |
| M2: Real cloud model | Rust model client scaffolds/tests exist | Linux can test model transport shapes; phone streaming remains open | Model client, event stream, iOS network integration |
| M3: Session resume | Rust and Swift persistence scaffolds exist | Linux can test Rust persistence; iOS kill/reopen remains open | SQLite persistence, iOS lifecycle |
| M4: SSH diagnostics | Rust SSH mapping/fake transport exists | Real Mac SSH execution remains open | SSH transport, approval gate, credential storage |
| M5: Approved repair | Rust/iOS approval scaffolds exist | Mock nonce UX can be reviewed; real high-risk command execution remains blocked/open | Risk classifier, approval UI, nonce binding |
| M6: Runner mode | Runner HTTP/auth/pairing/capability scaffold exists | Linux can test handler/API behavior; phone-to-live-runner remains open | Runner transport + capabilities + service lifecycle |
| M7: Browser assist | Browser schema/session/click approval scaffold exists | Mock browser card and approval can be reviewed; real engine remains open | Browser tool schema, runner `BrowserEngine`, audit policy |
| M8: Runner maintenance | Dry-run plan/audit scaffold exists | Linux can test self-update/uninstall plan and nonce audit; execution remains disabled | Signed artifacts, rollback, staged executor |
| M9: Runner pairing live smoke | Local HTTP runner pairing and bearer auth are covered by `runner_pairing_live_auth.rs` and `runner_mobile_core_live_e2e.rs` | Linux can test local socket pairing, capabilities, auth rejection, and diagnose call; LAN phone-to-runner is still open | Pairing manager, bearer token routes, capabilities, diagnose tool |
| M10: Maintenance approval E2E | Mobile approval metadata can be shaped into runner maintenance plan requests | Linux can test nonce metadata, denial short-circuit, replay rejection, and redacted audit; real update/uninstall remains disabled | Approval request, nonce manager, maintenance dry-run, audit redaction |
| M11: Browser adapter | Runner accepts browser open/extract/click shapes through `BrowserEngine` and session registry | Fake engine tests can validate adapter calls and click approval; no real browser is driven yet | `BrowserEngine`, session registry, approval-bound click policy |
| M12: Shell and PowerShell policy | Shell and PowerShell routes exist only behind explicit approval nonce scaffolds | Linux can test blocked defaults and scaffold request handling; real Windows host and production sandbox remain open | `ShellEnabledKaiRunner<E>`, nonce policy, PowerShell request/response schema |
| M13: iOS UniFFI handoff | macOS scripts and Swift package layout document generated binding/artifact paths | Script syntax and plan checks can be reviewed; real UniFFI generation, Xcode link, simulator, and device build remain open | `generate-uniffi-macos`, `verify-macos`, generated Swift bindings, static library or XCFramework |
| M14: UniFFI and pairing bridge scaffold | Rust UDL scaffold and iOS pairing-upgrade bridge exist without requiring Swift generation on Linux | Linux can test UDL/API alignment and inspect redacted iOS pairing profile shaping; real Swift generation/link remains open | `uniffi_api.udl`, `build.rs`, `CoreBridgeJSONAdapter`, pairing profile models |
| M15: Gated local executor smoke | A real local shell executor exists only behind explicit test opt-in, approval nonce, and `ShellExecutionPolicy` | Linux can run a harmless `printf ok` executor test while default runner remains blocked; production sandbox remains open | `SystemShellExecutor`, approval nonce, cwd/env/PATH policy |
| M16: PowerShell HTTP smoke | HTTP and live-local runner tests cover PowerShell planned and approval-required responses | Linux can test request/response shape and auth without Windows; real Windows host behavior remains open | `/tool-call`, `remote.powershell.exec`, `windows_rescue`, bearer auth |

## Risk List

| Risk | Mitigation |
|---|---|
| iOS background limits | Persist every turn/approval/tool state before network calls; resume on foreground; avoid long-running local servers. |
| Network disconnect/recovery | Use idempotency keys, reconnectable event streams, command status polling, and explicit unknown-state handling. |
| Tool call idempotency | Every mutating remote call requires an idempotency key and a preflight/dry-run where possible. |
| High-risk command approval | Central risk classifier plus mandatory Swift approval sheet; runner must reject unapproved nonce-less high-risk calls. |
| LLM hallucinated commands | Command risk classifier, allow/deny rules, OS-specific playbooks, one-command bootstrap steps, and user-visible explanations. |
| User computer initially unreachable | Bootstrap mode is a first-class transport, not an error path. |
| Runner install failure | Agent falls back to bootstrap/SSH and diagnoses install failure as a normal scenario. |
| App Store review risk | Avoid hidden remote-control behavior; present user-driven computer management, explicit approvals, no screen streaming, clear privacy disclosures. |
| Secrets and credentials | Store model keys and SSH credentials in Keychain; redact secrets in logs; never send private keys to model. |
| Prompt injection from logs/web pages | Treat command output, logs, browser text, MCP results, and issue text as untrusted data. Do not execute instructions found in outputs. |
| Cross-platform command differences | Use OS-specific diagnostic profiles and prefer structured runner tools once available. |
| Sudo/UAC prompts | Treat elevation as high-risk; explain local prompt behavior; never auto-submit passwords. |
| Large logs | Use bounded reads, truncation metadata, summaries, and explicit "read next" actions. |
| Remote MCP trust | Runner exposes capabilities and policy metadata; unknown mutating MCP tools require approval by default. |

## Minimal Technical Spike

Scenario: user enters on iOS demo: "帮我检查 Mac 上 Homebrew 为什么安装失败".

Target spike flow:

1. Phone Agent creates a diagnostic plan:
   - identify macOS version and architecture
   - check whether `brew` exists
   - check Xcode Command Line Tools
   - check DNS/HTTPS to Homebrew/GitHub
   - inspect common shell PATH locations
2. User chooses SSH or bootstrap.
3. Bootstrap path:
   - Agent emits:
     `uname -a; sw_vers; command -v brew; xcode-select -p; echo "$PATH"`
   - User runs it and pastes output.
   - Agent explains the finding and emits the next command.
4. SSH path:
   - Agent calls `remote.shell.exec` with the same low-risk diagnostics.
   - Output streams back as events.
5. If `brew doctor` is available, Agent runs it.
6. If CLT is missing, Agent proposes `xcode-select --install` and asks approval.
7. If PATH is broken, Agent proposes a `.zprofile` or `.zshrc` edit and asks
   approval, including backup/rollback plan.
8. Final output explains the root cause and next step in user language.

Spike success criteria:

- The phone app can complete both bootstrap and SSH variants against a real Mac.
- The app never attempts local phone shell execution.
- Every command/output pair is visible in audit history.
- High-risk commands stop at approval.
- The session can resume after app restart.

## Modules That Should Not Enter iOS

Do not link these into `mobile-agent-core`:

- `crates/tui/src/tui/*`
- `crates/tui/src/tools/shell.rs`
- `crates/tui/src/tools/file.rs`
- `crates/tui/src/tools/apply_patch.rs`
- `crates/tui/src/tools/git.rs`
- `crates/tui/src/lsp/*`
- `crates/tui/src/sandbox/*`
- `crates/tui/src/mcp_server.rs`
- local MCP stdio spawn parts of `crates/tui/src/mcp.rs`
- `crates/tui/src/repl/*`
- local `crates/tui/src/snapshot/*`
- CLI dispatcher code in `crates/cli`

These modules may be used as runner-side references or moved into a desktop
runner binary after explicit hardening.

## Concrete File/Module Change Map

First implementation wave:

- Add `crates/mobile-agent-core`.
- Add remote schema either in `crates/mobile-agent-core/src/remote_schema.rs`
  or a new shared crate if both app and runner need it immediately.
- Reuse `crates/protocol/src/lib.rs` event style, but do not force all mobile
  events into the existing desktop `EventFrame` until the schema settles.
- Reuse `crates/tools/src/lib.rs` registry concepts.
- Reuse and extend `crates/execpolicy/src/lib.rs` for remote command approval.
- Mine `crates/tui/src/core/engine.rs` for turn-loop behavior, not by direct
  dependency at first.
- Mine `crates/tui/src/client.rs`, `crates/tui/src/llm_client/*`, and
  `crates/tui/src/models.rs` for model streaming types.
- Add iOS bindings under `crates/mobile-agent-core/src/uniffi.rs`.

Second implementation wave:

- Create `crates/kai-runner` or a separate repo if release cadence differs.
- Move/adapt runner-safe parts of `crates/tui/src/tools/shell.rs`,
  `file.rs`, `diagnostics.rs`, and HTTP MCP support.
- Add runner capability reporting and approval nonce enforcement.
- Keep DeepSeek TUI behavior unchanged until the runner is a separate binary.

## Conclusion

DeepSeek TUI should not be ported wholesale to iOS. The correct path is to use
its protocol, approval, tool, session, and runtime ideas to build a smaller
phone-side `mobile-agent-core`, while turning desktop execution capabilities
into remote runner tools.

Investment decision: yes, spend one week on a technical spike. The week should
produce a phone-runnable bootstrap demo and an SSH diagnostic prototype. If that
works, continue with a 2-4 week MVP that adds persistence, approvals, and a
minimal runner.
