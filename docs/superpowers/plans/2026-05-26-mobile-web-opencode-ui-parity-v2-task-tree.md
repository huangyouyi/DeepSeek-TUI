# Mobile Web Opencode UI Parity V2 Task Tree

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` to execute this plan. Dispatch one fresh subagent per task, with spec review and code-quality review before marking a task complete.

**Goal:** Make `/web` closely match `/home/cu/projects/istore-ai-helper/web` as the formal chat UI, while preserving this project's Rust-server-controlled remote SSH/Linux execution, approvals, audit, and safety boundary. Keep `/debug` as the raw testing surface.

**Architecture:** Continue from the completed foundation work. The Rust server remains the source of truth for sessions, SSH target, command execution, approvals, audit, SSE, and model/tool policy. The browser adapts server state into opencode-like UI view models and renders one main conversation timeline containing user messages, assistant messages, inline authorization cards, inline tool cards, loading states, and errors.

**Tech Stack:** Rust 1.88+, Axum, React 19, Vite, TypeScript, Vitest, Testing Library, CSS, `lucide-react`, `sonner`, `marked@^15`, `dompurify`.

---

## Confirmed Product Target

`/web` should be the official user-facing mobile web product:

- One primary chat content area, not a separate tool activity panel.
- Same interaction feel as `/home/cu/projects/istore-ai-helper/web`: sidebar conversations, header, welcome page, markdown message rendering, auto-growing composer, stop/send behavior, inline permissions, inline tools, toast feedback.
- Remote SSH target settings are the intentional product-specific difference.
- Browser never runs shell/SSH/local file/model-secret work directly.
- High-risk actions remain controlled by the Rust server approval policy.
- `/debug` remains available for diagnostics, raw timeline, audit, manual command preparation, and test workflows.

## Current Baseline

The first foundation wave is already complete and reviewed:

- Backend session lifecycle:
  - `DELETE /api/sessions/{id}`
  - `PATCH /api/sessions/{id}`
  - session lifecycle audit/SSE
- Frontend API helpers:
  - `updateSessionTitle`
  - `deleteSession`
- Opencode adapter:
  - conversations
  - permission mapping
  - active-session timeline inputs
  - tool/message view models
- Markdown/style foundation:
  - `renderMarkdown`
  - local markdown/message/permission CSS
  - `marked` pinned to Node 18-compatible major
- Standalone shell components:
  - `OpencodeChatHeader`
  - `OpencodeChatInput`
  - `OpencodeSidebar`
  - `OpencodeWelcomePage`

Before executing this V2 plan, the parent must inspect the working tree. At the time this plan was written, one untracked file existed from an interrupted old Wave 2 worker:

- `mobile-web/src/components/InlinePermissionMessage.test.tsx`

Do not assume this file is correct. The Task A1 worker should either reuse it after inspection or replace it as part of its owned files.

## Parallelization Rules

No two workers may edit the same file at the same time.

Hotspot files, sequential only:

- `mobile-web/src/App.tsx`
- `mobile-web/src/App.test.tsx`
- `mobile-web/src/state.ts`
- `mobile-web/src/types.ts`
- `mobile-web/src/styles.css`
- `mobile-web/package.json`
- `mobile-web/package-lock.json`
- `crates/mobile-web-server/src/routes.rs`
- `crates/mobile-web-server/src/state.rs`
- `crates/mobile-web-server/src/types.rs`

Component tasks may run in parallel only when their file ownership is disjoint.

Review policy:

- Each implementation task must commit its own changes.
- Each implementation task gets spec review first.
- Only after spec review passes, dispatch code-quality review.
- If reviewer feedback conflicts with this plan, evaluate technically before implementing.

## V2 Parallelization Tree

```text
MW-O-V2. Opencode UI Parity From Completed Foundation
├─ Gate 0: Parent cleanup and baseline verification
│  └─ Task 0. Confirm current tree and pause stale Wave 2 artifacts
├─ Wave A: Parallel leaf components
│  ├─ Task A1. Inline permission card
│  ├─ Task A2. Inline chat message and tool renderer
│  ├─ Task A3. Remote settings dialog
│  └─ Task A4. Toast/status utility wrapper
├─ Wave B: Parallel state/session foundations
│  ├─ Task B1. Session deleted event and reducer support
│  ├─ Task B2. Product session controller hook
│  └─ Task B3. Product timeline selector tests
├─ Wave C: Sequential /web integration
│  └─ Task C1. Replace /web shell and preserve /debug
├─ Wave D: Parallel post-integration hardening
│  ├─ Task D1. Mobile layout and overflow polish
│  ├─ Task D2. Smoke/test updates
│  └─ Task D3. Documentation updates
└─ Wave E: Parent final review
   └─ Task E1. Full verification and handoff
```

Dependency rules:

- Gate 0 runs first.
- Wave A and Wave B may run in parallel after Gate 0.
- Task C1 starts only after all Wave A and Wave B tasks pass review.
- Wave D starts only after Task C1 passes tests and typecheck.
- Task E1 is parent-only.

---

## Task 0: Parent Cleanup And Baseline Verification

**Owner:** Parent only.

**Files:**

- No planned code files.
- May update this plan only if the baseline changed before execution starts.

**Goal:** Ensure no stale interrupted worker artifacts will confuse V2 workers.

- [ ] **Step 1: Inspect worktree**

Run:

```bash
git status --short
```

Expected: either clean, or only known stale files from interrupted old Wave 2 work.

- [ ] **Step 2: Inspect stale untracked files**

If `mobile-web/src/components/InlinePermissionMessage.test.tsx` exists, inspect it:

```bash
sed -n '1,240p' mobile-web/src/components/InlinePermissionMessage.test.tsx
```

If it is useful, leave it for Task A1. If it is broken partial work, Task A1 owns replacing it. Do not silently delete it in the parent unless it is clearly empty or generated junk.

- [ ] **Step 3: Run baseline checks**

Run:

```bash
cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm test -- --run src/api.test.ts src/opencodeAdapter.test.ts src/markdown.test.ts src/components/OpencodeShell.test.tsx
cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm run typecheck
cargo test -p deepseek-mobile-web-server --test http_api --all-features
cargo fmt --all --check
```

Expected: pass before starting V2 work.

---

## Task A1: Inline Permission Card

**Owner:** UI component worker.

**Files:**

- Create/replace: `mobile-web/src/components/InlinePermissionMessage.tsx`
- Create/replace: `mobile-web/src/components/InlinePermissionMessage.test.tsx`

**Do not edit:**

- `mobile-web/src/App.tsx`
- `mobile-web/src/api.ts`
- `mobile-web/src/state.ts`
- `mobile-web/src/styles.css`

**Goal:** Render `PermissionLike` as an opencode-style inline authorization card in the chat timeline.

**Implementation contract:**

```ts
import type { PermissionLike } from "../opencodeAdapter";

export type InlinePermissionMessageProps = {
  permission: PermissionLike;
  onRespond: (response: "once" | "always" | "reject") => Promise<void>;
};
```

Required behavior:

- Render `需要授权`.
- Render permission type, session id, formatted creation time, title.
- For `bash`, prefix each non-empty command line with `$ `.
- Show command/pattern in a compact code block.
- Show `permission.metadata.risk_reason` when present.
- Otherwise show: `即将执行 shell 命令，可能对系统产生影响。请确认命令内容后再授权。`
- Buttons:
  - `仅这次执行` -> `onRespond("once")`
  - `本会话都允许` -> `onRespond("always")`
  - `拒绝并停止` -> `onRespond("reject")`
- Disable all buttons while a response is in flight.
- If `onRespond` rejects, render an inline error block.
- Import `mobile-web/src/styles/permission-alert.css` from the component or keep equivalent local classes. Do not import it globally.

Tests:

- Renders bash approval with labels, session id, title, `$ command`, and three buttons.
- Clicking `本会话都允许` calls `onRespond("always")`.
- Rejected `onRespond` shows an error.
- Buttons are disabled while a pending response is unresolved.

Verification:

```bash
cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm test -- --run src/components/InlinePermissionMessage.test.tsx
cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm run typecheck
```

Commit:

```bash
git add mobile-web/src/components/InlinePermissionMessage.tsx mobile-web/src/components/InlinePermissionMessage.test.tsx
git commit -m "feat: render approvals inline in chat"
```

---

## Task A2: Inline Chat Message And Tool Renderer

**Owner:** UI component worker.

**Files:**

- Create: `mobile-web/src/components/OpencodeChatMessage.tsx`
- Create: `mobile-web/src/components/OpencodeChatMessage.test.tsx`

**Do not edit:**

- `mobile-web/src/App.tsx`
- `mobile-web/src/state.ts`
- `mobile-web/src/styles.css`
- `mobile-web/src/components/InlinePermissionMessage.tsx`

**Goal:** Render text/reasoning/tool parts from `SessionMessageLike` using opencode-style message bubbles and collapsible tool cards.

**Implementation contract:**

```ts
import type { SessionMessageLike, ToolPartLike } from "../opencodeAdapter";

export type OpencodeChatMessageProps = {
  message: SessionMessageLike;
};
```

Required behavior:

- User message aligns right.
- Assistant message aligns left.
- Text parts render sanitized markdown through `renderMarkdown`.
- Reasoning parts render dimmed.
- Tool parts render cards.
- Completed tools are collapsed by default.
- Pending/running/started/queued/in_progress tools are expanded by default.
- Error/rejected tool states expose error output and warning styling.
- Tool header shows command or title, status icon/text, exit code and duration when available.
- Long code/output scrolls inside the card.
- Assistant message with no renderable content shows a small loading bubble.
- Import `markdown.css` and `message.css` from the component, not globally.

Tests:

- Assistant markdown renders `<strong>` for `**完成**`.
- Completed tool output is hidden initially and appears after click.
- Pending/running tool output area is expanded by default.
- Error tool shows error text.
- User message receives user alignment/class.
- Empty assistant message renders loading bubble.

Verification:

```bash
cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm test -- --run src/components/OpencodeChatMessage.test.tsx
cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm run typecheck
```

Commit:

```bash
git add mobile-web/src/components/OpencodeChatMessage.tsx mobile-web/src/components/OpencodeChatMessage.test.tsx
git commit -m "feat: add opencode-style chat message renderer"
```

---

## Task A3: Remote Settings Dialog

**Owner:** UI component worker.

**Files:**

- Create: `mobile-web/src/components/RemoteSettingsDialog.tsx`
- Create: `mobile-web/src/components/RemoteSettingsDialog.test.tsx`

**Do not edit:**

- `mobile-web/src/App.tsx`
- `mobile-web/src/api.ts`
- `mobile-web/src/styles.css`

**Goal:** Provide the reference-style settings dialog interaction, but with this product's remote SSH target configuration.

Suggested props:

```ts
import type { SshCheckResponse, SshTarget } from "../types";

export type RemoteSettingsDialogProps = {
  open: boolean;
  target?: SshTarget | null;
  status: {
    server: string;
    sse: string;
    service: string;
    model: string;
    targetLabel?: string;
  };
  sshCheck?: SshCheckResponse | null;
  busy?: boolean;
  onOpenChange: (open: boolean) => void;
  onSaveTarget: (input: { host: string; user: string; port: number }) => Promise<void>;
  onCheckSsh: () => Promise<void>;
};
```

Required behavior:

- `open=false` renders nothing.
- Host/user/port fields initialize from `target`.
- Save validates host and user non-empty.
- Save validates integer port in `1..=65535`.
- Valid save calls `onSaveTarget`.
- Invalid save shows local validation text and does not call `onSaveTarget`.
- Check SSH button calls `onCheckSsh`.
- Close button calls `onOpenChange(false)`.
- Display server/SSE/service/model/current target status.
- Display `sshCheck.status`, `error_summary`, `exit_code`, and duration when provided.
- No browser-side SSH, shell, local file, API key, or direct model behavior.

Tests:

- Closed dialog renders nothing.
- Open dialog renders fields/status.
- Close calls `onOpenChange(false)`.
- Valid save parses port number.
- Empty host and invalid port block save and show validation.
- Check button calls `onCheckSsh`.
- `sshCheck` result is displayed.

Verification:

```bash
cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm test -- --run src/components/RemoteSettingsDialog.test.tsx
cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm run typecheck
```

Commit:

```bash
git add mobile-web/src/components/RemoteSettingsDialog.tsx mobile-web/src/components/RemoteSettingsDialog.test.tsx
git commit -m "feat: add remote settings dialog"
```

---

## Task A4: Toast And Product Status Utilities

**Owner:** UI utility worker.

**Files:**

- Create: `mobile-web/src/productStatus.ts`
- Create: `mobile-web/src/productStatus.test.ts`

**Do not edit:**

- `mobile-web/src/App.tsx`
- component files
- package files

**Goal:** Keep later `/web` integration small by extracting pure formatting/status helpers.

Required exports:

```ts
export type ProductConnectionInput = {
  server: string;
  sse: string;
  service?: string | null;
  model?: string | null;
  targetLabel?: string | null;
};

export function formatConnectionStatus(input: ProductConnectionInput): string;

export function isConnectionHealthy(input: ProductConnectionInput): boolean;

export function formatSessionTime(value: number): string;

export function buildTargetLabel(target?: { host: string; user: string; port: number } | null): string;
```

Required behavior:

- `formatConnectionStatus` includes server, SSE, service, and model.
- `isConnectionHealthy` returns false for server error, SSE error, or SSE disconnected.
- `formatSessionTime(0)` returns `No activity yet`.
- `buildTargetLabel(null)` returns `SSH target unknown`.
- `buildTargetLabel({ user, host, port })` returns `user@host:port`.

Tests:

- Cover every exported helper.

Verification:

```bash
cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm test -- --run src/productStatus.test.ts
cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm run typecheck
```

Commit:

```bash
git add mobile-web/src/productStatus.ts mobile-web/src/productStatus.test.ts
git commit -m "feat: add product status helpers"
```

---

## Task B1: Session Deleted Event And Reducer Support

**Owner:** State worker.

**Files:**

- Modify: `mobile-web/src/types.ts`
- Modify: `mobile-web/src/state.ts`
- Modify: `mobile-web/src/state.test.ts`

**Do not edit:**

- `mobile-web/src/App.tsx`
- components
- backend files

**Goal:** Teach the frontend state layer about the server's `session.deleted` lifecycle event.

Required behavior:

- Add `"session.deleted"` to `ServerEventType`.
- Add a typed payload helper if helpful:
  - `id?: string`
  - `session_id?: string`
  - `removed_pending_approvals?: number`
  - `deleted_at_ms?: number`
- `reduceEvent` on `session.deleted` removes:
  - matching session from `sessions`
  - active session id if it matches deleted id
  - pending approvals for that session
  - messages for that session if stored in state
  - active-session chat/tool entries if the current state can identify them
- Add timeline entry noting deletion.
- Do not break existing `/debug` behavior.

Tests:

- `session.deleted` removes session from `sessions`.
- `session.deleted` clears active session if deleted.
- `session.deleted` removes pending approvals for that session.
- Unknown/malformed payload does not crash.

Verification:

```bash
cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm test -- --run src/state.test.ts
cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm run typecheck
```

Commit:

```bash
git add mobile-web/src/types.ts mobile-web/src/state.ts mobile-web/src/state.test.ts
git commit -m "feat: handle mobile web session deletion events"
```

---

## Task B2: Product Session Controller Hook

**Owner:** Frontend state/hook worker.

**Files:**

- Create: `mobile-web/src/useProductSessions.ts`
- Create: `mobile-web/src/useProductSessions.test.tsx`

**Do not edit:**

- `mobile-web/src/App.tsx`
- `mobile-web/src/api.ts`
- components

**Goal:** Encapsulate reference-style session lifecycle behavior before integrating into `App.tsx`.

Suggested hook API:

```ts
export type UseProductSessionsOptions = {
  api?: ApiContext;
};

export function useProductSessions(options?: UseProductSessionsOptions): {
  sessions: SessionSummary[];
  activeSessionId: string | null;
  activeMessages: Message[];
  loading: boolean;
  error: string | null;
  selectSession: (id: string) => Promise<void>;
  createConversation: (title?: string) => Promise<SessionSummary>;
  deleteConversation: (id: string) => Promise<void>;
  updateConversationTitle: (id: string, title: string) => Promise<SessionSummary>;
  reloadSessions: () => Promise<void>;
  reloadMessages: (id: string) => Promise<void>;
};
```

Required behavior:

- On mount, call `listSessions`.
- If URL has `?session=<id>` and it exists, select it.
- Otherwise select most recent session if present.
- If no sessions exist, do not create one.
- Selecting a session loads `listMessages`.
- Active session changes update `?session=`.
- `createConversation` creates/selects session.
- `deleteConversation` deletes session, reloads sessions, and selects next available session or clears active session.
- `updateConversationTitle` calls API and updates local session list.

Tests:

- No sessions -> no create call.
- URL session wins when present.
- Selecting session loads messages and updates URL.
- Create conversation selects created session.
- Delete active session selects next session.
- Update title updates local list.

Verification:

```bash
cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm test -- --run src/useProductSessions.test.tsx
cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm run typecheck
```

Commit:

```bash
git add mobile-web/src/useProductSessions.ts mobile-web/src/useProductSessions.test.tsx
git commit -m "feat: add product session controller"
```

---

## Task B3: Product Timeline Selector Tests

**Owner:** Adapter/state worker.

**Files:**

- Modify: `mobile-web/src/opencodeAdapter.ts`
- Modify: `mobile-web/src/opencodeAdapter.test.ts`

**Do not edit:**

- `mobile-web/src/App.tsx`
- components

**Goal:** Add the final timeline helper needed by `/web` integration, while preserving the active-session-only contract established in Wave 1.

Required behavior:

- Export `buildProductTimeline` or extend `buildTimelineItems` so integration can combine:
  - active chat messages
  - active tools
  - globally stored pending approvals filtered by `sessionId`
  - optional loading marker
- Timeline item kinds should be explicit:
  - `message`
  - `permission`
  - `tool`
  - `loading`
- Stable ids:
  - `message:<id>`
  - `permission:<id>`
  - `tool:<id>`
  - `loading:<sessionId>`
- Sorting is chronological; loading marker appears last.

Tests:

- Mixed-session pending approvals are filtered.
- Active messages/tools are included.
- Loading marker appears last.
- Equal timestamps sort stably by id.

Verification:

```bash
cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm test -- --run src/opencodeAdapter.test.ts
cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm run typecheck
```

Commit:

```bash
git add mobile-web/src/opencodeAdapter.ts mobile-web/src/opencodeAdapter.test.ts
git commit -m "feat: add product chat timeline helper"
```

---

## Task C1: Replace `/web` Shell And Preserve `/debug`

**Owner:** Integration worker.

**Run only after:** all Wave A and Wave B tasks pass review.

**Files:**

- Modify: `mobile-web/src/App.tsx`
- Modify: `mobile-web/src/App.test.tsx`
- Modify: `mobile-web/src/styles.css`

**Goal:** Make `/web` the opencode-style formal chat UI and keep `/debug` as the old raw/test UI.

Required `/web` behavior:

- Load sessions via `useProductSessions`.
- Do not create a session on initial empty state.
- Show welcome page when active session has no messages/timeline.
- First send without active session creates a session using `buildConversationTitleHint`.
- Sending message appends user message optimistically and calls `sendAgentTurn`.
- Active conversation loads messages.
- `?session=` sync works through the session controller.
- Sidebar new/select/delete work.
- Header settings opens `RemoteSettingsDialog`.
- Composer sends and stops.
- Pending approvals render with `InlinePermissionMessage`.
- Approval buttons map:
  - `"once"` -> `approve_once`
  - `"always"` -> `approve_session`
  - `"reject"` -> `reject_stop`
- Tool activity renders inline through `OpencodeChatMessage`/tool parts.
- No `/web` `ProductShell` or separate `工具活动` panel.
- Toasts show transient errors.
- `/debug` still exposes old diagnostics/raw timeline/manual command affordances.

Tests:

- `/web` calls `GET /api/sessions` on load.
- `/web` does not eagerly create session when no sessions exist.
- First send creates session and sends agent turn.
- Selecting session loads messages.
- `?session=session-1` selects existing session.
- Pending approval renders inline and buttons call approval API.
- Tool activity renders inline and `工具活动` panel is absent.
- Remote settings dialog opens and saves/checks SSH through callbacks.
- `/debug` still renders raw/testing interface.

Verification:

```bash
cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm test -- --run src/App.test.tsx
cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm run typecheck
cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm run build
```

Commit:

```bash
git add mobile-web/src/App.tsx mobile-web/src/App.test.tsx mobile-web/src/styles.css
git commit -m "feat: replace mobile web with opencode chat shell"
```

---

## Task D1: Mobile Layout And Overflow Polish

**Owner:** CSS/layout worker.

**Run after:** Task C1.

**Files:**

- Modify: `mobile-web/src/styles.css`
- Modify component CSS/classes only if required for overflow fixes.
- Modify: `mobile-web/src/App.test.tsx` only for layout behavior assertions.

**Goal:** Ensure the integrated `/web` layout works on phone-width screens and long command/output content stays inside cards.

Required checks:

- No horizontal page overflow at 375px.
- Sidebar overlay has scrim and does not leave a blank gutter when closed.
- Composer stays visible at bottom.
- Long command text scrolls inside code blocks.
- Long stdout/stderr is contained in tool cards.
- Buttons do not overlap on 375px width.
- Cards are not nested inside cards.
- `/debug` raw panels remain usable.

Verification:

```bash
cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm test
cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm run typecheck
cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm run build
```

Commit:

```bash
git add mobile-web/src/styles.css mobile-web/src/components mobile-web/src/App.test.tsx
git commit -m "fix: polish mobile opencode chat layout"
```

---

## Task D2: Smoke And Regression Updates

**Owner:** Verification worker.

**Run after:** Task C1.

**Files:**

- Modify: `scripts/mobile_web_ai_chat_smoke_test.py` if it asserts old text.
- Modify: `mobile-web/src/App.test.tsx` only if smoke-relevant regression coverage belongs there.

**Goal:** Update automated checks to reflect `/web` as product chat and `/debug` as raw test surface.

Required smoke expectations:

- `/web` contains `DeepSeek 远程 Linux`.
- `/web` contains the chat composer.
- `/web` does not contain the old separate `工具活动` panel.
- `/debug` contains diagnostics/raw testing affordances.

Verification:

```bash
python3 scripts/mobile_web_ai_chat_smoke_test.py
cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm test -- --run src/App.test.tsx
```

Commit:

```bash
git add scripts/mobile_web_ai_chat_smoke_test.py mobile-web/src/App.test.tsx
git commit -m "test: update mobile web opencode smoke coverage"
```

---

## Task D3: Documentation Updates

**Owner:** Docs worker.

**Run after:** Task C1.

**Files:**

- Modify: `docs/linux-first-mobile-task-tree.md`
- Modify: `docs/mobile-porting-plan.md`
- Modify: `docs/mobile-validation-checklists.md`
- Modify: `mobile-web/README.md` if present and relevant.

**Goal:** Document the final `/web` and `/debug` split without overclaiming platform support.

Required wording:

- `/web` is the formal chat UI.
- `/debug` is the raw test/diagnostics UI.
- Tools and approvals render inline in the chat timeline.
- Browser is a thin client.
- Rust server controls remote execution and approval policy.
- Evidence remains Linux/LAN web unless manual user-run testing proves more.

Verification:

```bash
git diff --check -- docs mobile-web/README.md
```

Commit:

```bash
git add docs mobile-web/README.md
git commit -m "docs: document opencode-style mobile web chat"
```

---

## Task E1: Parent Final Review

**Owner:** Parent only.

**Goal:** Verify the integrated product before reporting completion.

Run:

```bash
git status --short
cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm test
cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm run typecheck
cd mobile-web && PATH=/var/tmp/deepseek-mobile-web-node/node-v22.22.3-linux-x64/bin:$PATH npm run build
cargo test -p deepseek-mobile-web-server --all-features
cargo fmt --all --check
python3 scripts/mobile_web_ai_chat_smoke_test.py
```

Optional full checks:

```bash
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features
```

Manual UI review:

- `/web` desktop width.
- `/web` 375px/390px/430px widths.
- Long approval command.
- Long stdout/stderr tool output.
- Remote settings save/check.
- New/select/delete sessions.
- Stop/send composer.
- `/debug` raw diagnostics.

Final report must include:

- commits made
- tests run
- skipped checks
- known gaps
- whether `/web` now matches the reference interaction target

