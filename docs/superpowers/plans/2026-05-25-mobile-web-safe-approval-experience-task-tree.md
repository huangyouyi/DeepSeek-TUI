# Mobile Web Safe Approval Experience Task Tree

**Stage:** Phase 2 safe approval experience

**Goal:** Make mobile Web approvals explicit, auditable, and safe for repeated
phone/LAN use while preserving the existing Linux-first evidence boundary.

**Evidence boundary:** This task tree is Linux/LAN Web contract work only. It
does not include real iOS, macOS, or Windows evidence; real package
install/update/uninstall; real browser automation; or real DeepSeek/SSH smoke
execution. Manual LAN/phone verification remains a user-run handoff and must
not be marked done until the user supplies accepted evidence.

## Current Status

- [x] S2.0 Goal and boundary documented.
- [x] S2.1 Backend approval v2 contract.
- [x] S2.2 Server-side session allow policy.
- [x] S2.3 `reject_stop` turn stop semantics.
- [x] S2.4 Enriched approval metadata and audit.
- [x] S2.5 `/web` three-option authorization card.
- [x] S2.6 `/debug` test-entry compatibility.
- [x] S2.7 Contract, unit, and smoke tests.
- [ ] S2.8 Manual LAN handoff pending.

## Task Tree

```text
Phase 2 Safe Approval Experience
├─ [done] S2.0. Goal and boundary
│  ├─ [done] S2.0.1. Define the phase as safe authorization UX and contract hardening for Linux/LAN Web only
│  ├─ [done] S2.0.2. State that "allow for this session" is a server-side exact command/cwd/session scope
│  ├─ [done] S2.0.3. State that this is not a frontend local-cache permission model
│  └─ [done] S2.0.4. Preserve non-goals: no real iOS/macOS/Windows evidence, no real package install/update/uninstall, no real browser automation, no real DeepSeek or SSH smoke execution
│
├─ [done] S2.1. Backend approval v2 contract
│  ├─ [done] S2.1.1. Accept response aliases for approve once, allow this session, reject, and reject/stop
│  ├─ [done] S2.1.2. Return stable JSON fields that old clients can ignore and new clients can render
│  ├─ [done] S2.1.3. Keep unsupported response values rejected with a clear error
│  └─ [done] S2.1.4. Preserve existing `approve_once` and `reject` behavior for smoke and debug clients
│
├─ [done] S2.2. Server-side session allow policy
│  ├─ [done] S2.2.1. Store "allow for this session" on the server, never only in browser state
│  ├─ [done] S2.2.2. Scope allow entries to exact `session_id`, exact command string, exact cwd, and the active server process policy
│  ├─ [done] S2.2.3. Require approval for high-risk commands when no exact server-side session allow entry matches
│  ├─ [done] S2.2.4. Avoid broad pattern, prefix, shell-equivalent, or cross-session grants
│  └─ [done] S2.2.5. Audit allow creation and allow hit/miss decisions without leaking secrets
│
├─ [done] S2.3. `reject_stop` turn stop semantics
│  ├─ [done] S2.3.1. Distinguish plain reject from reject-and-stop in the approval response contract
│  ├─ [done] S2.3.2. Mark the current agent turn stopped when `reject_stop` is selected
│  ├─ [done] S2.3.3. Prevent follow-up tool execution for the stopped turn after the rejection
│  └─ [done] S2.3.4. Return an assistant-visible summary that explains the user stopped the turn
│
├─ [done] S2.4. Enriched approval metadata and audit
│  ├─ [done] S2.4.1. Include risk reason, command, cwd, session id, turn id, and proposed action in the approval payload
│  ├─ [done] S2.4.2. Keep exact session allow decisions server-side and do not let the browser grant them
│  ├─ [done] S2.4.3. Record response type, allow scope, and stopped-turn status in audit metadata
│  └─ [done] S2.4.4. Keep API keys, bearer tokens, approval nonces, lease secrets, idempotency keys, and environment secrets out of browser-visible output
│
├─ [done] S2.5. `/web` three-option authorization card
│  ├─ [done] S2.5.1. Render three clear actions: approve once, allow for this session, and reject/stop
│  ├─ [done] S2.5.2. Explain the allow scope in UI copy without implying client-side persistence
│  ├─ [done] S2.5.3. Keep approval controls touch-sized and safe for mobile browsers
│  └─ [done] S2.5.4. Keep high-risk command execution unavailable from browser-only state
│
├─ [done] S2.6. `/debug` test-entry compatibility
│  ├─ [done] S2.6.1. Preserve existing `approve_once` and `reject` debug flows
│  ├─ [done] S2.6.2. Expose any new response aliases in a testable way without breaking existing scripts
│  ├─ [done] S2.6.3. Keep raw diagnostic, command, timeline, and audit debug panels available
│  └─ [done] S2.6.4. Preserve access-token behavior and server-side policy ownership
│
├─ [done] S2.7. Contract, unit, and smoke tests
│  ├─ [done] S2.7.1. Add Rust contract/unit tests for response aliases, exact session allow hit/miss, high-risk miss requiring approval, and `reject_stop`
│  ├─ [done] S2.7.2. Add React tests for the `/web` authorization card and `/debug` compatibility
│  ├─ [done] S2.7.3. Keep Python smoke on a fake server and avoid real DeepSeek or real SSH
│  ├─ [done] S2.7.4. Add Python smoke compatibility for approval response aliases
│  └─ [done] S2.7.5. Run final targeted verification after backend and frontend workers land their changes
│
└─ [pending/manual] S2.8. Manual LAN handoff pending
   ├─ [pending/manual] S2.8.1. User starts the LAN server and opens `/web` from a phone browser
   ├─ [pending/manual] S2.8.2. User verifies approve once, allow for this session, and reject/stop against a disposable target
   ├─ [pending/manual] S2.8.3. User confirms `/debug` still works for test fallback
   └─ [pending/manual] S2.8.4. Do not mark this done without accepted user-supplied LAN/phone evidence
```

## Acceptance Boundaries

- "Allow for this session" means a server-side exact command/cwd/session scope.
  It is not a frontend local cache, not a browser-only remember flag, and not a
  broad command-pattern grant.
- High-risk commands that do not hit an exact server-side session allow entry
  still require approval.
- Existing clients that send `approve_once` or `reject` must keep working.
- New response aliases must be contract-tested without requiring real DeepSeek,
  real SSH, or real browser automation.
- This phase must not claim real iOS Safari/WebView, native iOS app, macOS,
  Windows, real package install/update/uninstall, or real browser automation
  evidence.
- Manual LAN/phone verification is explicitly pending until the user executes
  it and provides evidence.

## Verification

Run after integrating backend/frontend changes:

```bash
python3 scripts/mobile_web_ai_chat_smoke_test.py
cargo fmt --all --check
git diff --check -- docs scripts mobile-web crates/mobile-web-server
```
