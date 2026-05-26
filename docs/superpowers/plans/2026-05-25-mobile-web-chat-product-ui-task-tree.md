# Mobile Web Chat Product UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete Mobile Web Chat product UI by replacing the test-style timeline-first surface with a phone-friendly Chat + Tool Activity experience.

**Architecture:** Keep the browser as an iOS-compatible thin client. The Rust server remains the owner of SSH, model config, tool policy, approvals, and message/tool contracts. The Web app derives product views from existing SSE/HTTP events: Chat shows user/assistant turns and final answers; Tool Activity shows command lifecycle with collapsed stdout/stderr; Timeline remains available as a secondary debug/report source.

**Tech Stack:** React 19, Vite, TypeScript, Vitest, Testing Library, existing CSS only, no new npm dependency unless explicitly approved.

---

## Final Objective

LF-L is complete when `mobile-web` provides a product-oriented mobile chat surface:

- The first useful screen is Agent Chat, not a raw test timeline.
- User messages and final assistant answers render as chat bubbles.
- The latest final answer is visually prominent and copyable.
- Tool execution moves to a separate Tool Activity area with one row per command.
- stdout/stderr and long command output are collapsed by default and can be expanded.
- Pending approvals remain explicit and server-driven.
- Copy final answer and copy full report both work, with manual-copy fallback still present.
- Existing diagnostics, advanced command, timeline, report, SSE, approval, smoke, and make entry points remain compatible.
- The UI remains Linux/Web evidence only and does not claim real iOS Safari/WebView or native iOS behavior.

## Task Tree

### LF-L0. Define Product UI Contract

- [x] Add this LF-L task tree and update `docs/linux-first-mobile-task-tree.md`.
- [x] Keep boundary language explicit: Linux/LAN Web UI only, no real iOS/macOS/Windows evidence.
- [x] Keep testing entry points intact for `make mobile-web-ai-server`, `make mobile-web-ai-smoke`, and existing smoke scripts.

### LF-L1. State Selectors For Product Views

Owned files:

- `mobile-web/src/state.ts`
- `mobile-web/src/state.test.ts`
- `mobile-web/src/types.ts` only if needed

Required behavior:

- [x] Derive `chatItems` from events without relying on the debug timeline.
- [x] Track user and assistant text messages separately from tool/audit/debug entries.
- [x] Derive `toolActivities` from `message.part.updated` tool parts and legacy tool events.
- [x] Deduplicate tool activity by stable part/tool id where available.
- [x] Preserve stdout/stderr in activity detail but do not duplicate it into the chat answer.
- [x] Derive `finalAnswer` from the latest assistant message that contains real assistant text.
- [x] Add report helpers for `buildFinalAnswerReport` and keep `buildFeedbackReport` compatible.

Tests:

- [x] Add RED tests proving duplicate stdout/tool events do not create duplicate activity rows.
- [x] Add RED tests proving final answer is selectable without including raw stdout twice.
- [x] Add RED tests proving legacy events still populate Tool Activity when typed parts are absent.
- [x] Run `cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm test -- --run src/state.test.ts`.

### LF-L2. Chat And Tool Activity Components

Owned files:

- Create `mobile-web/src/components/ChatView.tsx`
- Create `mobile-web/src/components/ToolActivity.tsx`
- Create `mobile-web/src/components/ProductPanels.test.tsx`

Required behavior:

- [x] Render empty, user, assistant, and final-answer states.
- [x] Render final answer with a copy command button controlled by parent callback.
- [x] Render Tool Activity as compact rows with status, command, exit code/duration when present.
- [x] Collapse stdout/stderr by default in `<details>`.
- [x] Show stderr with error styling.
- [x] Avoid any browser-side SSH, shell, file access, or API-key storage behavior.

Tests:

- [x] Add RED component tests for final answer rendering/copy button.
- [x] Add RED component tests for collapsed stdout/stderr details.
- [x] Run `cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm test -- --run src/components/ProductPanels.test.tsx`.

### LF-L3. App Integration And Mobile Layout

Owned files:

- `mobile-web/src/App.tsx`
- `mobile-web/src/App.test.tsx`
- `mobile-web/src/styles.css`

Required behavior:

- [x] Make Agent Chat the primary product surface.
- [x] Add a compact status header that preserves server/SSE/target visibility.
- [x] Place Pending Approvals near Chat and keep approval actions unchanged.
- [x] Move Diagnostics, Advanced Command, raw Timeline, and Recent Audit into secondary collapsible panels.
- [x] Add `Copy final answer` and keep `Copy report`.
- [x] Keep the copyable report textarea for mobile manual copy fallback.
- [x] Prevent layout/text overflow on phone-width screens.

Tests:

- [x] Add RED tests proving Chat appears before raw Timeline and final answer can be copied.
- [x] Add RED tests proving timeline/debug output is still reachable but secondary.
- [x] Run `cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm test -- --run src/App.test.tsx`.

### LF-L4. Compatibility And Validation

Required commands:

- [x] `cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm test`
- [x] `cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm run typecheck`
- [x] `cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm run build`
- [x] `python3 scripts/mobile_web_ai_chat_smoke_test.py`
- [x] `cargo fmt --all --check`
- [x] `git diff --check -- .gitignore docs ios/DeepSeekMobileDemo crates/kai-runner crates/mobile-agent-core crates/mobile-web-server mobile-web scripts Makefile Cargo.toml Cargo.lock`

Manual test, performed by the user, not claimed by agents unless provided:

- [ ] `HOST=0.0.0.0 PORT=8788 SSH_HOST=192.168.30.244 SSH_USER=root SSH_PORT=22 MODEL_MODE=auto make mobile-web-ai-server`
- [ ] Open `http://192.168.9.78:8788/`
- [ ] Ask a network/system diagnostic question.
- [ ] Approve pending commands.
- [ ] Confirm final answer is readable, copyable, and raw tool output is under Tool Activity.
