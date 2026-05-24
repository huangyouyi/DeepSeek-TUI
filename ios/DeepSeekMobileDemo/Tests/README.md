# DeepSeek Mobile Demo Testing Notes

Broad automated XCTest coverage is intentionally deferred until the production
`MobileCoreBridge` and UniFFI bindings exist. The package includes a small
model-level XCTest scaffold for runner request auth metadata, shell approval
nonce handling, command lease approval metadata, browser click approval state,
and CoreBridge JSON adapter pairing upgrade behavior. Pairing upgrade tests
cover provider call-site wiring, token redaction from runner profile JSON,
capabilities mapping, and the mock fallback path; continue to validate the
SwiftUI shell manually from Xcode with the mock bridge.

## Toolchain Checks

On macOS with the Swift/Xcode toolchain installed, run the package verification
entry point before the manual simulator pass:

```bash
ios/DeepSeekMobileDemo/Scripts/verify-macos
```

This runs `swift package describe` and `swift build` for the current SwiftUI demo
package. On Linux, this check is expected to stop with a clear macOS-toolchain
message; Linux CI/dev can only verify the Rust crates until the iOS demo is wired
to a macOS Swift toolchain and the Rust mobile core is linked into the app.

## Runner Command Lease Audit Contract

`IosMobileAgentCore.runner_recent_audit_json(session_id, response_json)` is the
Rust-side contract for runner `/mobile/audit/recent` command lease entries. For
`shell.command_lease` records, the returned `audit_entries` array must expose
only the fields the current Swift `AuditTimelineEntry` presentation expects:

- `kind`: `runner_command_lease`
- `action`: one of `accepted`, `consumed`, `replay_rejected`,
  `expired_rejected`, or `invalid_action_rejected`
- `summary`: `command lease <action>`
- `call_id`: the runner tool call id
- `detail`: a display-safe key/value string such as
  `lease replay rejected call_id=... tool=... error_code=...`

Swift timeline parsing intentionally whitelists only `call_id`, `tool`, and
`error_code` from that `detail` text. Rust must not emit bearer tokens, pairing
tokens, raw lease ids, idempotency keys/secrets, command text, env names, env
values, nonce material, or raw credential values in this UniFFI audit payload.
Linux verification covers this in
`cargo test -p deepseek-mobile-agent-core --test uniffi_api`; Swift compilation
remains macOS-only.

## Manual UI Pass

1. Open `ios/DeepSeekMobileDemo/Package.swift` in Xcode on macOS.
2. Run the `DeepSeekMobileDemo` executable target on an iOS simulator.
3. Confirm the session list loads with the demo sessions.
4. Select each session and confirm chat history, pending approval state, and
   unread counts render correctly.
5. Send a message and confirm the mock assistant response appears.
6. Open the pending command approval sheet, confirm the approval nonce label is
   pending/redacted, and confirm the command lease id, idempotency key, expiry,
   approved action summary, and one-time-before-expiry warning are visible
   without bearer, pairing token, nonce secret, or command secret material.
   Approve and deny in separate runs to confirm state and audit events update.
7. Open connection setup, edit settings, save, and confirm an audit entry is
   added.
8. Switch to Runner or Remote MCP mode, tap Discover, and confirm the
   documented tool count/list appears in setup, the chat header shows the count,
   and a capability discovery audit entry is added.
9. In Runner or Remote MCP mode, confirm `Runner Request Auth` shows `Unpaired`,
   the selected endpoint, no token account, and `Token: Not present`.
10. Tap Request Pairing Code, confirm a short code
   appears, redeem it, and confirm the status changes from unpaired to the demo
   credential label without displaying a raw token.
11. In `CoreBridge Pairing Upgrade`, enter an endpoint, a throwaway pairing
   token, and capabilities JSON; tap `Upgrade Pairing` and confirm the token
   field clears immediately.
12. Confirm the visible runner profile shows only a sanitized account label,
   credential label, fallback/CoreBridge status, and capability summaries. Raw
   pairing token text and the `pairing_token` request field must not appear in
   the form, profile, auth metadata, or audit log.
13. Repeat with the mock fallback provider path if available and confirm valid
   capabilities JSON still populates discovered runner capabilities.
14. Confirm `Runner Request Auth` shows `Ready`, the selected endpoint, the demo
   token account label, `Token: Present`, and no approval nonce before command
   approval without displaying the raw token.
15. Confirm the documented capabilities appear immediately after redeem without
   a separate Discover tap, and the chat header shows the discovered count.
16. Return to the selected chat session and confirm a browser session card
   appears after pairing/capability discovery with a redacted `browser-...`
   session id, status, page title/URL, extracted text preview, and pending click
   approval nonce status.
17. Open `Review Browser Click` and confirm the sheet is visually distinct from
   shell command approval: it is labeled as browser approval and shows the click
   target, URL, rationale, and redacted click nonce state.
18. Approve the browser click and confirm the browser card changes to click
   approved with a redacted `click-nonce-...` label. Repeat from a fresh pairing
   run and deny the click to confirm the nonce is not issued.
19. Approve the pending command, reopen connection setup, and confirm `Runner
   Request Auth` shows approval nonce presence with a redacted nonce label and
   bound status for the next mock runner shell request.
20. Repeat from a fresh run and deny the pending command; confirm request auth
   metadata does not report an approval nonce.
21. Open the audit log and confirm the pairing request, credential storage,
   pairing redemption, and capability discovery entries appear in reverse
   chronological order as separate events, with approved command entries
   recording nonce issued/bound status and denied command entries recording no
   nonce issuance. Confirm command lease audit rows show nonce issued, lease
   accepted, lease consumed, replay rejected, expired rejected, and invalid
   rejected lifecycle states, with only `call_id`, `tool`, and `error_code`
   metadata visible. Command approval UI and audit text must not display bearer
   tokens, pairing tokens, nonce secrets, raw lease ids, idempotency secrets,
   command text, env values, or raw credential values.
22. Confirm browser audit entries appear separately for browser session opened,
   text extracted, click approval requested, and click approval issued or denied.
   No raw token, raw shell nonce, or raw browser click nonce should be visible.
23. Paste bootstrap output, import it, and confirm the character count and stored
   settings update.
24. Open the audit log and confirm recent actions appear in reverse chronological
   order.

When XCTest coverage is added, prefer testing `MobileCoreBridge` adapters and
view models first so SwiftUI views remain thin over deterministic state.
