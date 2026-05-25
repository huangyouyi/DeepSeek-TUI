# Mobile Web SSH Simulator Design

## Purpose

Build a Linux-hosted mobile Web simulator for the DeepSeek-TUI mobile porting
work. The simulator lets a phone browser drive a Rust HTTP server on the Linux
development machine, which then uses `mobile-agent-core` and local SSH to
control a disposable Linux test host.

This is not real iOS, macOS, Xcode, Windows, Keychain, simulator, or physical
device evidence. It extends the Linux-first phase by proving more of the future
phone UX and remote-control flow before platform validation.

## Goals

- Serve a mobile-oriented Web UI over LAN from the Linux development machine.
- Run the Rust server on `0.0.0.0` so a phone browser can access it.
- Control the Linux test host at `root@192.168.30.244` through SSH.
- Provide preset read-only diagnostics that can run without manual approval.
- Provide an advanced command box where every command requires approval.
- Stream session, message, tool, approval, connection, and audit events to the
  Web UI.
- Keep DeepSeek API config read-only by loading from `~/.deepseek/config.toml`
  only when explicitly requested.
- Keep all token, SSH, nonce, and approval metadata out of logs, audit output,
  and evidence.
- Make development and validation reproducible through scripts.

## Non-Goals

- Do not claim real iOS execution.
- Do not run or mark complete Xcode builds, Swift compilation, simulator
  launches, physical iOS installs, Keychain behavior, local network permission
  prompts, macOS Homebrew behavior, Windows host behavior, or browser
  automation engine behavior.
- Do not add Web authentication in the first stage.
- Do not support `always approve` for advanced commands in the first stage.
- Do not store SSH private key contents or modify `~/.deepseek/config.toml`.
- Do not automatically install or mutate software on the remote host in the
  first stage.

## Architecture

```text
Phone browser on trusted LAN
  |
  | HTTP + SSE
  v
Rust mobile web server on Linux
  - static Web serving
  - HTTP API
  - SSE event broker
  - session/message state
  - approval state
  - audit state
  - SSH target manager
  - optional read-only DeepSeek config loader
  |
  v
mobile-agent-core
  - remote schema
  - SSH command builder
  - risk assessment
  - approval metadata
  - audit redaction
  |
  v
system ssh command
  |
  v
root@192.168.30.244
```

Add two new top-level implementation areas:

```text
mobile-web/
  React/Vite Web UI optimized for phone browser use.

crates/mobile-web-server/
  Axum server that exposes the mobile Web API and integrates with
  mobile-agent-core.
```

Reuse existing mobile and runner code where possible:

```text
crates/mobile-agent-core/src/ssh.rs
crates/mobile-agent-core/src/risk.rs
crates/mobile-agent-core/src/approval.rs
crates/mobile-agent-core/src/audit.rs
crates/mobile-agent-core/src/remote_schema.rs
crates/mobile-agent-core/src/transport.rs
```

## Server Startup

The development server should support this shape:

```bash
cargo run -p deepseek-mobile-web-server -- \
  --host 0.0.0.0 \
  --port 8788 \
  --ssh-host 192.168.30.244 \
  --ssh-user root \
  --ssh-port 22
```

Startup output should include local and LAN URLs, the configured SSH target, and
whether real model access is enabled. It must not print API tokens, SSH private
key paths, approval nonces, bearer tokens, or command lease secrets.

Real model access is opt-in. A flag such as `--use-real-model` may read
`~/.deepseek/config.toml`, but the file must remain unmodified and token values
must not be logged.

## API Surface

The first-stage API should be purpose-built for the mobile Web simulator rather
than OpenCode-compatible.

```text
GET  /health
GET  /event

GET  /api/sessions
POST /api/sessions
GET  /api/sessions/:id/messages
POST /api/sessions/:id/prompt

GET  /api/ssh/target
PUT  /api/ssh/target
POST /api/diagnostics/run

POST /api/commands/prepare
POST /api/approvals/:id/respond

GET  /api/audit/recent
```

The API may use in-memory state in the first stage. Persistence can be added
later once the UI and execution contract are stable.

## Event Stream

`GET /event` should use server-sent events. The Web UI should not rely only on
polling.

Initial event types:

```text
session.updated
message.updated
message.part.updated
tool.started
tool.stdout
tool.stderr
tool.completed
tool.failed
approval.asked
approval.replied
audit.updated
connection.updated
```

Events should be redacted before leaving the server.

## Web UI

The first screen should be the working mobile control surface, not a marketing
or landing page.

Required areas:

- Connection header with server status, SSE status, and SSH target.
- Conversation and execution timeline.
- Preset diagnostic buttons.
- Advanced command input.
- Inline pending approval cards.
- Recent audit timeline, collapsible if space is tight.

Preset diagnostics:

```text
System info: uname -a
Current user: id
Disk usage: df -h
Memory: free -m || cat /proc/meminfo
Network: ip addr || ifconfig
Working directory: pwd
```

Preset diagnostics are low-risk and may execute directly over SSH.

Every advanced command must become a pending approval before execution, even if
the command appears low-risk. The first stage supports `approve once` and
`reject`; it does not support `always approve`.

## Approval Flow

```text
Web submits advanced command
  -> POST /api/commands/prepare
  -> server creates pending approval
  -> SSE emits approval.asked
  -> Web renders approval card
  -> user chooses approve once or reject
  -> POST /api/approvals/:id/respond
  -> approved command executes through SSH
  -> SSE emits tool and audit events
```

Approvals should include replay-safe metadata internally. User-visible output
must not expose nonce values, command lease secrets, bearer tokens, or
idempotency keys.

## Scripts

Add script entrypoints so development and validation do not depend on manual
steps.

```text
scripts/mobile_web_dev.py
scripts/mobile_web_ssh_smoke.py
scripts/mobile_web_ssh_flow_simulator.py
```

`mobile_web_dev.py` should start the Rust server and Web dev workflow or print
the exact commands needed when a dependency is missing.

`mobile_web_ssh_smoke.py` should verify:

- `/health` returns ok.
- `/event` can connect.
- The configured SSH target is visible.
- A preset diagnostic can execute.
- Audit output is redacted.

`mobile_web_ssh_flow_simulator.py` should verify:

- A session can be created.
- Messages can be listed.
- A preset diagnostic runs without approval.
- An advanced command creates pending approval.
- `--auto-approve` approves and executes the command.
- Rejection does not execute the command.
- Audit and event output do not leak known secret-like fields.

## Security Boundary

The first stage intentionally allows LAN browser access without a Web token.
This is acceptable only for trusted development networks and must be documented
as such.

Security requirements that still apply:

- Advanced commands always require approval.
- `always approve` is disabled.
- Server logs and events are redacted.
- DeepSeek config is read-only.
- SSH private key contents are never read into API responses or evidence.
- Scripts should fail loudly if they detect token-like sentinels in
  user-visible output.

## Validation

Keep the existing Linux-first validation commands passing:

```bash
python3 scripts/mobile_linux_validation.py
cargo fmt --all --check
cargo test -p deepseek-mobile-agent-core -p kai-runner
cargo clippy -p deepseek-mobile-agent-core -p kai-runner --all-targets --all-features
git diff --check -- docs ios/DeepSeekMobileDemo crates/kai-runner crates/mobile-agent-core scripts Cargo.toml Cargo.lock
```

Add new validation for this feature:

```bash
cargo test -p deepseek-mobile-web-server
python3 scripts/mobile_web_ssh_smoke.py --server http://127.0.0.1:8788
python3 scripts/mobile_web_ssh_flow_simulator.py --server http://127.0.0.1:8788 --auto-approve --json
```

The Web build/test command should be chosen after inspecting the final
`mobile-web/package.json`.

## Completion Criteria

The first stage is complete when:

- A phone browser on the LAN can open the Web UI served by the Linux machine.
- The Web UI can run preset SSH diagnostics against `root@192.168.30.244`.
- The Web UI can create, approve once, reject, and observe advanced command
  approvals.
- The HTTP flow simulator can validate the same behavior without browser
  interaction.
- Existing Linux-first validation still passes.
- Documentation does not mark real macOS, iOS, Windows, or browser automation
  evidence as complete.
