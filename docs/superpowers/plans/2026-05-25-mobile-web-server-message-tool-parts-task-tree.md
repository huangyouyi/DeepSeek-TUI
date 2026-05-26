# Mobile Web Server Message/Tool Parts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a stable, server-owned message/tool part contract so the mobile Web UI can render chat, tool execution, approval, and final answers from session messages instead of reconstructing them from timeline events.

**Architecture:** Keep the current `mobile-web-v1` API compatible by preserving `MessagePart { id, kind, text, data }` and existing `assistant_text`, `executed_tools`, `pending_approvals`, and legacy SSE events. Add a typed-part layer inside `data`, emit `message.part.updated` for tool lifecycle updates, and expose a `typed_message_parts` capability so the future product UI can progressively switch to message-driven rendering. Hot files (`routes.rs`, `approvals.rs`, `types.rs`) are sequenced; frontend tests, scripts, and docs can run in parallel once the Rust contract lands.

**Tech Stack:** Rust 1.88+, Axum, Serde/serde_json, Tokio broadcast SSE, React/Vite TypeScript, Vitest, Python smoke tests.

---

## Final Objective

LF-K is complete when the Linux mobile Web server is message-contract ready for a product chat UI:

```text
Agent turn or approval continuation
  -> server creates/updates durable session messages
  -> text answers stay renderable as text parts
  -> remote shell tool lifecycle is represented as compatible kind="tool" parts
  -> pending approval, approved result, rejected result, stdout/stderr, exit code, duration, timeout, and final summary are available from message parts
  -> SSE emits message.part.updated for incremental UI updates
  -> legacy HTTP fields and legacy SSE tool/approval events still work
  -> Web reducer/API tests and Python smoke tests prove new and old clients remain compatible
```

The stopping condition is not “the UI looks productized.” The stopping condition is that the server contract is strong enough for the next phase to build a product Chat UI without reading raw timeline events as the source of truth.

Status: completed on Linux Web contract evidence. This does not claim real iOS Safari/WebView behavior, native iOS app integration, macOS, Windows, real package install/update/uninstall, real browser automation, or user-run manual `make` testing.

### Completion Definition

- `/health` reports `typed_message_parts`.
- `GET /api/sessions/{id}/messages` returns messages whose parts include stable `kind="text"` and `kind="tool"` entries for agent tool flows.
- Low-risk model-selected SSH commands write completed tool parts with command, status, output, stderr, exit code, duration, and timeout metadata.
- High-risk model-selected SSH commands write pending-approval tool parts before execution.
- Approval `approve_once` writes approved/completed tool result parts and still returns `result.summary` / `result.assistant_text`.
- Approval `reject` writes rejected tool result parts and still appends a final assistant-visible summary.
- `message.part.updated` SSE events are emitted for tool part lifecycle updates.
- Existing `assistant_text`, `executed_tools`, `pending_approvals`, `tool.started`, `tool.stdout`, `tool.stderr`, `tool.completed`, `tool.failed`, `approval.asked`, and `approval.replied` remain compatible.
- No browser-side SSH, local process execution, local file access, or DeepSeek API key storage is introduced.
- No real iOS/macOS/Windows/package/browser-automation evidence is claimed.

### Required Evidence

The goal is achieved only after these commands pass:

```bash
cargo test -p deepseek-mobile-web-server --test agent_chat_contract --all-features
cargo test -p deepseek-mobile-web-server --test agent_chat --all-features
cargo test -p deepseek-mobile-web-server --test agent_approval_summary --all-features
cargo test -p deepseek-mobile-web-server --test diagnostics --all-features
cargo test -p deepseek-mobile-web-server --test event_stream --all-features
cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm test
cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm run typecheck
python3 scripts/mobile_web_ai_chat_smoke_test.py
cargo fmt --all --check
cargo clippy -p deepseek-mobile-web-server --all-targets --all-features
git diff --check -- docs ios/DeepSeekMobileDemo crates/kai-runner crates/mobile-agent-core crates/mobile-web-server mobile-web scripts Makefile Cargo.toml Cargo.lock
```

Full workspace validation is preferred before final handoff:

```bash
python3 scripts/mobile_linux_validation.py
cargo test -p deepseek-mobile-agent-core -p kai-runner
cargo test -p deepseek-mobile-web-server
cargo clippy -p deepseek-mobile-agent-core -p kai-runner --all-targets --all-features
```

## Contract Decision

Use a compatible typed-part payload instead of replacing the current message schema.

Text part JSON stays valid:

```json
{
  "id": "part-1",
  "kind": "text",
  "text": "answer",
  "data": {}
}
```

Tool part JSON is introduced:

```json
{
  "id": "part-tool-1",
  "kind": "tool",
  "data": {
    "turn_id": "turn-1",
    "agent_turn_id": "turn-1",
    "tool_call_id": "agent-call-1",
    "tool": "remote.shell.exec",
    "title": "Remote shell command",
    "status": "completed",
    "requires_approval": false,
    "command": "uname -a",
    "input": { "command": "uname -a" },
    "output": "Linux test-host ...",
    "stderr": "",
    "exit_code": 0,
    "duration_ms": 12,
    "timed_out": false
  }
}
```

Approval-required tool part JSON is introduced:

```json
{
  "id": "part-tool-approval-1",
  "kind": "tool",
  "data": {
    "turn_id": "turn-1",
    "agent_turn_id": "turn-1",
    "tool_call_id": "agent-call-1",
    "approval_id": "approval-1",
    "tool": "remote.shell.exec",
    "title": "Remote shell command",
    "status": "pending_approval",
    "requires_approval": true,
    "command": "opkg update",
    "input": { "command": "opkg update" }
  }
}
```

New SSE event:

```json
{
  "type": "message.part.updated",
  "payload": {
    "session_id": "session-1",
    "message_id": "message-1",
    "part": { "id": "part-tool-1", "kind": "tool", "data": {} }
  }
}
```

Legacy `tool.started`, `tool.stdout`, `tool.stderr`, `tool.completed`, `tool.failed`, `approval.asked`, and `approval.replied` stay valid during this phase.

## Parallelization Tree

```text
LF-K. Server Message/Tool Part Contract
├─ Wave 0: Contract design gate (parent only)
│  └─ Task 0. Freeze compatible schema and tests
├─ Wave 1: Rust contract core (sequential hotspot)
│  ├─ Task 1. Add typed part builders and health capability
│  ├─ Task 2. Write low-risk agent tool parts
│  └─ Task 3. Write approval-originated tool parts
├─ Wave 2: Parallel adapters after Rust contract compiles
│  ├─ Task 4A. Frontend API/types/state tests and reducer support
│  ├─ Task 4B. Python smoke helper compatibility
│  └─ Task 4C. Docs/task-tree update
├─ Wave 3: Integration
│  ├─ Task 5. End-to-end contract verification
│  └─ Task 6. Review, cleanup, and final validation
```

Parallel rule: no two workers may edit the same file. `routes.rs`, `approvals.rs`, and `types.rs` are owned by one Rust integration worker at a time. Frontend, scripts, and docs can run concurrently after Task 3.

---

## Task 0: Contract Tests First

**Files:**
- Modify: `crates/mobile-web-server/tests/agent_chat_contract.rs`
- Modify: `mobile-web/src/types.ts`

- [x] **Step 1: Add failing Rust serialization tests**

Add tests named:

```rust
#[test]
fn message_contract_serializes_compatible_text_and_tool_parts() {
    // Create a Message with one text part and one kind="tool" part.
    // Assert JSON keeps id/kind/text/data and includes tool status fields in data.
}

#[test]
fn health_contract_can_report_typed_message_parts_capability() {
    // Create HealthResponse with capabilities vec containing typed_message_parts.
    // Assert JSON includes capabilities and existing status/service/protocol/model fields.
}
```

- [x] **Step 2: Add TypeScript type shape**

Extend `MessagePart` in `mobile-web/src/types.ts` with typed helpers only; do not remove existing fields:

```ts
export type ToolPartData = {
  turn_id?: string;
  agent_turn_id?: string;
  tool_call_id?: string;
  approval_id?: string;
  tool?: string;
  title?: string;
  status?: "pending" | "running" | "completed" | "failed" | "error" | "pending_approval" | "rejected";
  requires_approval?: boolean;
  command?: string;
  input?: Record<string, unknown>;
  output?: string;
  stdout?: string;
  stderr?: string;
  exit_code?: number | null;
  duration_ms?: number;
  timed_out?: boolean;
};
```

- [x] **Step 3: Run failing tests**

Run:

```bash
cargo test -p deepseek-mobile-web-server --test agent_chat_contract --all-features
cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm run typecheck
```

Expected before implementation: Rust contract tests fail on missing capability or typed fixtures.

## Task 1: Typed Part Builders and Capability

**Files:**
- Modify: `crates/mobile-web-server/src/types.rs`
- Modify: `crates/mobile-web-server/src/routes.rs`
- Test: `crates/mobile-web-server/tests/agent_chat_contract.rs`

- [x] **Step 1: Add compatible Rust data structs**

Add serializable helper structs without changing existing `MessagePart` fields:

```rust
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolPartData {
    pub turn_id: String,
    pub agent_turn_id: String,
    pub tool_call_id: String,
    pub tool: String,
    pub title: String,
    pub status: String,
    pub requires_approval: bool,
    pub command: String,
    #[serde(default)]
    pub input: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timed_out: Option<bool>,
}
```

- [x] **Step 2: Add builder helpers**

Add helper constructors near current message helpers:

```rust
pub fn text_part(text: impl Into<String>, data: serde_json::Value) -> MessagePart;
pub fn tool_part(data: ToolPartData) -> MessagePart;
```

- [x] **Step 3: Add health capability**

Extend `HealthResponse` with:

```rust
#[serde(default, skip_serializing_if = "Vec::is_empty")]
pub capabilities: Vec<String>,
```

Return `typed_message_parts` from `/health`.

- [x] **Step 4: Verify**

Run:

```bash
cargo test -p deepseek-mobile-web-server --test agent_chat_contract --all-features
```

Expected: contract tests pass.

## Task 2: Low-Risk Agent Tool Parts

**Files:**
- Modify: `crates/mobile-web-server/src/routes.rs`
- Modify: `crates/mobile-web-server/src/state.rs`
- Test: `crates/mobile-web-server/tests/agent_chat.rs`

- [x] **Step 1: Add failing low-risk test**

Add a test that sends an agent turn which maps to `uname -a`, then asserts stored assistant messages include a `kind="tool"` part with:

```json
{
  "tool": "remote.shell.exec",
  "status": "completed",
  "requires_approval": false,
  "command": "uname -a",
  "exit_code": 0
}
```

- [x] **Step 2: Add state update helper**

Add an `AppState` method:

```rust
pub fn push_message_part(&self, session_id: &str, message_id: &str, part: MessagePart) -> bool
```

It should append to the matching message and return `false` if no matching message exists.

- [x] **Step 3: Create assistant message before tool execution**

In `agent_turn`, create one assistant message for the turn, then append/update tool parts on that message instead of only broadcasting tool events.

- [x] **Step 4: Emit `message.part.updated`**

When a low-risk tool starts and completes, broadcast:

```rust
broadcast_event(
    &state.app,
    "message.part.updated",
    json!({
        "session_id": session_id,
        "message_id": assistant_message_id,
        "part": tool_part,
    }),
);
```

Keep legacy tool events.

- [x] **Step 5: Verify**

Run:

```bash
cargo test -p deepseek-mobile-web-server --test agent_chat --all-features
cargo test -p deepseek-mobile-web-server --test diagnostics --all-features
```

Expected: new tool part tests pass; existing diagnostics event tests still pass.

## Task 3: Approval-Originated Tool Parts

**Files:**
- Modify: `crates/mobile-web-server/src/approvals.rs`
- Modify: `crates/mobile-web-server/src/routes.rs`
- Modify: `crates/mobile-web-server/src/types.rs`
- Test: `crates/mobile-web-server/tests/agent_approval_summary.rs`
- Test: `crates/mobile-web-server/tests/agent_chat.rs`

- [x] **Step 1: Add pending approval tool part test**

For a high-risk agent command, assert the assistant message contains `kind="tool"` with:

```json
{
  "status": "pending_approval",
  "requires_approval": true,
  "approval_id": "approval-...",
  "agent_turn_id": "turn-..."
}
```

- [x] **Step 2: Add approved result part test**

After `approve_once`, assert a tool part or tool-result part includes `status="completed"`, redacted stdout/stderr fields, `exit_code`, `duration_ms`, and `timed_out`.

- [x] **Step 3: Add rejected result part test**

After `reject`, assert a tool part includes `status="rejected"` and the final assistant summary still exists as a text part.

- [x] **Step 4: Implement append helpers**

Move shared tool-part construction into one helper usable by both `routes.rs` and `approvals.rs`. Do not duplicate JSON field names.

- [x] **Step 5: Verify**

Run:

```bash
cargo test -p deepseek-mobile-web-server --test agent_chat --all-features
cargo test -p deepseek-mobile-web-server --test agent_approval_summary --all-features
```

Expected: approval continuation still returns `result.summary` / `result.assistant_text`; stored messages also contain typed tool parts.

## Task 4A: Frontend Contract Adapter

**Files:**
- Modify: `mobile-web/src/types.ts`
- Modify: `mobile-web/src/state.ts`
- Modify: `mobile-web/src/api.test.ts`
- Modify: `mobile-web/src/state.test.ts`

- [x] Add `ToolPartData` and helpers to read tool command/status/output from `part.data`.
- [x] Update reducer handling for `message.part.updated` so tool parts produce timeline entries without duplicate spam.
- [x] Keep current `approval.asked` flow as the source of pending approval cards.
- [x] Add tests for text part rendering, tool part timeline rendering, and part-id dedupe.
- [x] Run:

```bash
cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm test -- --run src/api.test.ts src/state.test.ts
cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm run typecheck
```

## Task 4B: Python Smoke Compatibility

**Files:**
- Modify: `scripts/mobile_web_ai_chat_smoke_test.py`
- Modify: `scripts/mobile_web_ai_chat_smoke.py`

- [x] Extend fake payloads with `message.part.updated` / typed tool parts.
- [x] Keep old `assistant_text` and approval summary extraction path.
- [x] Add assertions that new typed tool parts do not leak API keys or access tokens.
- [x] Run:

```bash
python3 scripts/mobile_web_ai_chat_smoke_test.py
```

## Task 4C: Docs Task Tree

**Files:**
- Modify: `docs/linux-first-mobile-task-tree.md`
- Modify: `docs/mobile-web-ssh-simulator.md` if API examples need updating
- Modify: `docs/mobile-validation-checklists.md` only if evidence wording changes

- [x] Add LF-K as an active Linux-only productization track.
- [x] State clearly that typed server message/tool parts are Linux Web contract evidence only.
- [x] Do not mark real iOS Safari/WebView, iOS app, macOS, Windows, package install, or browser automation evidence complete.

## Task 5: Integration Verification

**Files:**
- No planned source edits unless tests reveal integration bugs.

- [x] Run targeted Rust tests:

```bash
cargo test -p deepseek-mobile-web-server --test agent_chat_contract --all-features
cargo test -p deepseek-mobile-web-server --test agent_chat --all-features
cargo test -p deepseek-mobile-web-server --test agent_approval_summary --all-features
cargo test -p deepseek-mobile-web-server --test diagnostics --all-features
cargo test -p deepseek-mobile-web-server --test event_stream --all-features
```

- [x] Run frontend tests:

```bash
cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm test
cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm run typecheck
cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm run build
```

- [x] Run smoke:

```bash
python3 scripts/mobile_web_ai_chat_smoke_test.py
```

## Task 6: Final Validation

**Files:**
- No planned source edits unless validation fails.

- [x] Run:

```bash
python3 scripts/mobile_linux_validation.py
cargo fmt --all --check
cargo test -p deepseek-mobile-agent-core -p kai-runner
cargo test -p deepseek-mobile-web-server
cargo clippy -p deepseek-mobile-agent-core -p kai-runner --all-targets --all-features
cargo clippy -p deepseek-mobile-web-server --all-targets --all-features
git diff --check -- docs ios/DeepSeekMobileDemo crates/kai-runner crates/mobile-agent-core crates/mobile-web-server mobile-web scripts Makefile Cargo.toml Cargo.lock
```

- [x] Confirm no output includes DeepSeek API keys, bearer tokens, or access tokens.

## Subagent Execution Plan

Use these workers after Task 0 is accepted:

```text
Worker Rust-Contract:
  Owns crates/mobile-web-server/src/types.rs, routes.rs, state.rs, approvals.rs and Rust tests.
  Implements Tasks 1-3 sequentially. No frontend/script edits.

Worker Frontend-Adapter:
  Starts after Rust-Contract publishes the final JSON shape.
  Owns mobile-web/src/types.ts, state.ts, api.test.ts, state.test.ts.

Worker Smoke-Scripts:
  Starts after Rust-Contract publishes fake JSON examples.
  Owns scripts/mobile_web_ai_chat_smoke.py and scripts/mobile_web_ai_chat_smoke_test.py.

Worker Docs:
  Can start immediately from this plan.
  Owns docs/linux-first-mobile-task-tree.md and related docs only.
```

Workers are not alone in the codebase. They must not revert unrelated edits and must adapt to already-landed changes.

## Self-Review

- Spec coverage: covers typed part schema, low-risk tools, pending approvals, approval continuation, frontend adapter, smoke tests, docs, and validation.
- Placeholder scan: no `TBD` or open-ended implementation placeholders remain.
- Type consistency: use `kind="tool"` with typed payload in `data`; `turn_id` and legacy `agent_turn_id` both present during migration.
