import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";

class MockEventSource {
  static instances: MockEventSource[] = [];
  onopen: (() => void) | null = null;
  onerror: (() => void) | null = null;
  onmessage: ((event: MessageEvent<string>) => void) | null = null;

  constructor(readonly url: string) {
    MockEventSource.instances.push(this);
  }

  addEventListener = vi.fn();
  removeEventListener = vi.fn();
  close = vi.fn();
}

function jsonResponse(body: unknown, init: ResponseInit = {}): Response {
  return new Response(JSON.stringify(body), {
    headers: { "content-type": "application/json" },
    ...init
  });
}

function bootstrapFetch(agentResponse: Promise<Response> | Response = jsonResponse({
  session_id: "session-1",
  turn_id: "turn-1",
  status: "completed",
  assistant_text: "当前远程设备运行 Linux。",
  executed_tools: [],
  pending_approvals: []
})) {
  const fetchMock = vi
    .fn()
    .mockResolvedValueOnce(jsonResponse({ status: "ok", service: "mobile-web", protocol: "http", model: "test" }))
    .mockResolvedValueOnce(jsonResponse([]))
    .mockResolvedValueOnce(jsonResponse({ host: "192.168.30.244", user: "root", port: 22, key_present: true }))
    .mockResolvedValueOnce(jsonResponse([]))
    .mockResolvedValueOnce(jsonResponse({ id: "session-1", title: "Mobile SSH", created_at_ms: 1, updated_at_ms: 1 }))
    .mockResolvedValueOnce(agentResponse);

  vi.stubGlobal("fetch", fetchMock);
  return fetchMock;
}

describe("App agent chat", () => {
  beforeEach(() => {
    MockEventSource.instances = [];
    vi.stubGlobal("EventSource", MockEventSource);
    localStorage.clear();
    window.history.replaceState({}, "", "/debug");
  });

  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
  });

  it("submits the primary composer to the agent turn API instead of the raw command field", async () => {
    const fetchMock = bootstrapFetch();

    render(<App />);

    const composer = await screen.findByPlaceholderText("Ask the remote Linux device...");
    fireEvent.change(composer, { target: { value: "请问当前运行在什么系统？" } });
    fireEvent.submit(composer.closest("form")!);

    await waitFor(() => expect(fetchMock).toHaveBeenCalledWith("/api/sessions/session-1/agent-turn", expect.anything()));

    const agentCall = fetchMock.mock.calls.find(([url]) => url === "/api/sessions/session-1/agent-turn");
    expect(JSON.parse(agentCall?.[1]?.body as string)).toEqual({ message: "请问当前运行在什么系统？" });
    expect(fetchMock.mock.calls.some(([url]) => url === "/api/commands/prepare")).toBe(false);
    expect(screen.getAllByText("You").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Assistant").length).toBeGreaterThan(0);
    expect((composer as HTMLTextAreaElement).value).toBe("");
  });

  it("keeps the agent prompt editable on network failure", async () => {
    const fetchMock = bootstrapFetch(Promise.resolve(jsonResponse({ message: "offline" }, { status: 503, statusText: "Service Unavailable" })));

    render(<App />);

    const composer = await screen.findByPlaceholderText("Ask the remote Linux device...");
    fireEvent.change(composer, { target: { value: "请问当前运行在什么系统？" } });
    fireEvent.submit(composer.closest("form")!);

    await waitFor(() => expect(fetchMock).toHaveBeenCalledWith("/api/sessions/session-1/agent-turn", expect.anything()));
    await screen.findByText("offline");

    expect((composer as HTMLTextAreaElement).value).toBe("请问当前运行在什么系统？");
  });

  it("shows pending agent approvals in the existing approval cards", async () => {
    bootstrapFetch(jsonResponse({
      session_id: "session-1",
      turn_id: "turn-1",
      status: "awaiting_approval",
      assistant_text: "I need approval before running that command.",
      executed_tools: [],
      pending_approvals: [{
        id: "approval-1",
        session_id: "session-1",
        command: "uname -a",
        created_at_ms: 100,
        status: "pending"
      }]
    }));

    render(<App />);

    const composer = await screen.findByPlaceholderText("Ask the remote Linux device...");
    fireEvent.change(composer, { target: { value: "what system is this?" } });
    fireEvent.submit(composer.closest("form")!);

    expect(await screen.findAllByText("I need approval before running that command.")).not.toHaveLength(0);
    expect(screen.getByText("Approve once")).toBeTruthy();
    expect(screen.getByText("Approve session")).toBeTruthy();
    expect(screen.getByText("Reject and stop")).toBeTruthy();
    expect(screen.getAllByText("uname -a").length).toBeGreaterThan(0);
  });

  it("shows the approval response summary without waiting for sse", async () => {
    const fetchMock = bootstrapFetch(jsonResponse({
      session_id: "session-1",
      turn_id: "turn-1",
      status: "awaiting_approval",
      assistant_text: "I need approval before running that command.",
      executed_tools: [],
      pending_approvals: [{
        id: "approval-1",
        session_id: "session-1",
        command: "opkg update",
        created_at_ms: 100,
        status: "pending"
      }]
    }));
    fetchMock.mockResolvedValueOnce(jsonResponse({
      approval: {
        id: "approval-1",
        session_id: "session-1",
        command: "opkg update",
        created_at_ms: 100,
        status: "approved"
      },
      status: "approved",
      result: {
        summary: "本轮远程命令已全部执行完成。结果如下：\n\n1. opkg update completed"
      }
    }));

    render(<App />);

    const composer = await screen.findByPlaceholderText("Ask the remote Linux device...");
    fireEvent.change(composer, { target: { value: "请更新软件包索引" } });
    fireEvent.submit(composer.closest("form")!);
    fireEvent.click(await screen.findByRole("button", { name: "Approve once" }));

    expect(await screen.findAllByText(/本轮远程命令已全部执行完成/)).not.toHaveLength(0);
  });

  it("sends approve_session and reject_stop approval responses from the web product surface", async () => {
    window.history.replaceState({}, "", "/web");
    const fetchMock = bootstrapFetch(jsonResponse({
      session_id: "session-1",
      turn_id: "turn-1",
      status: "awaiting_approval",
      assistant_text: "I need approval before running that command.",
      executed_tools: [],
      pending_approvals: [
        {
          id: "approval-1",
          session_id: "session-1",
          command: "uname -a",
          created_at_ms: 100,
          status: "pending",
          risk_level: "low",
          risk_reason: "Reads system information.",
          target_label: "root@192.168.30.244:22"
        },
        {
          id: "approval-2",
          session_id: "session-1",
          command: "reboot",
          created_at_ms: 101,
          status: "pending",
          risk_level: "high",
          risk_reason: "Restarts the target device.",
          target_label: "root@192.168.30.244:22"
        }
      ]
    }));
    fetchMock
      .mockResolvedValueOnce(jsonResponse({
        approval: {
          id: "approval-1",
          session_id: "session-1",
          command: "uname -a",
          created_at_ms: 100,
          status: "approved_for_session"
        },
        status: "approved_for_session",
        result: { assistant_text: "Session approval accepted." }
      }))
      .mockResolvedValueOnce(jsonResponse({
        approval: {
          id: "approval-2",
          session_id: "session-1",
          command: "reboot",
          created_at_ms: 101,
          status: "rejected"
        },
        status: "rejected",
        result: { summary: "Execution stopped." }
      }));

    render(<App />);

    const composer = await screen.findByPlaceholderText("Ask the remote Linux device...");
    fireEvent.change(composer, { target: { value: "inspect and maybe reboot" } });
    fireEvent.submit(composer.closest("form")!);
    fireEvent.click((await screen.findAllByRole("button", { name: "本会话都允许" }))[0]);
    await waitFor(() => {
      const responseBodies = fetchMock.mock.calls
        .filter(([url]) => String(url).includes("/api/approvals/"))
        .map(([, init]) => JSON.parse(init?.body as string));
      expect(responseBodies).toEqual([{ response: "approve_session" }]);
    });
    fireEvent.click(await screen.findByRole("button", { name: "拒绝并停止" }));

    await waitFor(() => {
      const responseBodies = fetchMock.mock.calls
        .filter(([url]) => String(url).includes("/api/approvals/"))
        .map(([, init]) => JSON.parse(init?.body as string));
      expect(responseBodies).toEqual([{ response: "approve_session" }, { response: "reject_stop" }]);
    });
    expect((await screen.findByRole("region", { name: "Final answer" })).textContent).toContain("Execution stopped.");
  });

  it("disables the send button while an agent turn is pending", async () => {
    let resolveAgentTurn: (response: Response) => void = () => undefined;
    const agentTurn = new Promise<Response>((resolve) => {
      resolveAgentTurn = resolve;
    });
    bootstrapFetch(agentTurn);

    render(<App />);

    const composer = await screen.findByPlaceholderText("Ask the remote Linux device...");
    fireEvent.change(composer, { target: { value: "uname question" } });
    fireEvent.submit(composer.closest("form")!);

    expect((screen.getByRole("button", { name: "Sending" }) as HTMLButtonElement).disabled).toBe(true);

    resolveAgentTurn(jsonResponse({
      session_id: "session-1",
      turn_id: "turn-1",
      status: "completed",
      assistant_text: "Linux",
      executed_tools: [],
      pending_approvals: []
    }));
  });

  it("copies a compact feedback report for manual review", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText }
    });
    bootstrapFetch();

    render(<App />);

    await screen.findByText("Raw Timeline");
    fireEvent.click(screen.getByRole("button", { name: "Copy report" }));

    await waitFor(() => expect(writeText).toHaveBeenCalled());
    expect(writeText.mock.calls[0][0]).toContain("DeepSeek Mobile SSH report");
    expect(writeText.mock.calls[0][0]).toContain("Target: root@192.168.30.244:22");
    await screen.findByText("Report copied");
  });

  it("shows final answers in the chat product surface and copies only the final answer", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText }
    });
    bootstrapFetch();

    render(<App />);

    const composer = await screen.findByPlaceholderText("Ask the remote Linux device...");
    fireEvent.change(composer, { target: { value: "请问当前运行在什么系统？" } });
    fireEvent.submit(composer.closest("form")!);

    await screen.findByText("Final answer");
    expect(screen.getByRole("region", { name: "Final answer" }).textContent).toContain("当前远程设备运行 Linux。");

    fireEvent.click(screen.getByRole("button", { name: "Copy final answer" }));

    await waitFor(() => expect(writeText).toHaveBeenCalledWith("当前远程设备运行 Linux。"));
    await screen.findByText("Final answer copied");
  });

  it("keeps raw timeline secondary while exposing tool activity as the main execution view", async () => {
    bootstrapFetch();

    render(<App />);

    const toolActivity = await screen.findByRole("region", { name: "Tool activity" });
    const timelineSummary = await screen.findByText("Raw Timeline");

    expect(timelineSummary.closest("details")).toBeTruthy();
    expect(toolActivity.compareDocumentPosition(timelineSummary)).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
  });

  it("renders the product chat UI at /web instead of the debug timeline shell", async () => {
    window.history.replaceState({}, "", "/web");
    bootstrapFetch();

    render(<App />);

    expect((await screen.findByRole("banner")).textContent).toContain("DeepSeek 远程 Linux");
    expect(screen.getByRole("navigation", { name: "会话列表" })).toBeTruthy();
    expect(screen.getByRole("main", { name: "远程 Linux 对话工作区" })).toBeTruthy();
    expect(screen.queryByText("Raw Timeline")).toBeNull();
  });

  it("keeps the debug route as the raw timeline diagnostics surface", async () => {
    window.history.replaceState({}, "", "/debug");
    bootstrapFetch();

    render(<App />);

    expect(await screen.findByText("Raw Timeline")).toBeTruthy();
    expect(screen.queryByRole("navigation", { name: "会话列表" })).toBeNull();
  });

  it("shows agent server, sse, service model, and ssh target status in the web product entry", async () => {
    window.history.replaceState({}, "", "/web");
    bootstrapFetch();

    render(<App />);

    expect(await screen.findByText(/Agent Server ok/)).toBeTruthy();
    expect(screen.getByText(/SSE connecting/)).toBeTruthy();
    expect(screen.getByText(/Service mobile-web/)).toBeTruthy();
    expect(screen.getByText(/Model test/)).toBeTruthy();
    expect(screen.getByRole("button", { name: "目标设备设置" }).textContent).toContain("root@192.168.30.244:22");

    fireEvent.click(screen.getByRole("button", { name: "目标设备设置" }));
    expect(screen.getByRole("dialog", { name: "Server settings" }).textContent).toContain("mobile-web");
    expect(screen.getByRole("dialog", { name: "Server settings" }).textContent).toContain("test");
  });

  it("shows disconnected or error SSE status in the web product entry", async () => {
    window.history.replaceState({}, "", "/web");
    bootstrapFetch();

    render(<App />);

    await screen.findByText(/Agent Server ok/);
    MockEventSource.instances[0].onerror?.();

    expect((await screen.findByRole("status", { name: "执行状态" })).textContent).toContain("连接中断");
    expect(screen.getByText(/SSE error/)).toBeTruthy();
  });

  it("shows waiting status while an approval is pending in the web product entry", async () => {
    window.history.replaceState({}, "", "/web");
    bootstrapFetch(jsonResponse({
      session_id: "session-1",
      turn_id: "turn-1",
      status: "awaiting_approval",
      assistant_text: "I need approval.",
      executed_tools: [],
      pending_approvals: [{
        id: "approval-1",
        session_id: "session-1",
        command: "uname -a",
        created_at_ms: 100,
        status: "pending"
      }]
    }));

    render(<App />);

    const composer = await screen.findByPlaceholderText("Ask the remote Linux device...");
    fireEvent.change(composer, { target: { value: "check system" } });
    fireEvent.submit(composer.closest("form")!);

    await waitFor(() => {
      expect(screen.getAllByRole("status", { name: "执行状态" }).some((item) => item.textContent?.includes("等待授权"))).toBe(true);
    });
  });

  it("continues from the web product controls with an empty composer", async () => {
    window.history.replaceState({}, "", "/web");
    const fetchMock = bootstrapFetch(jsonResponse({
      session_id: "session-1",
      turn_id: "turn-continue",
      status: "completed",
      assistant_text: "Continuing.",
      executed_tools: [],
      pending_approvals: []
    }));

    render(<App />);

    await screen.findByPlaceholderText("Ask the remote Linux device...");
    fireEvent.click(screen.getByRole("button", { name: "继续" }));

    await waitFor(() => expect(fetchMock).toHaveBeenCalledWith("/api/sessions/session-1/agent-turn", expect.anything()));
    const agentCall = fetchMock.mock.calls.find(([url]) => url === "/api/sessions/session-1/agent-turn");
    expect(JSON.parse(agentCall?.[1]?.body as string)).toEqual({
      message: "请继续上一轮任务。",
      mode: "continue"
    });
  });

  it("retries from the web product controls with the latest response turn id", async () => {
    window.history.replaceState({}, "", "/web");
    const fetchMock = bootstrapFetch(jsonResponse({
      session_id: "session-1",
      turn_id: "turn-1",
      status: "completed",
      assistant_text: "First answer.",
      executed_tools: [],
      pending_approvals: []
    }));
    fetchMock.mockResolvedValueOnce(jsonResponse({
      session_id: "session-1",
      turn_id: "turn-2",
      status: "completed",
      assistant_text: "Retried.",
      executed_tools: [],
      pending_approvals: []
    }));

    render(<App />);

    const composer = await screen.findByPlaceholderText("Ask the remote Linux device...");
    fireEvent.change(composer, { target: { value: "check system" } });
    fireEvent.submit(composer.closest("form")!);
    expect(await screen.findAllByText("First answer.")).not.toHaveLength(0);

    fireEvent.click(screen.getByRole("button", { name: "重试" }));

    await waitFor(() => {
      const bodies = fetchMock.mock.calls
        .filter(([url]) => url === "/api/sessions/session-1/agent-turn")
        .map(([, init]) => JSON.parse(init?.body as string));
      expect(bodies).toEqual([
        { message: "check system" },
        { message: "", mode: "retry", retry_turn_id: "turn-1" }
      ]);
    });
  });

  it("stops the latest pending approval turn from the web product controls", async () => {
    window.history.replaceState({}, "", "/web");
    const fetchMock = bootstrapFetch(jsonResponse({
      session_id: "session-1",
      turn_id: "turn-response",
      status: "awaiting_approval",
      assistant_text: "I need approval.",
      executed_tools: [],
      pending_approvals: [{
        id: "approval-1",
        session_id: "session-1",
        command: "uname -a",
        created_at_ms: 100,
        status: "pending",
        agent_turn_id: "turn-from-approval"
      }]
    }));
    fetchMock.mockResolvedValueOnce(jsonResponse({ status: "stopping" }));

    render(<App />);

    const composer = await screen.findByPlaceholderText("Ask the remote Linux device...");
    fireEvent.change(composer, { target: { value: "check system" } });
    fireEvent.submit(composer.closest("form")!);
    await screen.findByText("I need approval.");

    fireEvent.click(screen.getByRole("button", { name: "停止" }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/sessions/session-1/agent-turns/turn-from-approval/stop",
        expect.objectContaining({ method: "POST" })
      );
    });
  });

  it("shows running status while a tool is running in the web product entry", async () => {
    window.history.replaceState({}, "", "/web");
    bootstrapFetch();

    render(<App />);

    await screen.findByText(/Agent Server ok/);
    MockEventSource.instances[0].onmessage?.(new MessageEvent("message", {
      data: JSON.stringify({
        type: "tool.started",
        payload: { id: "tool-1", command: "uname -a", status: "running" }
      })
    }));

    expect((await screen.findByRole("status", { name: "执行状态" })).textContent).toContain("执行中");
  });

  it("copies the full report from the web product entry", async () => {
    window.history.replaceState({}, "", "/web");
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText }
    });
    bootstrapFetch(jsonResponse({
      session_id: "session-1",
      turn_id: "turn-1",
      status: "awaiting_approval",
      assistant_text: "I need approval.",
      executed_tools: [],
      pending_approvals: [{
        id: "approval-1",
        session_id: "session-1",
        command: "uname -a",
        created_at_ms: 100,
        status: "pending"
      }]
    }));

    render(<App />);

    const composer = await screen.findByPlaceholderText("Ask the remote Linux device...");
    fireEvent.change(composer, { target: { value: "check system" } });
    fireEvent.submit(composer.closest("form")!);
    fireEvent.click(await screen.findByRole("button", { name: "复制完整报告" }));

    await waitFor(() => expect(writeText).toHaveBeenCalled());
    expect(writeText.mock.calls[0][0]).toContain("Target: root@192.168.30.244:22");
    expect(writeText.mock.calls[0][0]).toContain("Pending approvals: 1");
    expect(writeText.mock.calls[0][0]).not.toContain("sk-");
  });

  it("shows a manual report textbox in /web when clipboard copy fails", async () => {
    window.history.replaceState({}, "", "/web");
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText: vi.fn().mockRejectedValue(new Error("clipboard unavailable")) }
    });
    bootstrapFetch();

    render(<App />);

    fireEvent.click(await screen.findByRole("button", { name: "复制完整报告" }));

    const manualReport = await screen.findByRole("textbox", { name: "Manual copy report" });
    expect((manualReport as HTMLTextAreaElement).value).toContain("DeepSeek Mobile SSH report");
    expect((manualReport as HTMLTextAreaElement).value).toContain("Target: root@192.168.30.244:22");
    await screen.findByText("Copy failed");
  });

  it("opens product server settings and updates the ssh target", async () => {
    window.history.replaceState({}, "", "/web");
    const fetchMock = bootstrapFetch(jsonResponse({ host: "192.168.30.245", user: "root", port: 22, key_present: true }));
    fetchMock.mockResolvedValueOnce(jsonResponse({
      status: "reachable",
      target: { host: "192.168.30.245", user: "root", port: 22, key_present: true },
      check_id: "check-1",
      command: "true",
      requires_approval: false,
      exit_code: 0,
      duration_ms: 12,
      timed_out: false
    }));

    render(<App />);

    fireEvent.click(await screen.findByRole("button", { name: "目标设备设置" }));
    fireEvent.change(screen.getByLabelText("Host"), { target: { value: "192.168.30.245" } });
    fireEvent.submit(screen.getByLabelText("Host").closest("form")!);

    await waitFor(() => expect(fetchMock).toHaveBeenCalledWith("/api/ssh/target", expect.objectContaining({ method: "PUT" })));

    fireEvent.click(screen.getByRole("button", { name: "Check SSH" }));
    await waitFor(() => expect(fetchMock).toHaveBeenCalledWith("/api/ssh/check", expect.objectContaining({ method: "POST" })));
    expect(await screen.findByText("reachable")).toBeTruthy();
  });

  it("shows a final answer from an approved agent turn response", async () => {
    const fetchMock = bootstrapFetch(jsonResponse({
      session_id: "session-1",
      turn_id: "turn-1",
      status: "awaiting_approval",
      assistant_text: "I need approval before running that command.",
      executed_tools: [],
      pending_approvals: [{
        id: "approval-1",
        session_id: "session-1",
        command: "uname -a",
        created_at_ms: 100,
        status: "pending"
      }]
    }));
    fetchMock.mockResolvedValueOnce(jsonResponse({
      approval: {
        id: "approval-1",
        session_id: "session-1",
        command: "uname -a",
        created_at_ms: 100,
        status: "approved"
      },
      status: "approved",
      result: {
        assistant_text: "Linux host is ready."
      }
    }));

    render(<App />);

    const composer = await screen.findByPlaceholderText("Ask the remote Linux device...");
    fireEvent.change(composer, { target: { value: "check the host" } });
    fireEvent.submit(composer.closest("form")!);
    fireEvent.click(await screen.findByRole("button", { name: "Approve once" }));

    expect((await screen.findByRole("region", { name: "Final answer" })).textContent).toContain("Linux host is ready.");
  });
});
