import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";

class MockEventSource {
  static instances: MockEventSource[] = [];
  onopen: (() => void) | null = null;
  onerror: (() => void) | null = null;
  onmessage: ((event: MessageEvent<string>) => void) | null = null;
  listeners = new Map<string, (event: MessageEvent<string>) => void>();

  constructor(readonly url: string) {
    MockEventSource.instances.push(this);
  }

  addEventListener = vi.fn((type: string, listener: (event: MessageEvent<string>) => void) => {
    this.listeners.set(type, listener);
  });
  removeEventListener = vi.fn();
  close = vi.fn();
}

function jsonResponse(body: unknown, init: ResponseInit = {}): Response {
  return new Response(JSON.stringify(body), {
    headers: { "content-type": "application/json" },
    ...init
  });
}

type Session = {
  id: string;
  title: string;
  created_at_ms: number;
  updated_at_ms: number;
};

type HarnessOptions = {
  sessions?: Session[];
  messages?: Record<string, unknown[]>;
  agentResponse?: unknown;
  failAgentTurn?: boolean;
};

function createFetchHarness(options: HarnessOptions = {}) {
  const sessions = [...(options.sessions ?? [])];
  const messages = { ...(options.messages ?? {}) };
  const agentResponse =
    options.agentResponse ?? {
      session_id: "session-1",
      turn_id: "turn-1",
      status: "completed",
      assistant_text: "当前远程设备运行 Linux。",
      executed_tools: [],
      pending_approvals: []
    };
  let createCount = 0;

  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = String(input);
    const method = init?.method ?? "GET";

    if (url === "/health") {
      return jsonResponse({ status: "ok", service: "mobile-web", protocol: "http", model: "test-model" });
    }
    if (url === "/api/diagnostics/presets") {
      return jsonResponse([]);
    }
    if (url === "/api/ssh/target" && method === "GET") {
      return jsonResponse({ host: "192.168.30.244", user: "root", port: 22, key_present: true });
    }
    if (url === "/api/ssh/target" && method === "PUT") {
      const body = JSON.parse(String(init?.body ?? "{}"));
      return jsonResponse({ ...body, key_present: true });
    }
    if (url === "/api/ssh/check" && method === "POST") {
      return jsonResponse({
        status: "reachable",
        target: { host: "192.168.30.245", user: "root", port: 22, key_present: true },
        check_id: "check-1",
        command: "true",
        requires_approval: false,
        exit_code: 0,
        duration_ms: 12,
        timed_out: false
      });
    }
    if (url === "/api/audit/recent") {
      return jsonResponse([]);
    }
    if (url === "/api/sessions" && method === "GET") {
      return jsonResponse(sessions);
    }
    if (url === "/api/sessions" && method === "POST") {
      createCount += 1;
      const created = {
        id: `created-${createCount}`,
        title: "New chat",
        created_at_ms: 100 + createCount,
        updated_at_ms: 100 + createCount
      };
      sessions.unshift(created);
      messages[created.id] ??= [];
      return jsonResponse(created);
    }

    const messagesMatch = url.match(/^\/api\/sessions\/([^/]+)\/messages$/);
    if (messagesMatch && method === "GET") {
      return jsonResponse(messages[decodeURIComponent(messagesMatch[1])] ?? []);
    }

    const sessionMatch = url.match(/^\/api\/sessions\/([^/]+)$/);
    if (sessionMatch && method === "PATCH") {
      const sessionId = decodeURIComponent(sessionMatch[1]);
      const body = JSON.parse(String(init?.body ?? "{}"));
      const updated = {
        ...(sessions.find((session) => session.id === sessionId) ?? {
          id: sessionId,
          created_at_ms: 1,
          updated_at_ms: 1
        }),
        title: body.title
      };
      const index = sessions.findIndex((session) => session.id === sessionId);
      if (index >= 0) {
        sessions[index] = updated;
      }
      return jsonResponse(updated);
    }
    if (sessionMatch && method === "DELETE") {
      const sessionId = decodeURIComponent(sessionMatch[1]);
      const index = sessions.findIndex((session) => session.id === sessionId);
      if (index >= 0) {
        sessions.splice(index, 1);
      }
      delete messages[sessionId];
      return new Response(null, { status: 204 });
    }

    const agentMatch = url.match(/^\/api\/sessions\/([^/]+)\/agent-turn$/);
    if (agentMatch && method === "POST") {
      if (options.failAgentTurn) {
        return jsonResponse({ message: "agent offline" }, { status: 503, statusText: "Service Unavailable" });
      }
      return jsonResponse({ ...(agentResponse as object), session_id: decodeURIComponent(agentMatch[1]) });
    }

    const stopMatch = url.match(/^\/api\/sessions\/([^/]+)\/agent-turns\/([^/]+)\/stop$/);
    if (stopMatch && method === "POST") {
      return jsonResponse({ status: "stopping" });
    }

    const approvalMatch = url.match(/^\/api\/approvals\/([^/]+)\/respond$/);
    if (approvalMatch && method === "POST") {
      const id = decodeURIComponent(approvalMatch[1]);
      const body = JSON.parse(String(init?.body ?? "{}"));
      return jsonResponse({
        approval: { id, session_id: "session-1", command: "uname -a", created_at_ms: 100, status: body.response },
        status: body.response,
        result: body.response === "reject_stop" ? { summary: "Execution stopped." } : { assistant_text: "Accepted." }
      });
    }

    return jsonResponse({ message: `Unhandled ${method} ${url}` }, { status: 500, statusText: "Unhandled" });
  });

  vi.stubGlobal("fetch", fetchMock);
  return fetchMock;
}

function message(id: string, sessionId: string, role: string, text: string) {
  return {
    id,
    session_id: sessionId,
    role,
    created_at_ms: Number(id.replace(/\D/g, "")) || 1,
    parts: [{ id: `${id}-part`, kind: "text", text }]
  };
}

describe("App opencode web integration", () => {
  beforeEach(() => {
    MockEventSource.instances = [];
    vi.stubGlobal("EventSource", MockEventSource);
    localStorage.clear();
    window.history.replaceState({}, "", "/web");
  });

  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
    vi.useRealTimers();
  });

  it("/web renders the formal chat shell without the old separate tool activity panel", async () => {
    const fetchMock = createFetchHarness({ sessions: [] });

    render(<App />);

    await screen.findByText("暂无对话");
    expect(screen.getByRole("banner").textContent).toContain("DeepSeek 远程 Linux");
    expect(screen.getByRole("region", { name: "远程 Linux 对话工作区" })).toBeTruthy();
    expect(screen.getByRole("region", { name: "消息输入区" })).toBeTruthy();
    expect(screen.getByPlaceholderText("输入消息...")).toBeTruthy();
    await waitFor(() => expect(fetchMock).toHaveBeenCalledWith("/api/sessions", { headers: {} }));
    expect(fetchMock.mock.calls.some(([url, init]) => url === "/api/sessions" && init?.method === "POST")).toBe(false);
    expect(screen.queryByRole("region", { name: "Tool activity" })).toBeNull();
    expect(screen.queryByRole("complementary", { name: "工具活动" })).toBeNull();
    expect(screen.queryByText("Tool Activity")).toBeNull();
    expect(screen.queryByText("工具活动")).toBeNull();
  });

  it("first send without an active session creates a titled session and sends the agent turn", async () => {
    const fetchMock = createFetchHarness({ sessions: [] });

    render(<App />);

    const composer = await screen.findByPlaceholderText("输入消息...");
    fireEvent.change(composer, { target: { value: "### 检查网络\n请执行基础诊断" } });
    fireEvent.click(screen.getByRole("button", { name: "发送消息" }));

    await waitFor(() => expect(fetchMock).toHaveBeenCalledWith("/api/sessions", expect.objectContaining({ method: "POST" })));
    await waitFor(() =>
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/sessions/created-1",
        expect.objectContaining({ method: "PATCH", body: JSON.stringify({ title: "检查网络" }) })
      )
    );
    await waitFor(() => expect(fetchMock).toHaveBeenCalledWith("/api/sessions/created-1/agent-turn", expect.anything()));
    const agentCall = fetchMock.mock.calls.find(([url]) => url === "/api/sessions/created-1/agent-turn");
    expect(JSON.parse(agentCall?.[1]?.body as string)).toMatchObject({ message: "### 检查网络\n请执行基础诊断" });
  });

  it("selects the URL session and loads messages for sidebar selection", async () => {
    const fetchMock = createFetchHarness({
      sessions: [
        { id: "session-1", title: "Router", created_at_ms: 1, updated_at_ms: 1 },
        { id: "session-2", title: "Network", created_at_ms: 2, updated_at_ms: 2 }
      ],
      messages: {
        "session-1": [message("m1", "session-1", "assistant", "loaded session one")],
        "session-2": [message("m2", "session-2", "assistant", "loaded session two")]
      }
    });
    window.history.replaceState({}, "", "/web?session=session-1");

    render(<App />);

    await screen.findByText("loaded session one");
    fireEvent.click(screen.getByText("Network"));

    await screen.findByText("loaded session two");
    await waitFor(() => expect(fetchMock).toHaveBeenCalledWith("/api/sessions/session-2/messages", { headers: {} }));
  });

  it("keeps optimistic product messages scoped to the active conversation", async () => {
    createFetchHarness({
      sessions: [
        { id: "session-1", title: "Router", created_at_ms: 1, updated_at_ms: 1 },
        { id: "session-2", title: "Network", created_at_ms: 2, updated_at_ms: 2 }
      ],
      messages: {
        "session-1": [message("m1", "session-1", "assistant", "loaded session one")],
        "session-2": [message("m2", "session-2", "assistant", "loaded session two")]
      },
      agentResponse: {
        session_id: "session-1",
        turn_id: "turn-1",
        status: "completed",
        assistant_text: "session one local answer",
        executed_tools: [],
        pending_approvals: []
      }
    });
    window.history.replaceState({}, "", "/web?session=session-1");

    render(<App />);

    const composer = await screen.findByPlaceholderText("输入消息...");
    fireEvent.change(composer, { target: { value: "session one local question" } });
    fireEvent.click(screen.getByRole("button", { name: "发送消息" }));

    expect(await screen.findByText("session one local question")).toBeTruthy();
    expect(await screen.findByText("session one local answer")).toBeTruthy();

    fireEvent.click(screen.getByText("Network"));

    await screen.findByText("loaded session two");
    expect(screen.queryByText("session one local question")).toBeNull();
    expect(screen.queryByText("session one local answer")).toBeNull();
  });

  it("shows the welcome page for an active conversation with an empty timeline", async () => {
    createFetchHarness({
      sessions: [{ id: "session-1", title: "Empty", created_at_ms: 1, updated_at_ms: 1 }],
      messages: { "session-1": [] }
    });

    render(<App />);

    expect(await screen.findByLabelText("欢迎页")).toBeTruthy();
    expect(screen.getByText("从一个诊断问题开始")).toBeTruthy();
  });

  it("sidebar new and delete conversations call the session APIs", async () => {
    const fetchMock = createFetchHarness({
      sessions: [
        { id: "session-1", title: "Router", created_at_ms: 1, updated_at_ms: 1 },
        { id: "session-2", title: "Network", created_at_ms: 2, updated_at_ms: 2 }
      ]
    });

    render(<App />);

    fireEvent.click(await screen.findByRole("button", { name: "新建" }));
    await waitFor(() => expect(fetchMock).toHaveBeenCalledWith("/api/sessions", expect.objectContaining({ method: "POST" })));

    fireEvent.click(await screen.findByRole("button", { name: "删除 Router" }));
    await waitFor(() => expect(fetchMock).toHaveBeenCalledWith("/api/sessions/session-1", expect.objectContaining({ method: "DELETE" })));
  });

  it("header settings opens RemoteSettingsDialog and saves/checks SSH through callbacks", async () => {
    const fetchMock = createFetchHarness({ sessions: [] });

    render(<App />);

    fireEvent.click(await screen.findByRole("button", { name: "远程设置" }));
    const dialog = await screen.findByRole("dialog", { name: "Remote settings" });
    expect(within(dialog).getByText("test-model")).toBeTruthy();

    fireEvent.change(within(dialog).getByLabelText("Host"), { target: { value: "192.168.30.245" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Save target" }));
    await waitFor(() => expect(fetchMock).toHaveBeenCalledWith("/api/ssh/target", expect.objectContaining({ method: "PUT" })));

    fireEvent.click(within(dialog).getByRole("button", { name: "Check SSH" }));
    await waitFor(() => expect(fetchMock).toHaveBeenCalledWith("/api/ssh/check", expect.objectContaining({ method: "POST" })));
    expect(await within(dialog).findByText("reachable")).toBeTruthy();
  });

  it("composer sends and stops the active agent turn", async () => {
    const fetchMock = createFetchHarness({
      sessions: [{ id: "session-1", title: "Router", created_at_ms: 1, updated_at_ms: 1 }],
      agentResponse: {
        session_id: "session-1",
        turn_id: "turn-1",
        status: "awaiting_approval",
        assistant_text: "I need approval.",
        executed_tools: [],
        pending_approvals: [
          {
            id: "approval-1",
            session_id: "session-1",
            command: "uname -a",
            created_at_ms: 100,
            status: "pending",
            agent_turn_id: "turn-1"
          }
        ]
      }
    });

    render(<App />);

    const composer = await screen.findByPlaceholderText("输入消息...");
    fireEvent.change(composer, { target: { value: "check host" } });
    fireEvent.click(screen.getByRole("button", { name: "发送消息" }));
    await screen.findByLabelText("需要授权");
    fireEvent.click(screen.getByRole("button", { name: "停止生成" }));

    await waitFor(() =>
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/sessions/session-1/agent-turns/turn-1/stop",
        expect.objectContaining({ method: "POST" })
      )
    );
  });

  it("renders pending approvals inline and maps responses to server actions", async () => {
    const fetchMock = createFetchHarness({
      sessions: [{ id: "session-1", title: "Router", created_at_ms: 1, updated_at_ms: 1 }],
      agentResponse: {
        session_id: "session-1",
        turn_id: "turn-1",
        status: "awaiting_approval",
        assistant_text: "I need approval.",
        executed_tools: [],
        pending_approvals: [
          { id: "approval-1", session_id: "session-1", command: "uname -a", created_at_ms: 100, status: "pending" },
          { id: "approval-2", session_id: "session-1", command: "reboot", created_at_ms: 101, status: "pending" },
          { id: "approval-3", session_id: "session-1", command: "date", created_at_ms: 102, status: "pending" }
        ]
      }
    });

    render(<App />);

    const composer = await screen.findByPlaceholderText("输入消息...");
    fireEvent.change(composer, { target: { value: "inspect" } });
    fireEvent.click(screen.getByRole("button", { name: "发送消息" }));

    expect(await screen.findAllByLabelText("需要授权")).toHaveLength(3);
    fireEvent.click(screen.getAllByRole("button", { name: "仅这次执行" })[0]);
    await waitFor(() => {
      const bodies = fetchMock.mock.calls.filter(([url]) => String(url).includes("/api/approvals/"));
      expect(bodies).toHaveLength(1);
    });
    fireEvent.click(screen.getAllByRole("button", { name: "本会话都允许" })[0]);
    await waitFor(() => {
      const bodies = fetchMock.mock.calls.filter(([url]) => String(url).includes("/api/approvals/"));
      expect(bodies).toHaveLength(2);
    });
    fireEvent.click(screen.getAllByRole("button", { name: "拒绝并停止" })[0]);

    await waitFor(() => {
      const bodies = fetchMock.mock.calls
        .filter(([url]) => String(url).includes("/api/approvals/"))
        .map(([, init]) => JSON.parse(init?.body as string));
      expect(bodies).toEqual([{ response: "approve_once" }, { response: "approve_session" }, { response: "reject_stop" }]);
    });
  });

  it("renders tool activity inline through chat messages and omits the old Tool Activity panel", async () => {
    createFetchHarness({
      sessions: [{ id: "session-1", title: "Router", created_at_ms: 1, updated_at_ms: 1 }],
      agentResponse: {
        session_id: "session-1",
        turn_id: "turn-1",
        status: "completed",
        assistant_text: "Tool finished.",
        executed_tools: [{ tool: "bash", command: "df -h", requires_approval: false, exit_code: 0, status: "completed" }],
        pending_approvals: []
      }
    });

    render(<App />);

    const composer = await screen.findByPlaceholderText("输入消息...");
    fireEvent.change(composer, { target: { value: "disk" } });
    fireEvent.click(screen.getByRole("button", { name: "发送消息" }));

    expect(await screen.findByText("df -h")).toBeTruthy();
    expect(screen.queryByRole("region", { name: "Tool activity" })).toBeNull();
    expect(screen.queryByText("Tool Activity")).toBeNull();
  });

  it("registers session.deleted and agent tool event listeners", async () => {
    createFetchHarness({ sessions: [] });

    render(<App />);

    await screen.findByText("暂无对话");
    const registered = MockEventSource.instances[0].addEventListener.mock.calls.map(([type]) => type);
    expect(registered).toContain("session.deleted");
    expect(registered).toContain("agent.tool.completed");
    expect(registered).toContain("agent.tool.failed");
  });

  it("auto-dismisses web errors as transient toast feedback", async () => {
    createFetchHarness({
      sessions: [{ id: "session-1", title: "Router", created_at_ms: 1, updated_at_ms: 1 }],
      failAgentTurn: true
    });

    render(<App />);

    const composer = await screen.findByPlaceholderText("输入消息...");
    fireEvent.change(composer, { target: { value: "trigger failure" } });
    fireEvent.click(screen.getByRole("button", { name: "发送消息" }));

    expect(await screen.findByText("agent offline")).toBeTruthy();

    await waitFor(() => expect(screen.queryByText("agent offline")).toBeNull(), { timeout: 5200 });
  }, 6500);

  it("/debug still exposes raw diagnostics and testing affordances", async () => {
    window.history.replaceState({}, "", "/debug");
    createFetchHarness();

    render(<App />);

    expect(await screen.findByText("Linux Web Control")).toBeTruthy();
    expect(await screen.findByText("Raw Timeline")).toBeTruthy();
    expect(screen.getAllByText("Diagnostics").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Advanced Command").length).toBeGreaterThan(0);
    expect(screen.getByText("Pending Approvals")).toBeTruthy();
    expect(screen.getByText("Tool Activity")).toBeTruthy();
    expect(screen.queryByRole("navigation", { name: "会话列表" })).toBeNull();
  });
});
