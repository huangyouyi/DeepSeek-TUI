# Mobile Web `/web` Product UI And `/debug` Split Task Tree

**Goal:** Complete the Mobile Web dual-entry restructure: keep the current testing/debugging surface at `/debug`, add `/web` as the product Chat UI entry aligned with `/home/cu/projects/istore-ai-helper/web`, add product-level SSH target/server configuration, and preserve thin-client/server-policy/Linux-first boundaries.

**Boundary:** This proves Linux/LAN Web UI behavior only. It does not claim real iOS Safari/WebView, native iOS, macOS, Windows, real browser automation, or real install/update/uninstall behavior.

## Task Tree

### LF-M0. Route Contract

- [x] `/web` renders the product Chat UI.
- [x] `/debug` renders the current testing/debug UI.
- [x] `/` redirects or resolves to `/web`.
- [x] Rust static serving supports `/web` and `/debug` deep links to Vite `index.html`.
- [x] `/health`, `/event`, and `/api/*` continue to be API routes.

### LF-M1. Debug Entry Preservation

- [x] Preserve current Check SSH, Diagnostics, Advanced Command, Raw Timeline, Copy report, and copyable report behavior under `/debug`.
- [x] Keep existing smoke/test entry points compatible.
- [x] Keep debug UI secondary to product UI.

### LF-M2. Product Shell

- [x] Add a product Chat shell aligned with the reference app: sidebar/conversation rail, header, central chat stream, bottom composer.
- [x] Use product interaction patterns from `istore-ai-helper/web`, adapted to this repo without browser-side SSH/shell/file/API-key behavior.
- [x] Add empty/welcome state with suggested prompts.
- [x] Keep mobile-first layout and desktop compatibility.

### LF-M3. Product SSH/Server Settings

- [x] Product UI exposes server/SSE/target status in the header.
- [x] Product UI provides SSH host/user/port configuration through a settings-style panel.
- [x] Product UI can run Check SSH.
- [x] DeepSeek API key is never displayed; only server-side status may be shown.

### LF-M4. Product Chat, Tool, Approval Flow

- [x] User/assistant text appears in the product chat stream.
- [x] Tool execution appears as inline/collapsed tool cards, not raw timeline noise.
- [x] stdout/stderr/output are collapsed by default.
- [x] Pending approvals appear inline with Approve once / Reject actions.
- [x] Final answer is prominent and copyable.
- [x] Waiting-for-approval text is not treated as final answer.
- [x] REST + SSE echo dedupe remains intact.

### LF-M5. Tests

- [x] React tests cover `/web` product shell.
- [x] React tests cover `/debug` debug entry.
- [x] React tests cover product SSH settings save/check.
- [x] React/state tests cover approval/final answer/tool details behavior.
- [x] Rust tests cover static `/web` and `/debug` deep links.

### LF-M6. Validation

- [x] `cargo test -p deepseek-mobile-web-server --test http_api --all-features`
- [x] `cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm test`
- [x] `cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm run typecheck`
- [x] `cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm run build`
- [x] `python3 scripts/mobile_web_ai_chat_smoke_test.py`
- [x] `cargo fmt --all --check`
- [x] `git diff --check -- .gitignore docs ios/DeepSeekMobileDemo crates/kai-runner crates/mobile-agent-core crates/mobile-web-server mobile-web scripts Makefile Cargo.toml Cargo.lock`

### LF-M7. Manual Handoff

- [ ] User runs `HOST=0.0.0.0 PORT=8788 SSH_HOST=192.168.30.244 SSH_USER=root SSH_PORT=22 MODEL_MODE=auto make mobile-web-ai-server`.
- [ ] User opens `http://192.168.9.78:8788/web`.
- [ ] User opens `http://192.168.9.78:8788/debug`.
- [ ] User validates SSH target settings, approval flow, final answer, and debug fallback.
