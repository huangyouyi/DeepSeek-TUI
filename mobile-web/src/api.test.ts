import { describe, expect, it, vi } from "vitest";
import {
  approveCommand,
  buildEventUrl,
  checkSshTarget,
  createSession,
  getDiagnosticPresets,
  listMessages,
  prepareCommand,
  rejectCommand,
  runDiagnostic,
  updateSshTarget
} from "./api";

function jsonResponse(body: unknown): Response {
  return new Response(JSON.stringify(body), {
    headers: { "content-type": "application/json" }
  });
}

describe("api client", () => {
  it("uses shared contract routes and payload field names", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(jsonResponse({ id: "session-1", title: "Mobile SSH" }))
      .mockResolvedValueOnce(jsonResponse([]))
      .mockResolvedValueOnce(jsonResponse({ status: "queued" }))
      .mockResolvedValueOnce(jsonResponse({ id: "approval-1" }))
      .mockResolvedValueOnce(jsonResponse({ status: "approved" }))
      .mockResolvedValueOnce(jsonResponse({ status: "rejected" }))
      .mockResolvedValueOnce(jsonResponse({ host: "192.168.30.244", user: "root", port: 2222 }))
      .mockResolvedValueOnce(jsonResponse({ status: "reachable", command: "true" }));

    const api = { fetch: fetchMock as unknown as typeof fetch };

    await createSession(api);
    await listMessages("session-1", api);
    await runDiagnostic({ sessionId: "session-1", diagnostic: "system_info" }, api);
    await prepareCommand({ sessionId: "session-1", command: "uptime", cwd: "/tmp" }, api);
    await approveCommand("approval-1", api);
    await rejectCommand("approval-2", api);
    await updateSshTarget({ host: "192.168.30.244", user: "root", port: 2222 }, api);
    await checkSshTarget(api);

    expect(fetchMock.mock.calls.map(([url]) => url)).toEqual([
      "/api/sessions",
      "/api/sessions/session-1/messages",
      "/api/diagnostics/run",
      "/api/commands/prepare",
      "/api/approvals/approval-1/respond",
      "/api/approvals/approval-2/respond",
      "/api/ssh/target",
      "/api/ssh/check"
    ]);

    expect(JSON.parse(fetchMock.mock.calls[2][1].body)).toEqual({
      session_id: "session-1",
      diagnostic: "system_info"
    });
    expect(JSON.parse(fetchMock.mock.calls[3][1].body)).toEqual({
      session_id: "session-1",
      command: "uptime",
      cwd: "/tmp"
    });
    expect(JSON.parse(fetchMock.mock.calls[4][1].body)).toEqual({
      response: "approve_once"
    });
    expect(JSON.parse(fetchMock.mock.calls[5][1].body)).toEqual({
      response: "reject"
    });
    expect(fetchMock.mock.calls[6][1].method).toBe("PUT");
    expect(JSON.parse(fetchMock.mock.calls[6][1].body)).toEqual({
      host: "192.168.30.244",
      user: "root",
      port: 2222
    });
    expect(fetchMock.mock.calls[7][1].method).toBe("POST");
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
});
