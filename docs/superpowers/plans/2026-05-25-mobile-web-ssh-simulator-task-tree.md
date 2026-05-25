# Mobile Web SSH Simulator Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a LAN-accessible mobile Web simulator that drives a Rust HTTP server and controls `root@192.168.30.244` through approval-gated SSH flows.

**Architecture:** Add an independent `crates/mobile-web-server` Axum server and a separate `mobile-web` phone-oriented Web UI. The server owns HTTP/SSE state, approval state, SSH execution orchestration, and scriptable validation while reusing `deepseek-mobile-agent-core` for SSH command shape, risk, approval, audit, and remote tool contracts.

**Tech Stack:** Rust 1.88+, Axum, Tokio, Serde, `deepseek-mobile-agent-core`, system `ssh`, Python 3 smoke scripts, React/Vite or equivalent lightweight Web tooling.

---

## Parallelization Summary

The work should be split into six lanes. Lanes 1-4 can start in parallel after
the API contract is agreed. Lanes 5-6 integrate and verify the outputs.

```text
Phase 0: Contract Lock
  |
  ├─ Lane 1: Rust server skeleton and API state
  ├─ Lane 2: SSH execution backend and approval flow
  ├─ Lane 3: Web UI shell and API client
  └─ Lane 4: Python scripts and evidence helpers
       |
Phase 1: Integration
       |
Phase 2: Full validation and docs update
```

Use separate subagents for lanes 1-4. Keep lane 5 in the parent session or a
single integration subagent because it touches all boundaries.

## Shared Contract

All lanes must use this first-stage contract unless the parent coordinator
updates the plan.

### Server

```text
Package: deepseek-mobile-web-server
Crate path: crates/mobile-web-server
Default bind: 0.0.0.0:8788
Default SSH target: root@192.168.30.244:22
```

### API

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

### Event Types

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

### Preset Diagnostics

```text
system_info: uname -a
current_user: id
disk_usage: df -h
memory: free -m || cat /proc/meminfo
network: ip addr || ifconfig
working_directory: pwd
```

Preset diagnostics execute directly. Advanced commands always create pending
approval first.

## Task Tree

### Task 0: API Contract and Workspace Wiring

**Purpose:** Create the shared crate/package boundaries so independent agents do
not invent incompatible types.

**Files:**
- Create: `crates/mobile-web-server/Cargo.toml`
- Create: `crates/mobile-web-server/src/lib.rs`
- Create: `crates/mobile-web-server/src/types.rs`
- Create: `crates/mobile-web-server/src/state.rs`
- Create: `crates/mobile-web-server/src/main.rs`
- Modify: `Cargo.toml`

- [ ] Add `crates/mobile-web-server` to workspace members, but not default members.
- [ ] Define serializable API types in `types.rs`: `HealthResponse`,
  `SessionSummary`, `Message`, `MessagePart`, `SshTarget`, `DiagnosticRequest`,
  `CommandPrepareRequest`, `ApprovalRespondRequest`, `AuditEntry`,
  `ApiErrorBody`.
- [ ] Define in-memory state in `state.rs`: sessions, messages, pending
  approvals, audit ring, SSH target, and event subscribers.
- [ ] Add unit tests that serialize representative API responses and assert
  stable field names.
- [ ] Run `cargo test -p deepseek-mobile-web-server`.
- [ ] Commit with `feat: add mobile web server contract`.

**Suggested Subagent Prompt:**

```text
Create the initial deepseek-mobile-web-server crate and shared API/state types.
Do not implement SSH execution or Web UI. Use the design at
docs/superpowers/specs/2026-05-25-mobile-web-ssh-simulator-design.md.
Add tests for JSON shape stability. Return the files changed and test results.
```

### Task 1: Rust Server HTTP and SSE Skeleton

**Depends on:** Task 0.

**Purpose:** Make the server start on `0.0.0.0:8788`, expose the API routes, and
support SSE without SSH execution yet.

**Files:**
- Modify: `crates/mobile-web-server/src/main.rs`
- Modify: `crates/mobile-web-server/src/lib.rs`
- Create: `crates/mobile-web-server/src/routes.rs`
- Create: `crates/mobile-web-server/src/events.rs`
- Test: `crates/mobile-web-server/tests/http_api.rs`
- Test: `crates/mobile-web-server/tests/event_stream.rs`

- [ ] Add CLI args: `--host`, `--port`, `--ssh-host`, `--ssh-user`,
  `--ssh-port`, and `--use-real-model`.
- [ ] Implement `/health`, `/api/ssh/target`, `/api/sessions`,
  `/api/sessions/:id/messages`, `/api/audit/recent`.
- [ ] Implement `/event` SSE with a connection event and broadcast helper.
- [ ] Ensure startup logs do not include token-like data or approval secrets.
- [ ] Add tests for health, target response, session creation/listing, audit
  listing, and SSE response headers.
- [ ] Run `cargo test -p deepseek-mobile-web-server`.
- [ ] Commit with `feat: serve mobile web api skeleton`.

**Suggested Subagent Prompt:**

```text
Implement the HTTP/SSE skeleton for crates/mobile-web-server after Task 0
exists. Do not implement real SSH command execution. Add route tests for health,
sessions, SSH target, audit, and SSE headers. Keep logs redacted. Return summary
and exact test command output.
```

### Task 2: SSH Diagnostics Backend

**Depends on:** Task 0. Can run in parallel with Task 1 if it uses direct unit
tests instead of routes.

**Purpose:** Execute preset low-risk diagnostics through system `ssh` and return
structured tool events and audit entries.

**Files:**
- Create: `crates/mobile-web-server/src/ssh_exec.rs`
- Create: `crates/mobile-web-server/src/diagnostics.rs`
- Modify: `crates/mobile-web-server/src/routes.rs`
- Test: `crates/mobile-web-server/tests/diagnostics.rs`

- [ ] Define a small `CommandRunner` trait so tests can use a fake runner and
  production can invoke system `ssh`.
- [ ] Implement preset diagnostic mapping exactly from the shared contract.
- [ ] Use `deepseek-mobile-agent-core::ssh` types where useful for command
  invocation shape.
- [ ] Add tests proving preset diagnostics do not require approval.
- [ ] Add tests proving stdout/stderr/exit code/duration are captured in
  response, events, and audit.
- [ ] Add tests proving audit output redacts known token-like sentinel fields.
- [ ] Run `cargo test -p deepseek-mobile-web-server diagnostics`.
- [ ] Commit with `feat: add mobile web ssh diagnostics`.

**Suggested Subagent Prompt:**

```text
Implement preset SSH diagnostics for mobile-web-server using a fakeable command
runner. Production should call system ssh, but tests must not require the real
192.168.30.244 host. Preset diagnostics are low-risk and execute directly.
Return changed files and test results.
```

### Task 3: Advanced Command Approval Flow

**Depends on:** Task 0. Integrates with Task 1 routes and Task 2 runner.

**Purpose:** Make arbitrary advanced commands create pending approvals and only
execute after `approve_once`.

**Files:**
- Create: `crates/mobile-web-server/src/approvals.rs`
- Modify: `crates/mobile-web-server/src/routes.rs`
- Modify: `crates/mobile-web-server/src/events.rs`
- Modify: `crates/mobile-web-server/src/ssh_exec.rs`
- Test: `crates/mobile-web-server/tests/approvals.rs`

- [ ] Implement `POST /api/commands/prepare` to create a pending approval
  without executing the command.
- [ ] Emit `approval.asked` when an advanced command is prepared.
- [ ] Implement `POST /api/approvals/:id/respond` with `approve_once` and
  `reject`.
- [ ] On `approve_once`, execute the SSH command once and mark approval
  consumed.
- [ ] On `reject`, do not execute and emit `approval.replied`.
- [ ] Reject replay of a consumed approval id.
- [ ] Do not implement `always approve`.
- [ ] Add tests for prepare, reject, approve once, replay rejection, and event
  emission.
- [ ] Run `cargo test -p deepseek-mobile-web-server approvals`.
- [ ] Commit with `feat: gate advanced ssh commands behind approval`.

**Suggested Subagent Prompt:**

```text
Implement advanced command approval for mobile-web-server. Every arbitrary
command must create a pending approval first. Support approve_once and reject
only. Do not add always-approve. Use fake command execution in tests and assert
replay rejection. Return summary and tests.
```

### Task 4: Mobile Web UI

**Depends on:** Task 0 shared API names. Can start with mocked API responses.

**Purpose:** Build the phone-browser control surface.

**Files:**
- Create: `mobile-web/package.json`
- Create: `mobile-web/index.html`
- Create: `mobile-web/src/main.tsx`
- Create: `mobile-web/src/App.tsx`
- Create: `mobile-web/src/api.ts`
- Create: `mobile-web/src/types.ts`
- Create: `mobile-web/src/styles.css`
- Create: `mobile-web/vite.config.ts`
- Create: `mobile-web/tsconfig.json`

- [ ] Scaffold a lightweight React/Vite app under `mobile-web`, not the existing
  marketing `web/` directory.
- [ ] Implement API client functions for the shared contract routes.
- [ ] Implement SSE connection state and event reducer.
- [ ] Implement mobile layout: connection header, diagnostic buttons, timeline,
  advanced command input, approval cards, audit section.
- [ ] Hide `always approve`; show only approve once and reject.
- [ ] Ensure long stdout/stderr wraps and does not overflow on phone widths.
- [ ] Add `npm run build` and a `npm run typecheck` command.
- [ ] Commit with `feat: add mobile web ssh control UI`.

**Suggested Subagent Prompt:**

```text
Build the mobile-web React/Vite UI for the mobile Web SSH simulator. Do not use
the existing web/ marketing app. Use the shared API contract from the plan.
Focus on a usable phone browser control surface: status, diagnostics, timeline,
advanced command approval, audit. Return changed files and build result.
```

### Task 5: Development and Smoke Scripts

**Depends on:** Task 1 route skeleton. Can develop with expected route shapes
and finalize after Tasks 2-3.

**Purpose:** Make the flow reproducible without manual browser interaction.

**Files:**
- Create: `scripts/mobile_web_dev.py`
- Create: `scripts/mobile_web_ssh_smoke.py`
- Create: `scripts/mobile_web_ssh_flow_simulator.py`
- Test: `scripts/mobile_web_ssh_flow_simulator_smoke.py`

- [ ] Implement `mobile_web_dev.py` to start or print commands for Rust server
  and Web dev server.
- [ ] Implement `mobile_web_ssh_smoke.py` to check `/health`, `/event`,
  `/api/ssh/target`, one preset diagnostic, and `/api/audit/recent`.
- [ ] Implement `mobile_web_ssh_flow_simulator.py` to create a session, run a
  preset diagnostic, prepare an advanced command, reject one command, prepare a
  second command, auto-approve it with `--auto-approve`, and verify audit.
- [ ] Add `--server`, `--json`, `--auto-approve`, and target override flags.
- [ ] Ensure scripts never print DeepSeek token values.
- [ ] Add a smoke test using a local fake HTTP server or dry-run fixture.
- [ ] Run `python3 scripts/mobile_web_ssh_flow_simulator_smoke.py`.
- [ ] Commit with `test: add mobile web ssh simulator scripts`.

**Suggested Subagent Prompt:**

```text
Create Python scripts for mobile-web-server development and HTTP flow
validation. Do not require the real SSH host in script smoke tests; use dry-run
or a fake local HTTP server. The live script may target 127.0.0.1:8788. Return
changed files and smoke output.
```

### Task 6: Static Serving and LAN URL Integration

**Depends on:** Tasks 1 and 4.

**Purpose:** Let the Rust server serve the built Web UI and print useful LAN
URLs.

**Files:**
- Modify: `crates/mobile-web-server/src/routes.rs`
- Modify: `crates/mobile-web-server/src/main.rs`
- Modify: `mobile-web/vite.config.ts`
- Test: `crates/mobile-web-server/tests/static_serving.rs`

- [ ] Decide the static build directory, preferably `mobile-web/dist`.
- [ ] Add optional `--static-dir mobile-web/dist`.
- [ ] Serve `index.html` for the Web root and client-side routes.
- [ ] Keep API routes under `/api`, `/health`, and `/event`.
- [ ] Print local and LAN URLs on startup.
- [ ] Add tests for static index serving and API route priority.
- [ ] Run `cargo test -p deepseek-mobile-web-server static_serving`.
- [ ] Commit with `feat: serve mobile web app from rust server`.

**Suggested Subagent Prompt:**

```text
Integrate static serving for mobile-web-server. The server should serve
mobile-web/dist when provided and keep API/SSE routes working. Print LAN and
local URLs without secrets. Return tests and summary.
```

### Task 7: Integration Validation and Documentation

**Depends on:** Tasks 1-6.

**Purpose:** Verify the whole system and update docs without making false
platform claims.

**Files:**
- Modify: `docs/linux-first-mobile-task-tree.md`
- Modify: `docs/mobile-porting-plan.md`
- Modify: `docs/mobile-validation-checklists.md`
- Create: `docs/mobile-web-ssh-simulator.md`

- [ ] Run `cargo fmt --all --check`; fix formatting if needed.
- [ ] Run `cargo test -p deepseek-mobile-agent-core -p kai-runner`.
- [ ] Run `cargo test -p deepseek-mobile-web-server`.
- [ ] Run `cargo clippy -p deepseek-mobile-agent-core -p kai-runner --all-targets --all-features`.
- [ ] Run clippy for `deepseek-mobile-web-server`.
- [ ] Run `python3 scripts/mobile_linux_validation.py`.
- [ ] Run `python3 scripts/mobile_web_ssh_flow_simulator.py --server http://127.0.0.1:8788 --auto-approve --json` against a started server.
- [ ] Run the mobile Web build command.
- [ ] Run `git diff --check -- docs ios/DeepSeekMobileDemo crates/kai-runner crates/mobile-agent-core crates/mobile-web-server scripts Cargo.toml Cargo.lock mobile-web`.
- [ ] Update docs to say this is Linux/LAN Web simulator evidence only.
- [ ] Create `docs/mobile-web-ssh-simulator.md` with startup, LAN access,
  SSH target, approval behavior, script commands, and evidence boundary notes.
- [ ] Commit with `docs: document mobile web ssh simulator validation`.

**Suggested Subagent Prompt:**

```text
Run final integration validation for the mobile Web SSH simulator. Do not mark
real iOS, macOS, Windows, Xcode, Keychain, or browser automation evidence as
complete. Update docs only with Linux/LAN Web simulator status. Return command
results, failures, and doc changes.
```

## Recommended Parallel Dispatch

Dispatch these after Task 0 is complete:

```text
Agent A: Task 1 - Rust HTTP/SSE skeleton
Agent B: Task 2 - SSH diagnostics backend
Agent C: Task 4 - Mobile Web UI with mocked API
Agent D: Task 5 - Python scripts with fake server smoke
```

Then dispatch or run sequentially:

```text
Agent E: Task 3 - Advanced approval flow
Agent F: Task 6 - Static serving integration
Parent/Agent G: Task 7 - final integration and docs
```

Avoid parallel edits to the same files:

- `routes.rs` is shared by Tasks 1, 2, 3, and 6. The parent should merge these
  in order or assign route integration to one agent.
- `Cargo.toml` workspace edits belong to Task 0 only.
- `mobile-web/src/types.ts` should be treated as generated-from-contract by the
  Web agent; if server types change, parent updates the Web agent prompt.

## Verification Gate

Before claiming completion, run:

```bash
python3 scripts/mobile_linux_validation.py
cargo fmt --all --check
cargo test -p deepseek-mobile-agent-core -p kai-runner
cargo test -p deepseek-mobile-web-server
cargo clippy -p deepseek-mobile-agent-core -p kai-runner --all-targets --all-features
cargo clippy -p deepseek-mobile-web-server --all-targets --all-features
python3 scripts/mobile_web_ssh_flow_simulator.py --server http://127.0.0.1:8788 --auto-approve --json
git diff --check -- docs ios/DeepSeekMobileDemo crates/kai-runner crates/mobile-agent-core crates/mobile-web-server scripts Cargo.toml Cargo.lock mobile-web
```

The live SSH simulator command requires a running server and reachable
`root@192.168.30.244`. If that host is unavailable, record the failure as a
Linux/LAN Web simulator blocker, not as real iOS/macOS/Windows evidence.
