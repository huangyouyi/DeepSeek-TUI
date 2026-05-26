# Mobile Web Comfortable Chat + Tool Activity Task Tree

> **For agentic workers:** Phase 3 is a Linux-first Web UI polish phase. Keep manual phone/LAN validation pending unless accepted user evidence is provided.

**Goal:** Make the product Chat experience comfortable enough for daily Linux/LAN Web use while keeping detailed tool execution in a separate Tool Activity area for advanced users.

**Boundary:** This phase is implementation and automated Linux Web verification only. It does not include real iOS/macOS/Windows evidence, real browser automation evidence, or real package install/update/uninstall evidence.

## Final Objective

Phase 3 is complete when:

- Chat remains the primary product workflow.
- Tool Activity is a separate advanced detail area, not the main conversation.
- Each tool row shows status, command title/command, exit code, duration, and approval marking when present.
- stdout, stderr, and output are collapsed by default with line/length summaries.
- Long output is held in a bounded scrollable pre container so it cannot flood the page.
- Empty Tool Activity clearly says execution details will appear there.
- Linux component tests cover the behavior.
- Manual LAN/phone validation remains explicitly pending/manual until user-run evidence exists.

## Task Tree

### S3.0. Phase 3 Goal And Boundary

- [x] Document Phase 3 as comfortable Chat + Tool Activity polish for Linux/LAN Web.
- [x] State that real iOS/macOS/Windows evidence is not included.
- [x] State that real browser automation is not included.
- [x] State that real package install/update/uninstall is not included.

### S3.1. Tool Activity Row Metadata

- [x] Show product-readable status for `running`, `pending_approval`, `completed`, `failed`, `rejected`, and `stopped`.
- [x] Preserve the raw status in the status chip `title`.
- [x] Show command title when provided and command text when present.
- [x] Show exit code and duration when present.
- [x] Show an approval marker when the activity requires approval.

### S3.2. Collapsed Output Defaults

- [x] Keep stdout collapsed by default.
- [x] Keep stderr collapsed by default.
- [x] Keep generic output collapsed by default.
- [x] Add summary text with line count and character count for each stream.

### S3.3. Long Output Containment

- [x] Render output inside `pre.tool-activity__stream`.
- [x] Add max-height and overflow scrolling so long output does not expand the whole layout.
- [x] Keep stderr visually distinct.

### S3.4. Empty State

- [x] Replace the generic empty text with a product explanation that tool execution details will appear here.

### S3.5. Automated Component Coverage

- [x] Test default-collapsed stdout/stderr.
- [x] Test long output remains present inside the pre container.
- [x] Test status labels.
- [x] Test the empty state.

### S3.6. Linux Verification

- [x] Run `cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm test -- ProductPanels.test.tsx`.
- [x] Run `cargo fmt --all --check`.
- [x] Run `git diff --check -- docs mobile-web crates/mobile-web-server scripts`.

### S3.7. Manual LAN/Phone Validation

- [ ] Start the LAN Web server from Linux.
- [ ] Open the product Web UI from a phone browser on the LAN.
- [ ] Run a chat request that creates tool activity.
- [ ] Confirm Tool Activity remains readable with collapsed output on a phone screen.
- [ ] Confirm long output scrolls inside the detail container.

### S3.8. Acceptance Notes

- [x] Implementation and automated Linux component verification are done for Tool Activity behavior.
- [ ] Manual LAN/phone validation is pending/manual.
- [ ] No real iOS Safari/WebView evidence is claimed.
- [ ] No real macOS or Windows host evidence is claimed.
- [ ] No real browser automation evidence is claimed.
- [ ] No real package install/update/uninstall evidence is claimed.
