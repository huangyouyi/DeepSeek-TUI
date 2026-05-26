# Mobile Web Opencode UI Parity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace `/web` with an opencode-style chat UI that closely matches `/home/cu/projects/istore-ai-helper/web`, while keeping Rust server ownership of SSH target configuration, approvals, remote command execution, and model/tool policy.

**Architecture:** Keep the existing backend API as the safety boundary and add frontend adapters that map current mobile-web sessions, messages, approvals, and tools into opencode-like view models. Build `/web` as one primary chat timeline with inline permission and tool cards; keep `/debug` as the raw testing surface.

**Tech Stack:** Rust 1.88+, Axum, React 19, Vite, TypeScript, Vitest, Testing Library, CSS, lucide-react, sonner, marked, DOMPurify.

---

## Source Spec

Design spec:

- `docs/superpowers/specs/2026-05-26-mobile-web-opencode-ui-parity-design.md`

Reference UI:

- `/home/cu/projects/istore-ai-helper/web/src/App.tsx`
- `/home/cu/projects/istore-ai-helper/web/src/components/Sidebar.tsx`
- `/home/cu/projects/istore-ai-helper/web/src/components/ChatHeader.tsx`
- `/home/cu/projects/istore-ai-helper/web/src/components/ChatInput.tsx`
- `/home/cu/projects/istore-ai-helper/web/src/components/ChatMessage.tsx`
- `/home/cu/projects/istore-ai-helper/web/src/components/PermissionInlineMessage.tsx`
- `/home/cu/projects/istore-ai-helper/web/src/components/WelcomePage.tsx`
- `/home/cu/projects/istore-ai-helper/web/src/markdown.ts`
- `/home/cu/projects/istore-ai-helper/web/src/styles/markdown.css`
- `/home/cu/projects/istore-ai-helper/web/src/styles/message.css`
- `/home/cu/projects/istore-ai-helper/web/src/styles/permission-alert.css`

Current implementation:

- `mobile-web/src/App.tsx`
- `mobile-web/src/api.ts`
- `mobile-web/src/state.ts`
- `mobile-web/src/types.ts`
- `mobile-web/src/styles.css`
- `mobile-web/src/components/ProductShell.tsx`
- `mobile-web/src/components/ToolActivity.tsx`
- `crates/mobile-web-server/src/routes.rs`
- `crates/mobile-web-server/src/state.rs`
- `crates/mobile-web-server/src/types.rs`

## Parallelization Rules

Use one subagent per task unless a task explicitly says parent-only.

No two subagents may edit the same file at the same time. These are hotspot files and must be owned by one worker at a time:

- `mobile-web/src/App.tsx`
- `mobile-web/src/api.ts`
- `mobile-web/src/styles.css`
- `mobile-web/package.json`
- `mobile-web/package-lock.json`
- `crates/mobile-web-server/src/routes.rs`
- `crates/mobile-web-server/src/state.rs`
- `crates/mobile-web-server/src/types.rs`

Independent subagents may create new component/test files in parallel, but integration into `App.tsx` is a single sequential task.

## Parallelization Tree

```text
MW-O. Opencode UI Parity
├─ Wave 0: Planning gate (parent only)
│  └─ Task 0. Commit this task tree and confirm ownership rules
├─ Wave 1: Parallel foundations
│  ├─ Task 1A. Backend session lifecycle endpoints
│  ├─ Task 1B. Frontend API/session helpers
│  ├─ Task 1C. Opencode adapter and view-model tests
│  ├─ Task 1D. UI dependency and markdown utilities
│  └─ Task 1E. Port standalone chat components
├─ Wave 2: Parallel feature components
│  ├─ Task 2A. Inline permission card
│  ├─ Task 2B. Inline tool/message renderer
│  ├─ Task 2C. Remote settings dialog
│  └─ Task 2D. Sidebar/header/composer/welcome parity
├─ Wave 3: Sequential integration hotspot
│  └─ Task 3. Replace /web shell and preserve /debug
├─ Wave 4: Product polish and compatibility
│  ├─ Task 4A. Mobile layout and overflow polish
│  ├─ Task 4B. End-to-end tests and smoke updates
│  └─ Task 4C. Docs and final validation
└─ Wave 5: Parent review
   └─ Task 5. Cross-task integration review and final verification
```

Wave dependencies:

- Wave 1 tasks can run in parallel.
- Wave 2 tasks can run after Task 1C defines the adapter contracts; they should not edit `App.tsx`.
- Task 3 starts only after Wave 1 and Wave 2 are merged.
- Wave 4 starts after Task 3 compiles.
- Task 5 is parent-only review.

## Final Objective

This work is complete when:

- `/web` uses the reference-style chat shell.
- `/web` has one primary content area.
- The old `/web` tool activity side panel is gone.
- Sessions load from the server on startup.
- URL `?session=<id>` sync works.
- New conversation behavior matches the reference app.
- Session delete and title update work, or server-backed gaps are explicitly closed.
- Assistant text renders as sanitized markdown.
- Pending approvals render inline with `仅这次执行`, `本会话都允许`, and `拒绝并停止`.
- Tool execution renders inline as collapsible cards.
- Remote SSH target settings remain available from `/web`.
- `/debug` still exposes the current raw testing surface.
- Browser code never runs SSH, shell, local file access, or model secrets directly.

---

## Task 0: Commit Task Tree

**Owner:** Parent only.

**Files:**

- Create: `docs/superpowers/plans/2026-05-26-mobile-web-opencode-ui-parity-task-tree.md`

- [ ] **Step 1: Add this task tree**

Add this file and keep it focused on parallel execution structure.

- [ ] **Step 2: Verify no unrelated files changed**

Run:

```bash
git status --short
```

Expected: only this plan file is uncommitted.

- [ ] **Step 3: Commit the task tree**

Run:

```bash
git add docs/superpowers/plans/2026-05-26-mobile-web-opencode-ui-parity-task-tree.md
git commit -m "docs: plan mobile web opencode ui parity tasks"
```

Expected: commit succeeds.

---

## Task 1A: Backend Session Lifecycle Endpoints

**Owner:** Backend subagent.

**Can run in parallel with:** 1C, 1D, 1E.

**Do not run in parallel with:** another backend worker editing `routes.rs`, `state.rs`, or `types.rs`.

**Files:**

- Modify: `crates/mobile-web-server/src/routes.rs`
- Modify: `crates/mobile-web-server/src/state.rs`
- Modify: `crates/mobile-web-server/src/types.rs` if request/response structs are needed
- Test: `crates/mobile-web-server/tests/http_api.rs`

**Goal:** Add server-backed session delete and title update if missing, so `/web` can match the reference session behavior.

- [ ] **Step 1: Confirm missing routes**

Run:

```bash
rg -n "api/sessions|delete_session|update.*session|PATCH|DELETE" crates/mobile-web-server/src crates/mobile-web-server/tests/http_api.rs
```

Expected: `GET /api/sessions`, `POST /api/sessions`, and `GET /api/sessions/{id}/messages` already exist. `DELETE /api/sessions/{id}` and title update are missing unless another branch has added them.

- [ ] **Step 2: Add failing HTTP tests**

Add tests to `crates/mobile-web-server/tests/http_api.rs`:

```rust
#[tokio::test]
async fn delete_session_removes_session_and_messages() {
    let app = test_app().await;
    let created = create_test_session(&app).await;
    let session_id = created["id"].as_str().expect("session id");

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::DELETE)
                .uri(format!("/api/sessions/{session_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let sessions = get_json(&app, "/api/sessions").await;
    assert!(
        !sessions.as_array().unwrap().iter().any(|item| item["id"] == session_id),
        "deleted session should not appear in session list"
    );
}

#[tokio::test]
async fn patch_session_title_updates_list_sessions() {
    let app = test_app().await;
    let created = create_test_session(&app).await;
    let session_id = created["id"].as_str().expect("session id");

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::PATCH)
                .uri(format!("/api/sessions/{session_id}"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"title":"检查网络"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let updated = response_json(response).await;
    assert_eq!(updated["title"], "检查网络");
}
```

If helper names differ in `http_api.rs`, adapt the test to existing local helpers while preserving the assertions.

- [ ] **Step 3: Run tests and confirm failure**

Run:

```bash
cargo test -p deepseek-mobile-web-server --test http_api --all-features delete_session_removes_session_and_messages
cargo test -p deepseek-mobile-web-server --test http_api --all-features patch_session_title_updates_list_sessions
```

Expected before implementation: route not found or method not allowed.

- [ ] **Step 4: Implement state operations**

Add state methods in `crates/mobile-web-server/src/state.rs`:

```rust
pub fn delete_session(&self, session_id: &str) -> bool {
    let mut state = self.inner.write().expect("state lock poisoned");
    let existed = state.sessions.remove(session_id).is_some();
    state.messages.remove(session_id);
    state
        .pending_approvals
        .retain(|_, approval| approval.session_id != session_id);
    existed
}

pub fn update_session_title(&self, session_id: &str, title: String) -> Option<SessionSummary> {
    let mut state = self.inner.write().expect("state lock poisoned");
    let session = state.sessions.get_mut(session_id)?;
    session.title = title;
    session.updated_at_ms = now_ms();
    Some(session.clone())
}
```

Use existing field names, lock types, and timestamp helpers from `state.rs`. If `sessions` is not keyed by id, implement the equivalent mutation using the existing storage shape.

- [ ] **Step 5: Add route handlers**

Add routes in `crates/mobile-web-server/src/routes.rs`:

```rust
.route("/api/sessions/{id}", patch(update_session).delete(delete_session))
```

Add request type if none exists:

```rust
#[derive(Debug, Deserialize)]
struct UpdateSessionRequest {
    title: String,
}
```

Handler behavior:

- `DELETE`: return `204 No Content` when deleted, `404` when missing.
- `PATCH`: trim title, reject empty title with `400`, return updated `SessionSummary`, return `404` when missing.

- [ ] **Step 6: Run backend tests**

Run:

```bash
cargo test -p deepseek-mobile-web-server --test http_api --all-features
cargo fmt --all --check
```

Expected: pass.

- [ ] **Step 7: Commit**

Run:

```bash
git add crates/mobile-web-server/src/routes.rs crates/mobile-web-server/src/state.rs crates/mobile-web-server/src/types.rs crates/mobile-web-server/tests/http_api.rs
git commit -m "feat: add mobile web session lifecycle endpoints"
```

---

## Task 1B: Frontend API Session Helpers

**Owner:** Frontend API subagent.

**Can run in parallel with:** 1A if it only edits frontend files; integration with backend waits for 1A.

**Files:**

- Modify: `mobile-web/src/api.ts`
- Modify: `mobile-web/src/api.test.ts`
- Modify: `mobile-web/src/types.ts` if request types are needed

**Goal:** Add frontend helpers for list/load/delete/update session behavior used by the reference-style `/web` app.

- [ ] **Step 1: Add failing API tests**

In `mobile-web/src/api.test.ts`, add tests for:

```ts
import { deleteSession, listMessages, listSessions, updateSessionTitle } from "./api";

it("lists and loads sessions", async () => {
  const fetchMock = vi.fn()
    .mockResolvedValueOnce(jsonResponse([{ id: "session-1", title: "网络", created_at_ms: 1, updated_at_ms: 2 }]))
    .mockResolvedValueOnce(jsonResponse([{ id: "message-1", session_id: "session-1", role: "assistant", created_at_ms: 3, parts: [] }]));

  await expect(listSessions({ fetch: fetchMock as typeof fetch })).resolves.toHaveLength(1);
  await expect(listMessages("session-1", { fetch: fetchMock as typeof fetch })).resolves.toHaveLength(1);

  expect(fetchMock.mock.calls[0][0]).toBe("/api/sessions");
  expect(fetchMock.mock.calls[1][0]).toBe("/api/sessions/session-1/messages");
});

it("updates and deletes sessions", async () => {
  const fetchMock = vi.fn()
    .mockResolvedValueOnce(jsonResponse({ id: "session-1", title: "新标题", created_at_ms: 1, updated_at_ms: 4 }))
    .mockResolvedValueOnce(new Response(null, { status: 204 }));

  await updateSessionTitle("session-1", "新标题", { fetch: fetchMock as typeof fetch });
  await deleteSession("session-1", { fetch: fetchMock as typeof fetch });

  expect(fetchMock.mock.calls[0][0]).toBe("/api/sessions/session-1");
  expect(fetchMock.mock.calls[0][1]).toMatchObject({ method: "PATCH" });
  expect(fetchMock.mock.calls[1][0]).toBe("/api/sessions/session-1");
  expect(fetchMock.mock.calls[1][1]).toMatchObject({ method: "DELETE" });
});
```

Use the existing `jsonResponse` helper in the file.

- [ ] **Step 2: Run tests and confirm failure**

Run:

```bash
cd mobile-web && npm test -- --run src/api.test.ts
```

Expected before implementation: missing exported helpers.

- [ ] **Step 3: Implement helpers**

In `mobile-web/src/api.ts`, add:

```ts
export function updateSessionTitle(sessionId: string, title: string, api?: ApiContext): Promise<SessionSummary> {
  return requestJson<SessionSummary>(
    `/api/sessions/${encodeURIComponent(sessionId)}`,
    { method: "PATCH", body: JSON.stringify({ title }) },
    api
  );
}

export async function deleteSession(sessionId: string, api?: ApiContext): Promise<void> {
  await requestEmpty(
    `/api/sessions/${encodeURIComponent(sessionId)}`,
    { method: "DELETE" },
    api
  );
}
```

Add `requestEmpty` beside `requestJson`:

```ts
async function requestEmpty(path: string, init: RequestInit = {}, api: ApiContext = defaultApi): Promise<void> {
  const accessToken = resolveAccessToken(api);
  const response = await api.fetch(path, {
    ...init,
    headers: {
      ...(init.body ? { "content-type": "application/json" } : {}),
      ...(accessToken ? { "X-Mobile-Web-Token": accessToken } : {}),
      ...init.headers
    }
  });

  if (!response.ok) {
    let message = `${response.status} ${response.statusText}`;
    try {
      const body = (await response.json()) as { message?: string };
      message = body.message ?? message;
    } catch {
      // Keep the HTTP status fallback.
    }
    throw new Error(redactAccessToken(message, accessToken));
  }
}
```

- [ ] **Step 4: Run tests**

Run:

```bash
cd mobile-web && npm test -- --run src/api.test.ts
cd mobile-web && npm run typecheck
```

Expected: pass.

- [ ] **Step 5: Commit**

Run:

```bash
git add mobile-web/src/api.ts mobile-web/src/api.test.ts mobile-web/src/types.ts
git commit -m "feat: add mobile web session API helpers"
```

---

## Task 1C: Opencode Adapter And View Models

**Owner:** Adapter subagent.

**Can run in parallel with:** 1A, 1B, 1D, 1E.

**Files:**

- Create: `mobile-web/src/opencodeAdapter.ts`
- Create: `mobile-web/src/opencodeAdapter.test.ts`
- Modify: `mobile-web/src/types.ts` only if missing fields block the adapter

**Goal:** Convert current mobile-web state into opencode-like view models for components, without changing backend API shapes.

- [ ] **Step 1: Create failing adapter tests**

Create `mobile-web/src/opencodeAdapter.test.ts` with tests for:

```ts
import { describe, expect, it } from "vitest";
import { buildConversationTitleHint, mapApprovalToPermission, mapSessionToConversation, mapToolActivityToToolPart } from "./opencodeAdapter";

describe("opencodeAdapter", () => {
  it("maps sessions to conversations", () => {
    expect(mapSessionToConversation({ id: "s1", title: "网络", created_at_ms: 1, updated_at_ms: 2 })).toEqual({
      id: "s1",
      title: "网络",
      createdAt: 1,
      updatedAt: 2
    });
  });

  it("builds first-message title hints", () => {
    expect(buildConversationTitleHint("### 检查网络\n第二行")).toBe("检查网络");
    expect(buildConversationTitleHint("a".repeat(40))).toBe(`${"a".repeat(32)}...`);
  });

  it("maps pending approvals to permission cards", () => {
    const permission = mapApprovalToPermission({
      id: "approval-1",
      session_id: "session-1",
      command: "ping -c 3 223.5.5.5",
      created_at_ms: 10,
      status: "pending",
      cwd: "/root",
      risk_reason: "Network diagnostic",
      target_label: "router",
      target: "root@192.168.1.1:22"
    });

    expect(permission).toMatchObject({
      id: "approval-1",
      type: "bash",
      sessionID: "session-1",
      pattern: "ping -c 3 223.5.5.5",
      title: "Bash 命令执行请求"
    });
    expect(permission.metadata.command).toBe("ping -c 3 223.5.5.5");
  });

  it("maps tool activity to collapsed tool parts", () => {
    const part = mapToolActivityToToolPart({
      id: "tool-1",
      status: "completed",
      command: "df -h",
      stdout: "ok",
      stderr: "",
      exitCode: 0,
      durationMs: 74,
      createdAtMs: 20
    });

    expect(part.type).toBe("tool");
    expect(part.tool).toBe("bash");
    expect(part.state?.input).toMatchObject({ command: "df -h" });
    expect(part.state?.metadata).toMatchObject({ exit: 0, durationMs: 74 });
  });
});
```

- [ ] **Step 2: Run tests and confirm failure**

Run:

```bash
cd mobile-web && npm test -- --run src/opencodeAdapter.test.ts
```

Expected before implementation: module missing.

- [ ] **Step 3: Implement adapter**

Create `mobile-web/src/opencodeAdapter.ts` exporting:

- `Conversation`
- `PermissionLike`
- `SessionMessageLike`
- `MessagePartLike`
- `ToolPartLike`
- `mapSessionToConversation`
- `buildConversationTitleHint`
- `mapApprovalToPermission`
- `mapToolActivityToToolPart`
- `mapChatItemToSessionMessage`
- `buildTimelineItems`

Required behavior:

- Use `...` for truncated title hints to keep files ASCII.
- Keep `PermissionLike.response` out of the adapter; response mapping belongs in UI handlers.
- Preserve raw command in metadata.
- Convert `pending_approval` to visual status `pending`.
- Convert failed tool states to `error` for UI icon compatibility.

- [ ] **Step 4: Run adapter tests and typecheck**

Run:

```bash
cd mobile-web && npm test -- --run src/opencodeAdapter.test.ts
cd mobile-web && npm run typecheck
```

Expected: pass.

- [ ] **Step 5: Commit**

Run:

```bash
git add mobile-web/src/opencodeAdapter.ts mobile-web/src/opencodeAdapter.test.ts mobile-web/src/types.ts
git commit -m "feat: add opencode view adapters"
```

---

## Task 1D: UI Dependencies And Markdown Utilities

**Owner:** UI foundation subagent.

**Can run in parallel with:** 1A, 1B, 1C, 1E.

**Files:**

- Modify: `mobile-web/package.json`
- Modify: `mobile-web/package-lock.json`
- Create: `mobile-web/src/markdown.ts`
- Create: `mobile-web/src/styles/markdown.css`
- Create: `mobile-web/src/styles/message.css`
- Create: `mobile-web/src/styles/permission-alert.css`
- Modify: `mobile-web/src/styles.css` only for global imports if needed

**Goal:** Add the minimal local UI/runtime dependencies and markdown pipeline needed to match the reference rendering.

- [ ] **Step 1: Install dependencies**

Run:

```bash
cd mobile-web && npm install lucide-react sonner marked dompurify
```

Expected: `package.json` and `package-lock.json` update.

- [ ] **Step 2: Add markdown utility**

Create `mobile-web/src/markdown.ts`:

```ts
import DOMPurify from "dompurify";
import { marked } from "marked";

marked.setOptions({
  breaks: true
});

export function renderMarkdown(text?: string | null): string {
  if (!text) {
    return "";
  }
  const raw = marked.parse(text);
  return DOMPurify.sanitize(raw);
}
```

- [ ] **Step 3: Port CSS assets**

Copy the reference CSS content into local files:

- `/home/cu/projects/istore-ai-helper/web/src/styles/markdown.css` -> `mobile-web/src/styles/markdown.css`
- `/home/cu/projects/istore-ai-helper/web/src/styles/message.css` -> `mobile-web/src/styles/message.css`
- `/home/cu/projects/istore-ai-helper/web/src/styles/permission-alert.css` -> `mobile-web/src/styles/permission-alert.css`

Keep selectors scoped to component classes where possible. Do not add external fonts, analytics, or hosted assets.

- [ ] **Step 4: Add markdown tests**

Create or extend a small test file, `mobile-web/src/markdown.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { renderMarkdown } from "./markdown";

describe("renderMarkdown", () => {
  it("renders markdown and sanitizes scripts", () => {
    const html = renderMarkdown("**ok**<script>alert(1)</script>");
    expect(html).toContain("<strong>ok</strong>");
    expect(html).not.toContain("script");
  });
});
```

- [ ] **Step 5: Run tests and build**

Run:

```bash
cd mobile-web && npm test -- --run src/markdown.test.ts
cd mobile-web && npm run typecheck
cd mobile-web && npm run build
```

Expected: pass.

- [ ] **Step 6: Commit**

Run:

```bash
git add mobile-web/package.json mobile-web/package-lock.json mobile-web/src/markdown.ts mobile-web/src/markdown.test.ts mobile-web/src/styles
git commit -m "feat: add mobile web markdown rendering foundation"
```

---

## Task 1E: Port Standalone Chat Components

**Owner:** Component subagent.

**Can run in parallel with:** 1A, 1B, 1C, 1D.

**Files:**

- Create: `mobile-web/src/components/OpencodeChatInput.tsx`
- Create: `mobile-web/src/components/OpencodeSidebar.tsx`
- Create: `mobile-web/src/components/OpencodeWelcomePage.tsx`
- Create: `mobile-web/src/components/OpencodeChatHeader.tsx`
- Create: `mobile-web/src/components/OpencodeShell.test.tsx`

**Goal:** Port standalone layout components from the reference app without wiring them into `App.tsx`.

- [ ] **Step 1: Add failing component tests**

Create tests in `mobile-web/src/components/OpencodeShell.test.tsx`:

```tsx
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { OpencodeChatInput } from "./OpencodeChatInput";
import { OpencodeSidebar } from "./OpencodeSidebar";
import { OpencodeWelcomePage } from "./OpencodeWelcomePage";

it("sends chat input on Enter and preserves Shift Enter", () => {
  const onSend = vi.fn();
  render(<OpencodeChatInput onSendMessage={onSend} />);
  const input = screen.getByPlaceholderText(/输入消息/);
  fireEvent.change(input, { target: { value: "检查网络" } });
  fireEvent.keyDown(input, { key: "Enter", shiftKey: false });
  expect(onSend).toHaveBeenCalledWith("检查网络");
});

it("renders sidebar conversations and delete control", () => {
  render(
    <OpencodeSidebar
      conversations={[{ id: "s1", title: "网络", createdAt: 1, updatedAt: 2 }]}
      currentConversationId="s1"
      onSelectConversation={vi.fn()}
      onNewConversation={vi.fn()}
      onDeleteConversation={vi.fn()}
      isOpen
    />
  );
  expect(screen.getByText("网络")).toBeTruthy();
  expect(screen.getByLabelText("删除对话")).toBeTruthy();
});

it("renders welcome suggestions", () => {
  const onSuggestedQuestion = vi.fn();
  render(<OpencodeWelcomePage onSuggestedQuestion={onSuggestedQuestion} />);
  fireEvent.click(screen.getByText("检查远程 Linux 设备状态"));
  expect(onSuggestedQuestion).toHaveBeenCalledWith("检查远程 Linux 设备状态");
});
```

- [ ] **Step 2: Run tests and confirm failure**

Run:

```bash
cd mobile-web && npm test -- --run src/components/OpencodeShell.test.tsx
```

Expected before implementation: missing components.

- [ ] **Step 3: Implement components**

Port structure from the reference app and adapt labels to this product:

- App title: `DeepSeek 远程 Linux`
- Sidebar subtitle: `服务器侧安全执行`
- Welcome suggestions:
  - `检查远程 Linux 设备状态`
  - `排查网络连通性`
  - `检查磁盘和内存压力`
  - `总结刚才的工具输出`
- Header menu items:
  - `远程设备设置`
  - `语言设置` only if the implementation includes a local dialog; otherwise omit it for this phase.

Do not import any generated opencode API types. Use local props only.

- [ ] **Step 4: Run tests**

Run:

```bash
cd mobile-web && npm test -- --run src/components/OpencodeShell.test.tsx
cd mobile-web && npm run typecheck
```

Expected: pass.

- [ ] **Step 5: Commit**

Run:

```bash
git add mobile-web/src/components/OpencodeChatInput.tsx mobile-web/src/components/OpencodeSidebar.tsx mobile-web/src/components/OpencodeWelcomePage.tsx mobile-web/src/components/OpencodeChatHeader.tsx mobile-web/src/components/OpencodeShell.test.tsx
git commit -m "feat: port opencode chat shell components"
```

---

## Task 2A: Inline Permission Card

**Owner:** Permission UI subagent.

**Depends on:** Task 1C adapter contract.

**Files:**

- Create: `mobile-web/src/components/InlinePermissionMessage.tsx`
- Create: `mobile-web/src/components/InlinePermissionMessage.test.tsx`

**Goal:** Render `PendingApproval` as an opencode-style inline authorization card in the message flow.

- [ ] **Step 1: Add failing tests**

Create `mobile-web/src/components/InlinePermissionMessage.test.tsx`:

```tsx
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { mapApprovalToPermission } from "../opencodeAdapter";
import { InlinePermissionMessage } from "./InlinePermissionMessage";

it("renders bash approval with three actions", () => {
  const permission = mapApprovalToPermission({
    id: "approval-1",
    session_id: "session-1",
    command: "ping -c 3 223.5.5.5",
    created_at_ms: 1716684180000,
    status: "pending"
  });

  render(<InlinePermissionMessage permission={permission} onRespond={vi.fn()} />);

  expect(screen.getByText("需要授权")).toBeTruthy();
  expect(screen.getByText("bash")).toBeTruthy();
  expect(screen.getByText("Bash 命令执行请求")).toBeTruthy();
  expect(screen.getByText("$ ping -c 3 223.5.5.5")).toBeTruthy();
  expect(screen.getByText("仅这次执行")).toBeTruthy();
  expect(screen.getByText("本会话都允许")).toBeTruthy();
  expect(screen.getByText("拒绝并停止")).toBeTruthy();
});

it("maps buttons to once always reject", async () => {
  const permission = mapApprovalToPermission({
    id: "approval-1",
    session_id: "session-1",
    command: "df -h",
    created_at_ms: 1,
    status: "pending"
  });
  const onRespond = vi.fn().mockResolvedValue(undefined);

  render(<InlinePermissionMessage permission={permission} onRespond={onRespond} />);
  fireEvent.click(screen.getByText("本会话都允许"));

  await waitFor(() => expect(onRespond).toHaveBeenCalledWith("always"));
});
```

- [ ] **Step 2: Implement component**

Port the reference `PermissionInlineMessage` structure. Use local `PermissionLike` type from `opencodeAdapter.ts`.

Behavior:

- Prefix bash command lines with `$ `.
- Render session id and formatted time.
- Render `risk_reason` if available; otherwise use the standard shell warning text.
- Disable buttons while a response is in progress.
- Show an inline error block if `onRespond` rejects.

- [ ] **Step 3: Run tests**

Run:

```bash
cd mobile-web && npm test -- --run src/components/InlinePermissionMessage.test.tsx
cd mobile-web && npm run typecheck
```

Expected: pass.

- [ ] **Step 4: Commit**

Run:

```bash
git add mobile-web/src/components/InlinePermissionMessage.tsx mobile-web/src/components/InlinePermissionMessage.test.tsx
git commit -m "feat: render approvals inline in chat"
```

---

## Task 2B: Inline Tool And Message Renderer

**Owner:** Message renderer subagent.

**Depends on:** Task 1C and Task 1D.

**Files:**

- Create: `mobile-web/src/components/OpencodeChatMessage.tsx`
- Create: `mobile-web/src/components/OpencodeChatMessage.test.tsx`

**Goal:** Render markdown text and inline collapsible tool cards with opencode-style behavior.

- [ ] **Step 1: Add failing tests**

Create `mobile-web/src/components/OpencodeChatMessage.test.tsx`:

```tsx
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { OpencodeChatMessage } from "./OpencodeChatMessage";

it("renders assistant markdown", () => {
  render(
    <OpencodeChatMessage
      message={{
        info: { id: "m1", sessionID: "s1", role: "assistant", time: { created: 1 } },
        parts: [{ id: "p1", type: "text", text: "**完成**" }]
      }}
    />
  );
  expect(screen.getByText("完成").tagName.toLowerCase()).toBe("strong");
});

it("collapses completed tool output by default and expands on click", () => {
  render(
    <OpencodeChatMessage
      message={{
        info: { id: "m1", sessionID: "s1", role: "assistant", time: { created: 1 } },
        parts: [{
          id: "tool-1",
          type: "tool",
          tool: "bash",
          state: {
            status: "completed",
            input: { command: "df -h" },
            output: "Filesystem",
            metadata: { exit: 0, durationMs: 74 }
          }
        }]
      }}
    />
  );

  expect(screen.queryByText("Filesystem")).toBeNull();
  fireEvent.click(screen.getByText("df -h"));
  expect(screen.getByText("Filesystem")).toBeTruthy();
});
```

- [ ] **Step 2: Implement renderer**

Port the reference `ChatMessage` behavior and adapt local types:

- User messages align right.
- Assistant messages align left.
- Text parts use `renderMarkdown`.
- Reasoning parts render dimmed.
- Tool parts render as cards.
- Completed tools are collapsed by default.
- Pending/running tools are expanded by default.
- Failed/error tools show warning styling.
- Long code blocks scroll horizontally inside the card.

- [ ] **Step 3: Run tests**

Run:

```bash
cd mobile-web && npm test -- --run src/components/OpencodeChatMessage.test.tsx
cd mobile-web && npm run typecheck
```

Expected: pass.

- [ ] **Step 4: Commit**

Run:

```bash
git add mobile-web/src/components/OpencodeChatMessage.tsx mobile-web/src/components/OpencodeChatMessage.test.tsx
git commit -m "feat: add opencode-style chat message renderer"
```

---

## Task 2C: Remote Settings Dialog

**Owner:** Settings UI subagent.

**Files:**

- Create: `mobile-web/src/components/RemoteSettingsDialog.tsx`
- Create: `mobile-web/src/components/RemoteSettingsDialog.test.tsx`

**Goal:** Replace the reference model settings dialog with this product's SSH target settings while preserving the same header-menu interaction pattern.

- [ ] **Step 1: Add failing tests**

Create tests proving the dialog:

- shows host/user/port fields
- calls `onSaveTarget` with parsed port
- calls `onCheckSsh`
- shows server/SSE/service/model/target status
- can be closed

Use this shape:

```tsx
render(
  <RemoteSettingsDialog
    open
    target={{ host: "192.168.1.1", user: "root", port: 22, key_present: true }}
    status={{ server: "ok", sse: "connected", service: "deepseek", model: "deepseek-v4-flash" }}
    onOpenChange={vi.fn()}
    onSaveTarget={vi.fn().mockResolvedValue(undefined)}
    onCheckSsh={vi.fn().mockResolvedValue(undefined)}
  />
);
```

- [ ] **Step 2: Implement dialog**

Behavior:

- No browser-side SSH.
- Save validates host, user, and port range 1-65535.
- Check SSH calls parent callback.
- Display result from parent if provided.
- Keep styling consistent with reference dark modal.

- [ ] **Step 3: Run tests**

Run:

```bash
cd mobile-web && npm test -- --run src/components/RemoteSettingsDialog.test.tsx
cd mobile-web && npm run typecheck
```

Expected: pass.

- [ ] **Step 4: Commit**

Run:

```bash
git add mobile-web/src/components/RemoteSettingsDialog.tsx mobile-web/src/components/RemoteSettingsDialog.test.tsx
git commit -m "feat: add remote settings dialog"
```

---

## Task 2D: Shell Component Parity Polish

**Owner:** Shell UI subagent.

**Depends on:** Task 1E.

**Files:**

- Modify: `mobile-web/src/components/OpencodeChatHeader.tsx`
- Modify: `mobile-web/src/components/OpencodeChatInput.tsx`
- Modify: `mobile-web/src/components/OpencodeSidebar.tsx`
- Modify: `mobile-web/src/components/OpencodeWelcomePage.tsx`
- Modify: `mobile-web/src/components/OpencodeShell.test.tsx`

**Goal:** Bring copied shell pieces closer to the reference app and this product's wording.

- [ ] **Step 1: Add tests for stop/settings behavior**

Extend `OpencodeShell.test.tsx`:

- Header stop button calls `onStop`.
- Header settings menu calls `onOpenRemoteSettings`.
- Input send button becomes stop when `isSending`.
- Sidebar delete button is disabled when only one conversation exists.

- [ ] **Step 2: Implement behavior**

Keep prop names local and explicit:

- `onStop`
- `onOpenRemoteSettings`
- `isSending`
- `connectionStatusText`
- `messageCount`

Do not import API functions in shell components.

- [ ] **Step 3: Run tests**

Run:

```bash
cd mobile-web && npm test -- --run src/components/OpencodeShell.test.tsx
cd mobile-web && npm run typecheck
```

Expected: pass.

- [ ] **Step 4: Commit**

Run:

```bash
git add mobile-web/src/components/OpencodeChatHeader.tsx mobile-web/src/components/OpencodeChatInput.tsx mobile-web/src/components/OpencodeSidebar.tsx mobile-web/src/components/OpencodeWelcomePage.tsx mobile-web/src/components/OpencodeShell.test.tsx
git commit -m "feat: refine opencode shell interactions"
```

---

## Task 3: `/web` App Integration And `/debug` Preservation

**Owner:** Integration subagent.

**Run only after:** Wave 1 and Wave 2 are merged.

**Files:**

- Modify: `mobile-web/src/App.tsx`
- Modify: `mobile-web/src/App.test.tsx`
- Modify: `mobile-web/src/state.ts` if selectors need active-session filtering
- Modify: `mobile-web/src/styles.css`

**Goal:** Replace the current `/web` `ProductShell` with the new opencode-style shell and keep `/debug` using the old raw testing UI.

- [ ] **Step 1: Add failing integration tests**

In `mobile-web/src/App.test.tsx`, add or update tests proving:

- `/web` calls `GET /api/sessions` on load.
- `/web` does not eagerly `POST /api/sessions` when no sessions exist.
- first send creates a session and sends `/agent-turn`.
- selecting a session calls `GET /api/sessions/{id}/messages`.
- `?session=session-1` selects an existing session.
- pending approval renders inline in the chat content.
- old text `工具活动` is not visible on `/web`.
- `/debug` still renders diagnostics/raw testing affordances.

- [ ] **Step 2: Run tests and confirm failure**

Run:

```bash
cd mobile-web && npm test -- --run src/App.test.tsx
```

Expected before integration: failures around session loading and new UI text.

- [ ] **Step 3: Integrate session lifecycle**

In `App.tsx`:

- Load sessions on startup.
- Select URL session when available.
- Select newest session otherwise.
- Do not create a session until user sends or clicks new conversation.
- Load messages on session selection.
- Update `?session=` with `window.history.replaceState`.
- Generate first-message title hints using `buildConversationTitleHint`.
- Use `updateSessionTitle` when existing title is empty or placeholder.
- Use `deleteSession` for sidebar delete.

- [ ] **Step 4: Integrate timeline rendering**

In `App.tsx`:

- Build conversations via adapter.
- Build chat messages/tool parts via adapter.
- Render `OpencodeChatMessage` for user/assistant/tool timeline entries.
- Render `InlinePermissionMessage` for pending approvals in the same scroll column.
- Render loading bubble while `busy === "agent"`.
- Remove `/web` rendering of `ProductShell` and `ToolActivity`.
- Keep old debug UI behind `isDebugRoute`.

- [ ] **Step 5: Integrate controls**

Wire:

- `OpencodeChatInput.onSendMessage` -> `sendAgentTurn`
- sending stop button -> `stopAgentTurn`
- header stop -> `stopAgentTurn`
- settings menu -> `RemoteSettingsDialog`
- permission once -> `approve_once`
- permission always -> `approve_session`
- permission reject -> `reject_stop`
- SSH save/check -> existing `updateSshTarget` and `checkSshTarget`

- [ ] **Step 6: Run integration tests**

Run:

```bash
cd mobile-web && npm test -- --run src/App.test.tsx
cd mobile-web && npm run typecheck
```

Expected: pass.

- [ ] **Step 7: Commit**

Run:

```bash
git add mobile-web/src/App.tsx mobile-web/src/App.test.tsx mobile-web/src/state.ts mobile-web/src/styles.css
git commit -m "feat: replace mobile web with opencode chat shell"
```

---

## Task 4A: Mobile Layout And Overflow Polish

**Owner:** CSS/polish subagent.

**Run after:** Task 3.

**Files:**

- Modify: `mobile-web/src/styles.css`
- Modify: `mobile-web/src/styles/markdown.css`
- Modify: `mobile-web/src/styles/message.css`
- Modify: component files only if CSS cannot solve overflow

**Goal:** Make the final `/web` layout usable on mobile widths and with long commands/output.

- [ ] **Step 1: Add or update component tests**

Add tests that assert:

- long command text is present inside a scrollable/pre block
- completed tool output is hidden until expanded
- sidebar can close on mobile overlay action
- no `/web` layout renders `product-shell__tool-activity-panel`

- [ ] **Step 2: Polish CSS**

Required CSS behavior:

- `body` and root use `height: 100vh` without horizontal overflow.
- main chat column uses `min-width: 0`.
- code/pre blocks use `overflow-x: auto`.
- cards use `overflow-wrap: anywhere` only for prose/metadata, not for shell output that should scroll.
- composer remains visible at bottom.
- mobile sidebar overlays content and uses a scrim.
- no text overlaps buttons on 375px width.

- [ ] **Step 3: Run tests/build**

Run:

```bash
cd mobile-web && npm test
cd mobile-web && npm run typecheck
cd mobile-web && npm run build
```

Expected: pass.

- [ ] **Step 4: Commit**

Run:

```bash
git add mobile-web/src/styles.css mobile-web/src/styles/markdown.css mobile-web/src/styles/message.css mobile-web/src/components mobile-web/src/App.test.tsx
git commit -m "fix: polish mobile opencode chat layout"
```

---

## Task 4B: End-To-End Tests And Smoke Updates

**Owner:** Verification subagent.

**Run after:** Task 3.

**Files:**

- Modify: `mobile-web/src/App.test.tsx`
- Modify: `scripts/mobile_web_ai_chat_smoke_test.py` if it asserts old `/web` text
- Modify: docs only if smoke command text changed

**Goal:** Ensure automated evidence covers the new product UI and `/debug` preservation.

- [ ] **Step 1: Run existing verification**

Run:

```bash
cd mobile-web && npm test
cd mobile-web && npm run typecheck
cd mobile-web && npm run build
python3 scripts/mobile_web_ai_chat_smoke_test.py
```

Record any failures caused by changed visible text.

- [ ] **Step 2: Update smoke expectations**

If the smoke test expects old `/web` labels such as `工具活动`, update it to assert:

- `/web` contains `DeepSeek 远程 Linux`
- `/web` contains chat input placeholder
- `/web` does not contain raw debug-only panel headings
- `/debug` contains diagnostics/raw testing headings

- [ ] **Step 3: Run final targeted verification**

Run:

```bash
cd mobile-web && npm test
cd mobile-web && npm run typecheck
cd mobile-web && npm run build
python3 scripts/mobile_web_ai_chat_smoke_test.py
cargo test -p deepseek-mobile-web-server --test http_api --all-features
```

Expected: pass.

- [ ] **Step 4: Commit**

Run:

```bash
git add mobile-web/src/App.test.tsx scripts/mobile_web_ai_chat_smoke_test.py docs
git commit -m "test: cover opencode mobile web chat flow"
```

---

## Task 4C: Docs And Product Notes

**Owner:** Docs subagent.

**Run after:** Task 3.

**Files:**

- Modify: `docs/linux-first-mobile-task-tree.md`
- Modify: `docs/mobile-porting-plan.md` if it references the old product UI
- Modify: `docs/mobile-validation-checklists.md`
- Modify: `mobile-web/README.md` if present and relevant

**Goal:** Update docs to describe the new `/web` and `/debug` split without overclaiming platform support.

- [ ] **Step 1: Search stale wording**

Run:

```bash
rg -n "Tool Activity|工具活动|ProductShell|/debug|/web|mobile web" docs mobile-web/README.md
```

- [ ] **Step 2: Update docs**

Required wording:

- `/web` is the formal chat UI.
- `/debug` is the raw testing surface.
- Tools and approvals render inline in the chat timeline.
- Browser remains a thin client; Rust server controls remote execution.
- Evidence is Linux/LAN web unless user-run manual tests prove more.

- [ ] **Step 3: Run docs diff check**

Run:

```bash
git diff --check -- docs mobile-web/README.md
```

Expected: no whitespace errors.

- [ ] **Step 4: Commit**

Run:

```bash
git add docs mobile-web/README.md
git commit -m "docs: document opencode-style mobile web chat"
```

---

## Task 5: Parent Review And Final Verification

**Owner:** Parent coordinator only.

**Run after:** all implementation tasks merge.

**Goal:** Verify the integrated result and catch cross-task regressions.

- [ ] **Step 1: Inspect final diff**

Run:

```bash
git status --short
git log --oneline -10
git diff --stat HEAD~10..HEAD
```

Check for accidental edits outside the planned files.

- [ ] **Step 2: Run frontend verification**

Run:

```bash
cd mobile-web && npm test
cd mobile-web && npm run typecheck
cd mobile-web && npm run build
```

Expected: pass.

- [ ] **Step 3: Run backend verification**

Run:

```bash
cargo test -p deepseek-mobile-web-server --all-features
cargo fmt --all --check
```

Expected: pass.

- [ ] **Step 4: Run smoke test**

Run:

```bash
python3 scripts/mobile_web_ai_chat_smoke_test.py
```

Expected: pass.

- [ ] **Step 5: Optional full workspace check**

Run if time allows:

```bash
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features
```

Expected: pass. If this is too slow or fails in unrelated areas, record the exact failing command and reason.

- [ ] **Step 6: Manual UI check**

Start the server according to project docs, open `/web`, and check:

- 375px, 390px, and desktop widths.
- Sidebar opens/closes.
- New conversation works.
- Long command approval card stays inside viewport.
- Tool output is inline and collapsible.
- SSH settings dialog saves/checks target through server API.
- `/debug` still exposes testing controls.

- [ ] **Step 7: Final report**

Report:

- commits made
- verification commands and results
- any skipped checks
- any known backend/product gaps

