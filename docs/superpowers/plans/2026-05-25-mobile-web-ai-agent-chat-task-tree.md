# Mobile Web AI Agent Chat Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Upgrade the Linux mobile Web SSH control surface from a command console into an AI chat Agent that interprets natural language, proposes or runs remote Linux actions through SSH, gates risky actions behind approval, and returns assistant summaries.

**Architecture:** Keep the existing LAN Web + Rust server + SSH runner foundation. Add a server-side Agent turn layer that accepts natural language, calls either a deterministic mock model or the DeepSeek chat completion API, maps model tool calls into the existing diagnostic/approval/SSH services, and broadcasts chat/tool/approval events over the current SSE stream. The Web UI makes chat the primary workflow and keeps raw shell commands as a secondary advanced tool.

**Tech Stack:** Rust 1.88+, Axum, Tokio, Serde, `deepseek-mobile-agent-core`, DeepSeek OpenAI-compatible chat completions, system `ssh`, React/Vite mobile Web UI, Python smoke/evidence scripts.

---

## Current Problem

The current `Advanced Command` field executes its content as a shell command after approval. If the user types natural language such as `请问当前运行在什么系统？`, the remote shell receives that exact text and returns `not found`. This is correct for a command console, but wrong for the product goal.

The target behavior is:

```text
User natural language
  -> AI interprets intent
  -> AI chooses diagnostic or shell tool
  -> Low-risk tools run directly
  -> High-risk tools ask for approval
  -> SSH executes only approved commands
  -> Assistant summarizes the remote result
```

## Non-Goals

- Do not claim real iOS/macOS/Windows evidence.
- Do not modify `~/.deepseek/config.toml`; read it only.
- Do not log or return DeepSeek API tokens.
- Do not remove the raw command path; keep it as an advanced secondary control.
- Do not implement always-approve.

## Shared Contract

### New API

```text
POST /api/sessions/:id/agent-turn
```

Request:

```json
{
  "message": "请问当前运行在什么系统？"
}
```

Response when a low-risk tool ran:

```json
{
  "session_id": "session-id",
  "turn_id": "turn-id",
  "status": "completed",
  "assistant_text": "这台远程设备运行的是 Linux ...",
  "executed_tools": [
    {
      "tool": "remote.shell.exec",
      "command": "uname -a",
      "requires_approval": false,
      "exit_code": 0
    }
  ],
  "pending_approvals": []
}
```

Response when approval is required:

```json
{
  "session_id": "session-id",
  "turn_id": "turn-id",
  "status": "waiting_for_approval",
  "assistant_text": "我需要执行这条高风险命令，请确认。",
  "executed_tools": [],
  "pending_approvals": [
    {
      "id": "approval-id",
      "command": "opkg update",
      "status": "pending"
    }
  ]
}
```

### New SSE Events

```text
assistant.started
assistant.message
assistant.completed
agent.tool.proposed
agent.tool.completed
agent.tool.failed
```

Existing `approval.asked`, `approval.replied`, `tool.started`, `tool.stdout`, `tool.stderr`, `tool.completed`, and `tool.failed` stay valid.

### Tool Policy

- Low-risk read-only diagnostics may run without approval:
  - `uname -a`
  - `id`
  - `pwd`
  - `df -h`
  - `free -m || cat /proc/meminfo`
  - `ip addr || ifconfig`
- Arbitrary shell commands are high-risk and must create pending approvals.
- The model must never bypass the existing approval service for high-risk commands.

---

## Parallelization Summary

```text
Phase 0: Agent chat contract
  |
  ├─ Lane A: Model/config provider
  ├─ Lane B: Agent turn executor
  ├─ Lane C: Mobile Web chat UI
  └─ Lane D: Script/evidence coverage
       |
Phase 1: Approval continuation + summarization
       |
Phase 2: Full Linux verification
```

Avoid parallel edits to the same file:

- `crates/mobile-web-server/src/routes.rs` should be owned by one integration task.
- `mobile-web/src/App.tsx` should be owned by the Web UI task.
- `scripts/mobile_web_ai_chat_smoke.py` should be owned by the script task.
- `~/.deepseek/config.toml` must not be edited by any task.

---

## Task 0: Agent Chat API Contract

**Purpose:** Define stable request/response/event shapes before model or UI work starts.

**Files:**
- Modify: `crates/mobile-web-server/src/types.rs`
- Modify: `mobile-web/src/types.ts`
- Test: `crates/mobile-web-server/src/lib.rs` or `crates/mobile-web-server/tests/agent_chat_contract.rs`

- [ ] Add Rust API types:
  - `AgentTurnRequest { message: String }`
  - `AgentExecutedTool { tool, command, requires_approval, exit_code, status }`
  - `AgentTurnResponse { session_id, turn_id, status, assistant_text, executed_tools, pending_approvals }`
- [ ] Add matching TypeScript types in `mobile-web/src/types.ts`.
- [ ] Add serialization tests proving field names are stable.
- [ ] Run:
  ```bash
  cargo test -p deepseek-mobile-web-server agent_chat_contract
  ```
- [ ] Commit:
  ```bash
  git commit -m "feat: add mobile web agent chat contract"
  ```

**Subagent Prompt:**

```text
Implement only the Agent chat request/response contract. Do not call models,
execute SSH, or edit the UI. Add Rust and TypeScript types and stable JSON
shape tests. Return changed files and exact test output.
```

## Task 1: Read-Only DeepSeek Config Loader

**Purpose:** Let the server discover model settings from `~/.deepseek/config.toml` without modifying or leaking it.

**Files:**
- Create: `crates/mobile-web-server/src/model_config.rs`
- Modify: `crates/mobile-web-server/src/lib.rs`
- Modify: `crates/mobile-web-server/Cargo.toml`
- Test: `crates/mobile-web-server/tests/model_config.rs`

- [ ] Add `toml.workspace = true` if needed.
- [ ] Implement `MobileModelConfig`:
  ```rust
  pub struct MobileModelConfig {
      pub provider: String,
      pub base_url: String,
      pub model: String,
      pub api_key: Option<String>,
  }
  ```
- [ ] Read config from an explicit path in tests and from `~/.deepseek/config.toml` in production.
- [ ] Support a conservative first pass:
  - `api_key`
  - `model`
  - `base_url`
  - fallback model: `deepseek-v4-flash`
  - fallback base URL: `https://api.deepseek.com`
- [ ] Add a redacted debug/status helper that reports `api_key_present: true/false` but never the value.
- [ ] Tests must prove the config file is read-only and token values are not present in formatted output.
- [ ] Run:
  ```bash
  cargo test -p deepseek-mobile-web-server model_config
  ```
- [ ] Commit:
  ```bash
  git commit -m "feat: load mobile web model config read only"
  ```

**Subagent Prompt:**

```text
Implement a read-only model config loader for mobile-web-server. It may read an
explicit path in tests and ~/.deepseek/config.toml in production. Never write the
file and never expose API token values in logs, JSON, Debug, or errors.
```

## Task 2: Agent Model Provider

**Purpose:** Provide a model abstraction usable by route tests without network and by live runs with DeepSeek.

**Files:**
- Create: `crates/mobile-web-server/src/agent_model.rs`
- Modify: `crates/mobile-web-server/src/lib.rs`
- Modify: `crates/mobile-web-server/Cargo.toml`
- Test: `crates/mobile-web-server/tests/agent_model.rs`

- [ ] Define:
  ```rust
  pub trait AgentModel {
      fn complete(&self, request: AgentModelRequest) -> Result<AgentModelResponse, AgentModelError>;
  }
  ```
- [ ] Implement `MockAgentModel` for Linux tests:
  - Chinese or English OS/system questions produce a low-risk `uname -a` tool.
  - Disk questions produce `df -h`.
  - Identity/user questions produce `id`.
  - Unknown questions produce assistant text with no tool.
- [ ] Implement a DeepSeek HTTP provider behind `--use-real-model`.
  - Use OpenAI-compatible `/chat/completions`.
  - Use function tools for `remote.shell.exec`.
  - Do not log API key.
  - Return clear error text if config has no API key.
- [ ] Tests use fake HTTP/model transport; live tests are not required.
- [ ] Run:
  ```bash
  cargo test -p deepseek-mobile-web-server agent_model
  cargo clippy -p deepseek-mobile-web-server --all-targets --all-features
  ```
- [ ] Commit:
  ```bash
  git commit -m "feat: add mobile web agent model provider"
  ```

**Subagent Prompt:**

```text
Implement the Agent model provider layer. Tests must not call the network.
MockAgentModel should map natural-language OS/disk/user questions to tool calls.
DeepSeek provider may be configured but must be tested with fake transport only.
```

## Task 3: Agent Turn Executor

**Purpose:** Convert model responses into existing diagnostics, SSH execution, or approval requests.

**Files:**
- Create: `crates/mobile-web-server/src/agent_chat.rs`
- Modify: `crates/mobile-web-server/src/routes.rs`
- Modify: `crates/mobile-web-server/src/lib.rs`
- Test: `crates/mobile-web-server/tests/agent_chat.rs`

- [ ] Add `POST /api/sessions/{id}/agent-turn`.
- [ ] Store the user message as a session message.
- [ ] Call `AgentModel`.
- [ ] If the model proposes a known low-risk diagnostic command, execute through `DiagnosticService` or the shared `CommandRunner` with `requires_approval: false`.
- [ ] If the model proposes any other shell command, create a pending approval through `ApprovalService`.
- [ ] Broadcast assistant and tool events over SSE.
- [ ] Return `AgentTurnResponse`.
- [ ] Tests:
  - `请问当前运行在什么系统？` with mock model executes `uname -a` and returns assistant summary.
  - `请查看磁盘空间` executes `df -h`.
  - high-risk command proposal creates pending approval and does not execute.
  - model text-only answer creates no tool call.
- [ ] Run:
  ```bash
  cargo test -p deepseek-mobile-web-server agent_chat
  ```
- [ ] Commit:
  ```bash
  git commit -m "feat: execute mobile web agent chat turns"
  ```

**Subagent Prompt:**

```text
Implement POST /api/sessions/:id/agent-turn using the model abstraction and
existing SSH/approval services. Natural-language OS questions must not be
executed as shell text; they should become a low-risk diagnostic command.
```

## Task 4: Approval Continuation Summary

**Purpose:** After a user approves a high-risk AI-proposed command, show a useful assistant result instead of only raw stdout/stderr.

**Files:**
- Modify: `crates/mobile-web-server/src/approvals.rs`
- Modify: `crates/mobile-web-server/src/routes.rs`
- Modify: `crates/mobile-web-server/src/types.rs`
- Test: `crates/mobile-web-server/tests/agent_approval_summary.rs`

- [ ] Add enough metadata to pending approvals to know whether they came from an Agent turn.
- [ ] After `approve_once`, append an assistant message summarizing command result.
- [ ] Keep `reject` behavior clear: assistant message says the command was not run.
- [ ] Do not call the real model in tests; use deterministic summary text.
- [ ] Run:
  ```bash
  cargo test -p deepseek-mobile-web-server agent_approval_summary
  ```
- [ ] Commit:
  ```bash
  git commit -m "feat: summarize approved agent commands"
  ```

**Subagent Prompt:**

```text
Extend approval continuation for Agent-originated commands. Approved commands
should produce an assistant summary message; rejected commands should produce a
clear assistant message saying nothing was executed.
```

## Task 5: Mobile Web Chat UI

**Purpose:** Make AI chat the primary user interaction and move raw command execution into a secondary advanced section.

**Files:**
- Modify: `mobile-web/src/App.tsx`
- Modify: `mobile-web/src/api.ts`
- Modify: `mobile-web/src/types.ts`
- Modify: `mobile-web/src/state.ts`
- Modify: `mobile-web/src/styles.css`
- Test: `mobile-web/src/api.test.ts`
- Test: `mobile-web/src/state.test.ts`

- [ ] Add `sendAgentTurn(sessionId, message)` API client.
- [ ] Add a primary chat composer with input hint text `Ask the remote Linux device...`.
- [ ] Display user and assistant messages distinctly in the timeline.
- [ ] Keep Diagnostics and Advanced Command available, but visually secondary.
- [ ] When `agent-turn` returns pending approvals, render the same approval cards.
- [ ] The text `请问当前运行在什么系统？` must be submitted to Agent chat, not the raw command field.
- [ ] Run:
  ```bash
  cd mobile-web
  PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm test
  PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm run typecheck
  PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm run build
  ```
- [ ] Commit:
  ```bash
  git commit -m "feat: make mobile web ai chat primary"
  ```

**Subagent Prompt:**

```text
Update the mobile Web UI so AI chat is the primary workflow. The raw command
form must remain available but secondary. Add API/state tests for agent-turn.
```

## Task 6: AI Chat Script and Evidence Coverage

**Purpose:** Make the AI chat path reproducible from Linux without manual browser interaction.

**Files:**
- Create: `scripts/mobile_web_ai_chat_smoke.py`
- Create: `scripts/mobile_web_ai_chat_evidence.py` or extend `scripts/mobile_web_ssh_evidence.py`
- Test: `scripts/mobile_web_ai_chat_smoke_test.py`
- Modify: `docs/mobile-web-ssh-simulator.md`
- Modify: `docs/mobile-validation-checklists.md`

- [ ] Add a fake-server smoke proving `agent-turn` handles:
  - OS question -> completed answer.
  - high-risk request -> pending approval.
  - approval -> assistant summary.
- [ ] Add live script flags:
  - `--server`
  - `--access-token`
  - `--message`
  - `--auto-approve`
  - `--json`
- [ ] Ensure script output does not print DeepSeek API keys.
- [ ] Add evidence output rows that remain Linux/LAN Web evidence only.
- [ ] Run:
  ```bash
  python3 scripts/mobile_web_ai_chat_smoke_test.py
  python3 scripts/mobile_web_ssh_evidence_smoke.py
  ```
- [ ] Commit:
  ```bash
  git commit -m "test: add mobile web ai chat smoke flow"
  ```

**Subagent Prompt:**

```text
Add scriptable AI chat smoke/evidence coverage. It must prove natural language
goes through agent-turn, not the raw command executor. Do not require a real
DeepSeek API call in smoke tests.
```

## Task 7: Final Linux Verification and Docs

**Purpose:** Prove the Linux/Web/Rust/SSH AI Agent chain works without making real-platform claims.

**Files:**
- Modify: `docs/linux-first-mobile-task-tree.md`
- Modify: `docs/mobile-web-ssh-simulator.md`
- Modify: `docs/mobile-validation-checklists.md`
- Modify: `Makefile`

- [ ] Add Make targets:
  - `make mobile-web-ai-smoke`
  - `make mobile-web-ai-server`
- [ ] Run full verification:
  ```bash
  python3 scripts/mobile_linux_validation.py
  cargo fmt --all --check
  cargo test -p deepseek-mobile-agent-core -p kai-runner
  cargo test -p deepseek-mobile-web-server
  cargo clippy -p deepseek-mobile-agent-core -p kai-runner --all-targets --all-features
  cargo clippy -p deepseek-mobile-web-server --all-targets --all-features
  cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm test
  cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm run typecheck
  cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm run build
  python3 scripts/mobile_web_ai_chat_smoke.py --server http://127.0.0.1:8788 --message "请问当前运行在什么系统？" --json
  git diff --check -- docs crates/mobile-web-server scripts mobile-web Makefile Cargo.toml Cargo.lock
  ```
- [ ] Run one live server against `root@192.168.30.244` and verify the answer is not `not found`.
- [ ] Update docs to say the Linux Web simulator now proves AI chat -> SSH tool routing, not real iOS.
- [ ] Commit:
  ```bash
  git commit -m "docs: validate mobile web ai agent chat"
  ```

**Subagent Prompt:**

```text
Run final Linux verification for the mobile Web AI Agent chat path. Do not mark
real iOS/macOS/Windows evidence complete. The key acceptance criterion is that
Chinese natural language OS questions are answered through AI/tool routing and
are never executed as raw shell text.
```

---

## Recommended Subagent Dispatch for Next Turn

Dispatch these first:

```text
Agent A: Task 0 - Agent chat contract
Agent B: Task 1 - Read-only model config loader
Agent C: Task 5 - Web chat UI skeleton using a mocked API contract
Agent D: Task 6 - AI chat script smoke skeleton using a fake server
```

Then dispatch:

```text
Agent E: Task 2 - Agent model provider
Agent F: Task 3 - Agent turn executor
Agent G: Task 4 - Approval continuation summary
Parent/Agent H: Task 7 - final integration and verification
```

## Acceptance Criteria

- The Web UI primary input is AI chat, not raw shell.
- Typing `请问当前运行在什么系统？` produces an assistant answer and an SSH diagnostic/tool execution, not `ash: ... not found`.
- High-risk model-proposed commands require `approve_once` or `reject`.
- Approval replay remains rejected.
- DeepSeek API tokens are read-only and never logged.
- Linux verification passes.
- Real iOS/macOS/Windows claims remain unchanged.
