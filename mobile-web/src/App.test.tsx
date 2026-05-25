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
    expect(screen.getByText("You")).toBeTruthy();
    expect(screen.getByText("Assistant")).toBeTruthy();
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

    await screen.findByText("I need approval before running that command.");
    expect(screen.getByText("Approve once")).toBeTruthy();
    expect(screen.getByText("Reject")).toBeTruthy();
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

    await screen.findByText("Timeline");
    fireEvent.click(screen.getByRole("button", { name: "Copy report" }));

    await waitFor(() => expect(writeText).toHaveBeenCalled());
    expect(writeText.mock.calls[0][0]).toContain("DeepSeek Mobile SSH report");
    expect(writeText.mock.calls[0][0]).toContain("Target: root@192.168.30.244:22");
    await screen.findByText("Report copied");
  });
});
