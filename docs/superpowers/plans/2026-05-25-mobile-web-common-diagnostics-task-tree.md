# Mobile Web Common Diagnostics Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete Phase 5: common troubleshooting diagnostics for ordinary users, available from natural-language chat and product quick entries.

**Architecture:** Rust server owns diagnostic intent routing, low-risk command catalog, SSH execution, conclusion summaries, and policy. `/web` exposes common user-facing entry points that only send natural-language prompts to the server; it never executes commands or decides risk. Existing `/debug` preset diagnostics remain available for testing.

**Tech Stack:** Rust/Axum server, existing `AgentModel` abstraction, existing diagnostic preset catalog, React/Vite product shell, Vitest, Rust tests, Python fake smoke tests.

---

## Phase 5 Task Tree

### Task 1: Common Diagnostic Command Catalog

**Files:**
- Modify: `crates/mobile-web-server/src/diagnostics.rs`
- Modify: `crates/mobile-web-server/src/agent_model.rs`
- Test: `crates/mobile-web-server/tests/diagnostics.rs`
- Test: `crates/mobile-web-server/tests/agent_model.rs`
- Test: `crates/mobile-web-server/tests/agent_tool_policy.rs`

- [ ] Add low-risk diagnostic presets for: `dns`, `cpu_memory`, `services`, `docker`, `openwrt_network`, and `logs`.
- [ ] Keep existing presets stable: `system_info`, `current_user`, `disk_usage`, `memory`, `network`, `working_directory`.
- [ ] Use read-only commands only; no package install/update, no service restart, no destructive changes.
- [ ] Update `MockAgentModel` natural-language routing for Chinese and English prompts covering system, network, DNS, disk, memory/CPU, service, Docker, OpenWrt/router, and logs.
- [ ] Ensure all new mock-routed commands exactly match diagnostic presets so `AgentToolPolicy` treats them as low-risk.
- [ ] Add tests that each priority scenario maps to the expected command and that new preset commands are allowed by policy.

### Task 2: Conclusion-First Assistant Summaries

**Files:**
- Modify: `crates/mobile-web-server/src/routes.rs`
- Test: `crates/mobile-web-server/tests/agent_chat.rs`

- [ ] Replace generic `Command completed + stdout dump` assistant text for known diagnostic commands with deterministic conclusion summaries.
- [ ] Include a short conclusion line such as `结论：DNS 看起来正常。` or `结论：磁盘空间需要关注。`.
- [ ] Keep stdout/stderr in Tool Activity message parts for advanced users.
- [ ] Keep long outputs out of Chat final text; Chat should summarize and point to Tool Activity for details.
- [ ] Add tests for DNS, disk, Docker/log summaries, and a fallback summary for unknown commands.

### Task 3: `/web` Common Diagnostic Entries

**Files:**
- Modify: `mobile-web/src/components/ProductShell.tsx`
- Modify: `mobile-web/src/components/ProductShell.test.tsx`
- Modify: `mobile-web/src/styles.css`

- [ ] Add a product “常用诊断” prompt grid with entries for system, network, DNS, disk, memory/CPU, services, Docker, OpenWrt/router, and logs.
- [ ] Clicking an entry fills the composer with a natural-language prompt; it does not call SSH, APIs, or policy directly.
- [ ] Keep mobile tap targets comfortable and text non-overlapping.
- [ ] Preserve existing continue/retry/stop controls.
- [ ] Add component tests for the new prompt grid.

### Task 4: Smoke And Documentation

**Files:**
- Modify: `scripts/mobile_web_ai_chat_smoke.py`
- Modify: `scripts/mobile_web_ai_chat_smoke_test.py`
- Modify: `docs/linux-first-mobile-task-tree.md`

- [ ] Extend fake smoke coverage for at least DNS and Docker/OpenWrt/log natural-language turns.
- [ ] Keep fake smoke local: no real DeepSeek, no real SSH.
- [ ] Add LF-Q / Phase 5 docs with Linux/LAN Web boundary and manual phone validation pending.
- [ ] Do not claim real iOS/macOS/Windows/browser automation/package install evidence.

## Verification Gate

- [ ] `cargo fmt --all --check`
- [ ] `cargo test -p deepseek-mobile-web-server --all-features`
- [ ] `cargo clippy -p deepseek-mobile-web-server --all-targets --all-features`
- [ ] `cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm test`
- [ ] `cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm run typecheck`
- [ ] `cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm run build`
- [ ] `python3 scripts/mobile_web_ai_chat_smoke_test.py`
- [ ] `python3 scripts/mobile_linux_validation.py`
- [ ] `git diff --check -- .gitignore docs ios/DeepSeekMobileDemo crates/kai-runner crates/mobile-agent-core crates/mobile-web-server mobile-web scripts Makefile Cargo.toml Cargo.lock`

## Acceptance Boundary

- [ ] Natural-language prompts route to appropriate server-side diagnostic commands.
- [ ] `/web` exposes common diagnostic entries without browser-side SSH or local execution.
- [ ] Chat answer gives a conclusion first; raw command output remains in Tool Activity.
- [ ] New diagnostics are read-only and low-risk under server policy.
- [ ] Manual LAN/phone validation remains user-run after implementation.
