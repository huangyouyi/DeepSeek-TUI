# Mobile Web Product Entry MVP Task Tree

**Goal:** Make `/web` the formal Mobile Web product entry, keep `/debug` as the test/debug entry, expose SSH target configuration in the product flow, and make first-pass remote diagnosis the primary path.

**Architecture:** The browser remains a thin client. React renders chat, settings, approvals, and report views; the Rust server owns model configuration, API keys, SSH execution, tool policy, approvals, static serving, and SSE/REST contracts. `/web` and `/debug` both serve the same SPA bundle, but the client route decides whether the product or debug surface is shown.

**Tech Stack:** Rust, Axum, tower-http static services, React, Vite, TypeScript, existing Mobile Web HTTP/SSE APIs.

---

## Scope Boundary

This task tree covers Linux/LAN Web evidence only.

The browser thin client may:

- Render `/web` product chat, SSH target settings, server status, approvals, final answer, and report output.
- Render `/debug` diagnostics, advanced command, raw timeline, audit, and compatibility testing controls.
- Call server REST/SSE APIs and display server-owned session/message/tool state.

The browser thin client must not:

- Read `~/.deepseek/config.toml` or display API keys.
- Execute SSH, shell, package, browser automation, file, or local process work directly.
- Decide command risk, bypass approval, persist privileged host credentials, or claim platform evidence.

This stage explicitly excludes:

- Approval v2.
- Multi-turn/multi-round context beyond the current server message/tool contract.
- Real iOS Safari/WebView, native iOS, macOS, or Windows evidence.
- Real package install, update, or uninstall execution.
- Real browser automation engine execution.

## Product Path

The intended first-use path is:

1. Open `/web`.
2. Review server/SSE status and current SSH target.
3. Configure SSH host, user, and port in product settings.
4. Run Check SSH from the product surface.
5. Ask for a first remote diagnosis in chat.
6. Let the server route low-risk diagnostics through SSH policy.
7. Approve only explicit high-risk commands when the server creates an approval.
8. Read and copy the final assistant answer/report.

`/debug` remains available for diagnostics, raw timeline inspection, audit review, smoke-test compatibility, and advanced command testing.

## Task Tree

### M1.0. Entry Route Contract

- [done] `/web` is the formal product entry.
- [done] `/debug` is the retained test/debug entry.
- [done] `/` remains compatible with the SPA/static bundle.
- [done] `/web` and `/debug` exact paths return `index.html` from the Rust static server.
- [done] `/web/...` and `/debug/...` deep links return `index.html` from the Rust static server.
- [done] `/health`, `/event`, and `/api/*` are not replaced by static fallback.

Evidence:

- `crates/mobile-web-server/tests/static_serving.rs`
- `crates/mobile-web-server/src/routes.rs`
- `cargo test -p deepseek-mobile-web-server --test static_serving --all-features`

### M1.1. Product `/web` Chat Shell

- [done] `/web` presents the product Chat-first shell instead of the debug console.
- [done] Product chat renders user text, assistant text, final answer, tool activity, and pending approvals from server-owned message/tool state.
- [done] First-screen product flow prioritizes remote diagnosis over raw timeline inspection.
- [done] Product UI keeps raw diagnostics/debug surfaces secondary.

Evidence:

- Existing Mobile Web React route/product-shell implementation and tests owned by the frontend workers.
- This document records Linux/LAN Web scope only.

### M1.2. Debug `/debug` Preservation

- [done] `/debug` keeps Check SSH, Diagnostics, Advanced Command, raw Timeline, recent Audit, Copy report, and smoke-test compatibility surfaces.
- [done] `/debug` remains available for scripted and manual investigation.
- [done] `/debug` does not become the advertised product entry.

Evidence:

- Existing Mobile Web debug entry implementation and tests owned by the frontend workers.

### M1.3. SSH Target Configuration

- [done] Product settings expose SSH host, user, port, key-present/server status, and Check SSH.
- [done] SSH target updates continue through server APIs.
- [done] The browser never receives or displays DeepSeek API keys.
- [done] Risk policy and SSH execution remain server-side.

Evidence:

- `GET /api/ssh/target`
- `PUT /api/ssh/target`
- `POST /api/ssh/check`
- Product UI tests owned by the frontend workers.

### M1.4. First Remote Diagnosis Main Path

- [done] Product chat supports a natural-language first diagnosis request.
- [done] Low-risk diagnostic tool calls execute through the server-side SSH runner path.
- [done] High-risk commands produce explicit approvals before execution.
- [done] Approved command results return to assistant-visible message/tool parts.
- [done] Final answer/report is copyable from the product flow.

Evidence:

- Existing server message/tool part contract tests.
- Existing AI chat smoke compatibility tests.
- Manual phone/LAN validation remains pending/manual.

### M1.5. Browser Thin-Client Boundary

- [done] Server owns model config loading and never exposes API keys to the browser.
- [done] Server owns tool policy, approval creation, SSH execution, audit, and message/tool persistence.
- [done] Browser renders server state and sends user/approval/target intents only.
- [done] Documentation states this is Linux/LAN Web evidence only.

Evidence:

- This task tree.
- `docs/linux-first-mobile-task-tree.md`

### M1.6. Static Serving And API Safety

- [done] `/web` and `/debug` exact routes serve the SPA index.
- [done] `/web/...` and `/debug/...` deep links serve the SPA index.
- [done] Static asset paths such as `/asset.txt` still serve assets.
- [done] `/health` still returns the health API response.
- [done] Existing API routes such as `/api/sessions` still return API responses.
- [done] Missing API routes return `404` rather than `index.html`.

Evidence:

- `cargo test -p deepseek-mobile-web-server --test static_serving --all-features`

### M1.7. Explicit Non-Goals

- [done] Approval v2 is out of scope.
- [done] Multi-round context expansion is out of scope.
- [done] Real iOS Safari/WebView, native iOS, macOS, and Windows evidence are out of scope.
- [done] Real package install/update/uninstall execution is out of scope.
- [done] Real browser automation engine execution is out of scope.

Evidence:

- This task tree.
- `docs/linux-first-mobile-task-tree.md`

### M1.8. Validation And Manual Handoff

- [done] `cargo test -p deepseek-mobile-web-server --test static_serving --all-features`
- [done] `cargo fmt --all --check`
- [pending/manual] Start the LAN server with the maintainer-selected host/port/SSH target.
- [pending/manual] Open `/web` from a phone browser on the same LAN.
- [pending/manual] Configure/check the SSH target from `/web`.
- [pending/manual] Run the first remote diagnosis path through chat.
- [pending/manual] Approve any explicit high-risk command and confirm the final answer/report.
- [pending/manual] Open `/debug` from the same phone and confirm debug fallback surfaces remain available.

Manual evidence must not be described as real iOS/macOS/Windows evidence unless the maintainer provides a separate platform evidence bundle.
