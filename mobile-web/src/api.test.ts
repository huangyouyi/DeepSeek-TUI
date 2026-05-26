import { describe, expect, it, vi } from "vitest";
import {
  approveCommand,
  approveCommandForSession,
  buildEventUrl,
  checkSshTarget,
  createSession,
  deleteSession,
  getDiagnosticPresets,
  listSessions,
  listMessages,
  prepareCommand,
  rejectCommand,
  rejectStopCommand,
  runDiagnostic,
  sendAgentTurn,
  stopAgentTurn,
  updateSessionTitle,
  updateSshTarget
} from "./api";

function jsonResponse(body: unknown): Response {
  return new Response(JSON.stringify(body), {
    headers: { "content-type": "application/json" }
  });
}

function findRequest(fetchMock: ReturnType<typeof vi.fn>, path: string): RequestInit {
  const request = fetchMock.mock.calls.find(([url]) => url === path)?.[1];
  if (!request) {
    throw new Error(`Expected request to ${path}`);
  }
  return request;
}

describe("api client", () => {
  it("uses shared contract routes and payload field names", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(
        jsonResponse([{ id: "session-1", title: "Mobile SSH", created_at_ms: 1, updated_at_ms: 1 }])
      )
      .mockResolvedValueOnce(jsonResponse({ id: "session-1", title: "Mobile SSH" }))
      .mockResolvedValueOnce(jsonResponse([]))
      .mockResolvedValueOnce(jsonResponse({ status: "queued" }))
      .mockResolvedValueOnce(jsonResponse({ id: "approval-1" }))
      .mockResolvedValueOnce(jsonResponse({ status: "approved" }))
      .mockResolvedValueOnce(jsonResponse({ status: "approved_for_session" }))
      .mockResolvedValueOnce(jsonResponse({ status: "rejected" }))
      .mockResolvedValueOnce(jsonResponse({ status: "rejected" }))
      .mockResolvedValueOnce(jsonResponse({ host: "192.168.30.244", user: "root", port: 2222 }))
      .mockResolvedValueOnce(jsonResponse({ status: "reachable", command: "true" }))
      .mockResolvedValueOnce(jsonResponse({ session_id: "session-1", assistant_text: "Linux", pending_approvals: [] }));

    const api = { fetch: fetchMock as unknown as typeof fetch };

    await listSessions(api);
    await createSession(api);
    await listMessages("session-1", api);
    await runDiagnostic({ sessionId: "session-1", diagnostic: "system_info" }, api);
    await prepareCommand({ sessionId: "session-1", command: "uptime", cwd: "/tmp" }, api);
    await approveCommand("approval-1", api);
    await approveCommandForSession("approval-2", api);
    await rejectStopCommand("approval-3", api);
    await rejectCommand("approval-4", api);
    await updateSshTarget({ host: "192.168.30.244", user: "root", port: 2222 }, api);
    await checkSshTarget(api);
    await sendAgentTurn("session-1", "请问当前运行在什么系统？", api);

    expect(fetchMock.mock.calls.map(([url]) => url)).toEqual([
      "/api/sessions",
      "/api/sessions",
      "/api/sessions/session-1/messages",
      "/api/diagnostics/run",
      "/api/commands/prepare",
      "/api/approvals/approval-1/respond",
      "/api/approvals/approval-2/respond",
      "/api/approvals/approval-3/respond",
      "/api/approvals/approval-4/respond",
      "/api/ssh/target",
      "/api/ssh/check",
      "/api/sessions/session-1/agent-turn"
    ]);

    expect(JSON.parse(findRequest(fetchMock, "/api/diagnostics/run").body as string)).toEqual({
      session_id: "session-1",
      diagnostic: "system_info"
    });
    expect(JSON.parse(findRequest(fetchMock, "/api/commands/prepare").body as string)).toEqual({
      session_id: "session-1",
      command: "uptime",
      cwd: "/tmp"
    });
    expect(JSON.parse(findRequest(fetchMock, "/api/approvals/approval-1/respond").body as string)).toEqual({
      response: "approve_once"
    });
    expect(JSON.parse(findRequest(fetchMock, "/api/approvals/approval-2/respond").body as string)).toEqual({
      response: "approve_session"
    });
    expect(JSON.parse(findRequest(fetchMock, "/api/approvals/approval-3/respond").body as string)).toEqual({
      response: "reject_stop"
    });
    expect(JSON.parse(findRequest(fetchMock, "/api/approvals/approval-4/respond").body as string)).toEqual({
      response: "reject_stop"
    });
    expect(findRequest(fetchMock, "/api/ssh/target").method).toBe("PUT");
    expect(JSON.parse(findRequest(fetchMock, "/api/ssh/target").body as string)).toEqual({
      host: "192.168.30.244",
      user: "root",
      port: 2222
    });
    expect(findRequest(fetchMock, "/api/ssh/check").method).toBe("POST");
    expect(findRequest(fetchMock, "/api/sessions/session-1/agent-turn").method).toBe("POST");
    expect(JSON.parse(findRequest(fetchMock, "/api/sessions/session-1/agent-turn").body as string)).toEqual({
      message: "请问当前运行在什么系统？"
    });
  });

  it("updates a session title with PATCH", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(
        jsonResponse({ id: "session-1", title: "Renamed", created_at_ms: 1, updated_at_ms: 2 })
      );
    const api = { fetch: fetchMock as unknown as typeof fetch };

    await expect(updateSessionTitle("session-1", "Renamed", api)).resolves.toEqual({
      id: "session-1",
      title: "Renamed",
      created_at_ms: 1,
      updated_at_ms: 2
    });

    expect(fetchMock).toHaveBeenCalledWith("/api/sessions/session-1", {
      method: "PATCH",
      body: JSON.stringify({ title: "Renamed" }),
      headers: {
        "content-type": "application/json"
      }
    });
  });

  it("deletes a session with DELETE", async () => {
    const fetchMock = vi.fn().mockResolvedValueOnce(new Response(null, { status: 204 }));
    const api = { fetch: fetchMock as unknown as typeof fetch };

    await expect(deleteSession("session-1", api)).resolves.toBeUndefined();

    expect(fetchMock).toHaveBeenCalledWith("/api/sessions/session-1", {
      method: "DELETE",
      headers: {}
    });
  });

  it("redacts access tokens from delete error messages", async () => {
    const fetchMock = vi.fn().mockResolvedValueOnce(
      new Response(JSON.stringify({ message: "token phone-token rejected" }), {
        status: 500,
        statusText: "Internal Server Error",
        headers: { "content-type": "application/json" }
      })
    );
    const api = {
      fetch: fetchMock as unknown as typeof fetch,
      accessToken: "phone-token"
    };

    await expect(deleteSession("session-1", api)).rejects.toThrow("token [access token] rejected");
  });

  it("loads diagnostic presets from the server", async () => {
    const presets = [
      { key: "system_info", label: "Kernel", command: "uname -a", requires_approval: false },
      { key: "network", label: "Interfaces", command: "ip addr", requires_approval: false }
    ];
    const fetchMock = vi.fn().mockResolvedValueOnce(jsonResponse(presets));
    const api = { fetch: fetchMock as unknown as typeof fetch };

    await expect(getDiagnosticPresets(api)).resolves.toEqual(presets);

    expect(fetchMock).toHaveBeenCalledWith("/api/diagnostics/presets", {
      headers: {}
    });
  });

  it("adds an access token header when configured", async () => {
    const fetchMock = vi.fn().mockResolvedValueOnce(jsonResponse({ status: "ok" }));
    const api = {
      fetch: fetchMock as unknown as typeof fetch,
      accessToken: "phone-token"
    };

    await createSession(api);

    expect(fetchMock).toHaveBeenCalledWith("/api/sessions", {
      method: "POST",
      headers: {
        "X-Mobile-Web-Token": "phone-token"
      }
    });
  });

  it("adds an access token query parameter to the event stream URL when configured", () => {
    expect(buildEventUrl("phone token")).toBe("/event?access_token=phone+token");
    expect(buildEventUrl("")).toBe("/event");
    expect(buildEventUrl("   ")).toBe("/event");
  });

  it("returns agent turn messages with tool part data without dropping fields", async () => {
    const response = {
      session_id: "session-1",
      turn_id: "turn-1",
      status: "completed",
      assistant_text: "done",
      executed_tools: [],
      pending_approvals: [],
      messages: [{
        id: "message-1",
        session_id: "session-1",
        role: "assistant",
        created_at_ms: 123,
        parts: [{
          id: "part-tool-1",
          kind: "tool",
          data: {
            turn_id: "turn-1",
            agent_turn_id: "agent-turn-1",
            tool_call_id: "tool-call-1",
            approval_id: "approval-1",
            tool: "shell",
            title: "Run uname",
            status: "completed",
            requires_approval: true,
            command: "uname -a",
            input: { command: "uname -a" },
            output: "Linux test-host",
            stdout: "Linux test-host",
            stderr: "",
            exit_code: 0,
            duration_ms: 42,
            timed_out: false
          }
        }]
      }]
    };
    const fetchMock = vi.fn().mockResolvedValueOnce(jsonResponse(response));
    const api = { fetch: fetchMock as unknown as typeof fetch };

    const result = await sendAgentTurn("session-1", "run uname", api);

    expect(result.messages).toEqual(response.messages);
    expect(result.messages?.[0].parts[0].data).toEqual(response.messages[0].parts[0].data);
  });

  it("sends retry and continue agent turn options in the request body", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(jsonResponse({ session_id: "session-1", turn_id: "turn-2", status: "queued", assistant_text: "", executed_tools: [], pending_approvals: [] }))
      .mockResolvedValueOnce(jsonResponse({ session_id: "session-1", turn_id: "turn-3", status: "queued", assistant_text: "", executed_tools: [], pending_approvals: [] }));
    const api = { fetch: fetchMock as unknown as typeof fetch };

    await sendAgentTurn("session-1", "继续", { mode: "continue" }, api);
    await sendAgentTurn("session-1", "重试上一轮", { mode: "retry", retryTurnId: "turn-1" }, api);

    expect(JSON.parse(fetchMock.mock.calls[0][1].body)).toEqual({
      message: "继续",
      mode: "continue"
    });
    expect(JSON.parse(fetchMock.mock.calls[1][1].body)).toEqual({
      message: "重试上一轮",
      mode: "retry",
      retry_turn_id: "turn-1"
    });
  });

  it("posts stop requests to the current agent turn endpoint", async () => {
    const fetchMock = vi.fn().mockResolvedValueOnce(jsonResponse({ status: "stopping" }));
    const api = { fetch: fetchMock as unknown as typeof fetch };

    await stopAgentTurn("session-1", "turn-1", api);

    expect(fetchMock).toHaveBeenCalledWith(
      "/api/sessions/session-1/agent-turns/turn-1/stop",
      expect.objectContaining({ method: "POST" })
    );
  });
});
