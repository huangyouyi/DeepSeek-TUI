# Mobile Web Opencode UI Parity Design

## Goal

Make `/web` feel and behave like `/home/cu/projects/istore-ai-helper/web` while keeping this project's Rust server, SSH target configuration, approval policy, and remote command execution as the security boundary. `/debug` remains the raw testing and diagnostics entry point.

The product goal is not a small visual polish pass. The `/web` route should become the formal chat UI for real LAN/Linux trials, with one primary conversation content area and opencode-style rendering for messages, tool activity, authorization requests, errors, and assistant output.

## Non-Goals

- Do not make the Rust server pretend to be the opencode HTTP API.
- Do not move SSH, command execution, model calls, or approval enforcement into the browser.
- Do not remove `/debug` or its existing raw testing affordances.
- Do not add third-party services, analytics, hosted endpoints, telemetry, branding, or dependencies beyond local UI/runtime libraries needed for the chat experience.

## Current State

`mobile-web` already has the core backend-facing API surface:

- Sessions: `GET /api/sessions`, `POST /api/sessions`, `GET /api/sessions/{id}/messages`
- Agent turns: `POST /api/sessions/{id}/agent-turn`
- Stop: `POST /api/sessions/{id}/agent-turns/{turn_id}/stop`
- Approvals: `POST /api/approvals/{id}/respond`
- SSH target: `GET/PUT /api/ssh/target`, `POST /api/ssh/check`
- Events: `/event`

The current `/web` route uses `ProductShell` with a separate tool activity panel. This makes tool state feel detached from the conversation, prints long command information too prominently, and diverges from the reference app's message-flow experience.

The reference app has a better interaction model:

- Conversation sidebar with new/delete/select behavior
- Header with status, stop, and settings menu
- Welcome page with suggested prompts
- Auto-growing input composer with send/stop behavior
- Markdown-rendered assistant messages
- Inline permission cards
- Inline collapsible tool cards
- Toast-based error and status feedback
- URL session synchronization

## Architecture

Use an adapter-based UI architecture:

1. Keep the existing Rust server API and safety model.
2. Add a frontend adapter that maps this project's `SessionSummary`, `Message`, `ChatItem`, `PendingApproval`, and `ToolActivity` into opencode-style UI view models.
3. Reuse or port the reference app's chat components against those view models.
4. Render all conversational artifacts in a single timeline inside the main content area.

This keeps API compatibility work local to the frontend, avoids server protocol churn, and allows the UI to match the reference app without weakening the remote execution boundary.

## Routes

### `/web`

`/web` is the formal product route. It should use the opencode-style chat shell:

- Sidebar/drawer for conversations only
- Header for status and settings
- One scrollable conversation content area
- Bottom composer
- Toast notifications
- Remote SSH settings dialog

Tool activity, approvals, and execution summaries appear inline in the conversation area.

### `/debug`

`/debug` remains a testing surface. It can keep raw timelines, diagnostics, manual command preparation, audit details, and verbose state. It is acceptable for `/debug` to look different from `/web`.

## Main UI Layout

The `/web` shell mirrors the reference app:

- Root: full-height dark chat surface.
- Left sidebar on desktop: session list, new conversation, delete controls, session count.
- Mobile sidebar: overlay drawer controlled by the header menu button.
- Header: app identity, online/connection state, message count, stop button, settings menu.
- Content: one centered `max-w-4xl` scrollable message column.
- Composer: sticky bottom input with auto-resize, Enter-to-send, Shift+Enter newline, send button that becomes stop while an agent turn is active.

There is no separate right-side or lower tool activity region in `/web`.

## Timeline Model

The frontend should derive a single sorted timeline for the active session. Timeline entries include:

- User message
- Assistant message
- Pending approval card
- Tool activity card
- Final answer card when distinct from regular assistant text
- Loading bubble while waiting for a response
- Error card when a message or turn fails

Each entry carries `createdAtMs`, `sessionId`, and a stable id. Items must be filtered to the active session.

The current `ToolActivity` collection should not be rendered as a separate panel in `/web`. It should be converted into inline tool cards and inserted according to its timestamp. If event ordering is ambiguous, the UI should place tool cards after the latest assistant/user turn associated with the tool's `agent_turn_id` or after the latest visible message in the active session.

## Data Adapter

Create a dedicated adapter module rather than mixing mapping logic into React components.

### Session Mapping

Map:

- `SessionSummary.id` -> `Conversation.id`
- `SessionSummary.title` -> `Conversation.title`
- `SessionSummary.created_at_ms` -> `Conversation.createdAt`
- `SessionSummary.updated_at_ms` -> `Conversation.updatedAt`

### Message Mapping

Map project messages and local chat items into reference-style session messages:

- `role: "user"` and `role: "assistant"` map directly.
- Plain text becomes a `text` part.
- Assistant markdown must render through the same sanitized markdown path as the reference app.
- Empty assistant messages during an active turn should render the reference loading bubble, not a blank block.

### Approval Mapping

Map `PendingApproval` to an inline permission card:

- `id` -> permission id
- `session_id` -> `sessionID`
- `created_at_ms` -> `time.created`
- `command` -> `pattern` and `metadata.command`
- `cwd` -> `metadata.cwd`
- `target` and `target_label` -> `metadata.target`
- `risk_reason` -> card explanation
- Default `type` -> `bash`
- Default title -> `Bash 命令执行请求`

The buttons map as:

- `仅这次执行` -> `approve_once`
- `本会话都允许` -> `approve_session`
- `拒绝并停止` -> `reject_stop`

If the server later emits non-shell approval kinds such as `external_directory`, the adapter should map those to the same inline permission component without changing the shell layout.

### Tool Mapping

Map `ToolActivity` to an inline tool card:

- `command` -> `state.input.command`
- `status` -> `state.status`
- `stdout`/`stderr`/`output` -> output sections
- `exitCode` -> metadata exit code
- `durationMs` -> metadata duration
- `requiresApproval` -> metadata flag

The visual behavior matches the reference app:

- Pending and running tools are expanded by default.
- Completed tools are collapsed by default.
- Failed tools show a warning state and expose error output.
- Long commands are shown as a compact command block inside the card, not as a full-width page-level panel.

## Session Behavior

The `/web` route should restore the reference session behavior:

1. On load, call `listSessions`.
2. If the URL has `?session=<id>` and that session exists, select it.
3. Otherwise select the most recent session if present.
4. If no session exists, show the welcome screen without eagerly creating a session.
5. On first send without an active session, create a session using a title hint from the first user message.
6. When a session is selected, load its messages via `listMessages`.
7. Keep `?session=<id>` synchronized with the active session.
8. New conversation creates and selects a new placeholder session.
9. Delete conversation removes a session and selects the next available one.

This requires adding or confirming backend support for:

- `DELETE /api/sessions/{id}`
- `PATCH /api/sessions/{id}` or equivalent title update endpoint

If those endpoints are not available, the first implementation phase may hide delete and persist title hints locally, but the product target includes server-backed delete and title update.

## Remote Settings

The reference app's model settings dialog should not be copied as-is. Replace it with a project-specific remote settings dialog:

- SSH host
- SSH user
- SSH port
- Save target
- Check SSH
- Server status
- SSE status
- Service/model display
- Current target display
- Access token entry only if token auth is enabled for the deployment

This dialog is opened from the header menu, preserving the reference interaction pattern while reflecting this product's remote-first requirement.

## Text Rendering

Assistant output should not be dumped as raw plain text. Use the reference markdown pipeline:

- `marked` for markdown parsing
- `DOMPurify` for sanitization
- Reference markdown styles for code, lists, headings, and preformatted output

Tool stdout/stderr remains preformatted inside collapsible tool cards. General assistant explanations use markdown rendering.

## Error And Status Handling

Use toast notifications for transient errors and status updates:

- Session load failure
- Send failure
- Approval response failure
- SSH target save/check failure
- Clipboard failure

Inline error cards are reserved for errors that belong to a specific message, tool, or permission.

Global fixed banners should be removed from `/web` unless they represent a blocking connectivity state. `/debug` may keep raw banners.

## Missing Feature Inventory

The implementation should check and fill these gaps:

- Session list loading in `/web`
- Active session message loading
- URL query synchronization
- First-message title hints
- Session deletion
- Session title update
- Markdown rendering
- Toast notifications
- Inline permission cards
- Inline tool cards
- Loading bubble while sending
- Send/stop composer behavior
- Mobile sidebar overlay parity
- Remote settings dialog
- Clipboard behavior for final answer/report if still needed
- Tests for `/web` and `/debug` split

## Testing Strategy

Frontend tests should verify:

- `/web` loads sessions and does not eagerly create a session when sessions are absent.
- Sending the first message creates a session, sends the agent turn, inserts the user message, and updates the URL.
- Selecting a conversation loads its messages.
- Pending approvals render inline in the content area with the three expected actions.
- Approval buttons call the existing approval API with the correct action.
- Tool activity renders inline and completed tools are collapsed by default.
- `/web` does not render the old tool activity side panel.
- `/debug` still renders the raw/testing interface.
- SSH target settings remain available from `/web`.
- Long commands do not overflow the mobile viewport.

Backend tests should be added only for endpoints that are missing:

- Delete session removes it from the list and message store.
- Update session title persists and appears in list sessions.

Manual verification should cover:

- Desktop width around 1440px.
- Mobile widths around 375px, 390px, and 430px.
- Long command approval cards.
- Long stdout/stderr tool output.
- SSE reconnect and transient error display.
- `/debug` unchanged enough for diagnostics.

## Rollout Plan

Implement in phases:

1. Add adapter and opencode-style render components while preserving existing APIs.
2. Replace `/web` shell with the reference-style chat layout.
3. Move approvals and tools into the single timeline.
4. Restore session lifecycle parity.
5. Add backend endpoints for delete/title if missing.
6. Polish mobile layout and long-content behavior.
7. Verify `/debug` remains the testing entry point.

Each phase should leave `/web` usable and `/debug` available.

## Acceptance Criteria

- `/web` visually and behaviorally resembles `/home/cu/projects/istore-ai-helper/web` for chat, sessions, composer, message rendering, permissions, and tool cards.
- The only intentional product difference is remote SSH/Linux target configuration and Rust-server-controlled command execution.
- `/web` has one primary content area; no separate tool activity panel is visible.
- Pending approvals render inline with `仅这次执行`, `本会话都允许`, and `拒绝并停止`.
- Tool output is inline and collapsible.
- Assistant text renders as sanitized markdown.
- New session, select session, stop, and URL session sync behave like the reference app.
- `/debug` remains available for raw testing workflows.
- High-risk actions still require Rust server approval and are never executed directly by the browser.
