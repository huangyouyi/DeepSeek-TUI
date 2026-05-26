# Mobile Web Reliable Multi-Turn Agent Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete Phase 4: reliable multi-turn Agent behavior for `/web`, so one session can continue diagnosis with bounded context, deterministic fallback, and stop/retry/continue controls.

**Architecture:** The Rust server remains the source of truth for model context, SSH/tool policy, approvals, and audit. The browser sends user intent only; it does not construct model context or decide tool permissions. Session history is converted into a compact server-owned context pack before model calls, preserving final summaries and tool result summaries while bounding long stdout/stderr.

**Tech Stack:** Rust/Axum server, existing `AgentModel` abstraction, OpenAI-compatible DeepSeek chat completions, React/Vite mobile Web UI, Vitest, Rust integration tests, Python smoke tests.

---

## Phase 4 Task Tree

### Task 1: Server-Owned Multi-Turn Context Contract

**Files:**
- Modify: `crates/mobile-web-server/src/agent_model.rs`
- Modify: `crates/mobile-web-server/src/routes.rs`
- Modify: `crates/mobile-web-server/src/types.rs`
- Test: `crates/mobile-web-server/tests/agent_model.rs`
- Test: `crates/mobile-web-server/tests/agent_chat_contract.rs`
- Test: `crates/mobile-web-server/tests/agent_chat.rs`

- [ ] Add `AgentContextMessage { role, content }` to `agent_model.rs`.
- [ ] Extend `AgentModelRequest` with `context: Vec<AgentContextMessage>`.
- [ ] Build model requests in `routes.rs` with compact context from `state.app.messages(session_id)` before the current user message is persisted.
- [ ] Include recent user text, assistant final summaries, and tool result summaries.
- [ ] Never send raw long stdout/stderr to the model context; summarize with command, status, exit code, line/char counts, and bounded head/tail snippets.
- [ ] Update `DeepSeekAgentModel::build_http_request` so `/chat/completions` receives system prompt, compact session context, and the new user message.
- [ ] Update `MockAgentModel` so follow-up prompts like `继续`, `刚才`, or `continue` can use context to choose a relevant diagnostic command.
- [ ] Add tests proving previous-turn context is included and long stdout is summarized rather than copied wholesale.

### Task 2: Deterministic Fallback And Context Safety

**Files:**
- Modify: `crates/mobile-web-server/src/routes.rs`
- Modify: `crates/mobile-web-server/src/agent_model.rs`
- Test: `crates/mobile-web-server/tests/agent_chat.rs`
- Test: `crates/mobile-web-server/tests/agent_model.rs`

- [ ] Replace hard model failure as the only behavior with deterministic fallback text in `agent_turn`.
- [ ] On model failure, return a successful `AgentTurnResponse` with status `model_fallback`, no tool calls, and an assistant message explaining the model failed while summarizing known session context.
- [ ] Redact the model error and never include API keys.
- [ ] Keep the existing redaction tests passing.
- [ ] Add tests for fallback after a previous tool result and for fallback with no prior context.

### Task 3: Stop, Retry, And Continue Server API

**Files:**
- Modify: `crates/mobile-web-server/src/types.rs`
- Modify: `crates/mobile-web-server/src/routes.rs`
- Test: `crates/mobile-web-server/tests/agent_chat_contract.rs`
- Test: `crates/mobile-web-server/tests/agent_chat.rs`
- Test: `crates/mobile-web-server/tests/http_api.rs`

- [ ] Extend `AgentTurnRequest` with optional `mode: "normal" | "retry" | "continue"` and optional `retry_turn_id`.
- [ ] For `retry`, reuse the most recent user message in the session when the request message is empty.
- [ ] For `continue`, use the provided message or a server default like `继续分析上一轮结果`.
- [ ] Add `POST /api/sessions/{id}/agent-turns/{turn_id}/stop`.
- [ ] Stop must remove pending approvals for that turn, append a deterministic assistant summary, emit `message.updated` and `assistant.completed`, and audit the stop.
- [ ] State clearly in tests that this stop controls waiting/pending work and does not claim to kill an already running OS process.

### Task 4: `/web` Product Controls For Multi-Turn Work

**Files:**
- Modify: `mobile-web/src/api.ts`
- Modify: `mobile-web/src/types.ts`
- Modify: `mobile-web/src/App.tsx`
- Modify: `mobile-web/src/components/ProductShell.tsx`
- Modify: `mobile-web/src/styles.css`
- Test: `mobile-web/src/api.test.ts`
- Test: `mobile-web/src/App.test.tsx`
- Test: `mobile-web/src/components/ProductShell.test.tsx`

- [ ] Add client API helpers for `sendAgentTurn(..., { mode, retryTurnId })` and `stopAgentTurn(sessionId, turnId)`.
- [ ] Track the latest turn id from `AgentTurnResponse`, `assistant.started`, and pending approval metadata.
- [ ] Add product controls: `继续`, `重试`, and `停止`.
- [ ] `继续` sends mode `continue` with a short default continuation prompt if the composer is empty.
- [ ] `重试` sends mode `retry` for the latest user turn or latest assistant turn context.
- [ ] `停止` calls the stop endpoint when a turn is waiting/running and disables itself when idle.
- [ ] Preserve `/debug` compatibility and existing approval buttons.

### Task 5: Smoke, Docs, And Evidence Boundary

**Files:**
- Modify: `scripts/mobile_web_ai_chat_smoke.py`
- Modify: `scripts/mobile_web_ai_chat_smoke_test.py`
- Modify: `docs/linux-first-mobile-task-tree.md`
- Test: `python3 scripts/mobile_web_ai_chat_smoke_test.py`

- [ ] Extend fake smoke server to exercise a two-turn session where the second turn depends on previous context.
- [ ] Add smoke coverage for `continue`, `retry`, and `stop` contracts without real DeepSeek calls.
- [ ] Update `docs/linux-first-mobile-task-tree.md` with LF-P Phase 4 and keep real-platform evidence boundaries explicit.
- [ ] Do not claim real iOS/macOS/Windows/browser automation/package install evidence.

## Verification Gate

- [ ] `cargo fmt --all --check`
- [ ] `cargo test -p deepseek-mobile-web-server --all-features`
- [ ] `cargo clippy -p deepseek-mobile-web-server --all-targets --all-features`
- [ ] `cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm test`
- [ ] `cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm run typecheck`
- [ ] `cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm run build`
- [ ] `python3 scripts/mobile_web_ai_chat_smoke_test.py`
- [ ] `git diff --check -- .gitignore docs ios/DeepSeekMobileDemo crates/kai-runner crates/mobile-agent-core crates/mobile-web-server mobile-web scripts Makefile Cargo.toml Cargo.lock`

## Acceptance Boundary

- [ ] User can ask a follow-up like `刚才那个网络问题继续帮我看` in the same session and the server-side model request includes compact prior context.
- [ ] Prior final summaries and tool summaries can guide the next turn.
- [ ] Long stdout/stderr remains visible in Tool Activity but is summarized before model input.
- [ ] Model failures return deterministic assistant fallback without leaking secrets.
- [ ] `/web` exposes stop/retry/continue controls and `/debug` remains available.
- [ ] Manual LAN/phone validation remains user-run after this implementation.
