import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useProductSessions } from "./useProductSessions";
import type { ApiContext } from "./api";
import type { Message, SessionSummary } from "./types";

function session(id: string, title: string, updatedAt: number): SessionSummary {
  return {
    id,
    title,
    created_at_ms: updatedAt,
    updated_at_ms: updatedAt
  };
}

function message(id: string, sessionId: string, text: string): Message {
  return {
    id,
    session_id: sessionId,
    role: "assistant",
    created_at_ms: 1,
    parts: [{ id: `${id}-part`, kind: "text", text }]
  };
}

function jsonResponse(body: unknown, init: ResponseInit = {}): Response {
  return new Response(JSON.stringify(body), {
    headers: { "content-type": "application/json" },
    ...init
  });
}

type ApiHarnessOptions = {
  sessions?: SessionSummary[];
  messages?: Record<string, Message[]>;
  createdSession?: SessionSummary;
};

function createApiHarness(options: ApiHarnessOptions = {}): {
  api: ApiContext;
  fetchMock: ReturnType<typeof vi.fn>;
  sessions: SessionSummary[];
} {
  const sessions = [...(options.sessions ?? [])];
  const messages = { ...(options.messages ?? {}) };
  const createdSession = options.createdSession ?? session("created", "New chat", 30);
  let createCount = 0;

  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = String(input);
    const method = init?.method ?? "GET";

    if (url === "/api/sessions" && method === "GET") {
      return jsonResponse(sessions);
    }

    if (url === "/api/sessions" && method === "POST") {
      createCount += 1;
      const created = createCount === 1 ? createdSession : session(`created-${createCount}`, "New chat", 30 + createCount);
      sessions.unshift(created);
      messages[created.id] ??= [];
      return jsonResponse(created);
    }

    const messagesMatch = url.match(/^\/api\/sessions\/([^/]+)\/messages$/);
    if (messagesMatch && method === "GET") {
      const sessionId = decodeURIComponent(messagesMatch[1]);
      return jsonResponse(messages[sessionId] ?? []);
    }

    const sessionMatch = url.match(/^\/api\/sessions\/([^/]+)$/);
    if (sessionMatch && method === "DELETE") {
      const sessionId = decodeURIComponent(sessionMatch[1]);
      const index = sessions.findIndex((candidate) => candidate.id === sessionId);
      if (index >= 0) {
        sessions.splice(index, 1);
      }
      delete messages[sessionId];
      return new Response(null, { status: 204 });
    }

    if (sessionMatch && method === "PATCH") {
      const sessionId = decodeURIComponent(sessionMatch[1]);
      const body = JSON.parse(String(init?.body ?? "{}")) as { title?: string };
      const index = sessions.findIndex((candidate) => candidate.id === sessionId);
      const updated = {
        ...(sessions[index] ?? session(sessionId, body.title ?? "Untitled", 1)),
        title: body.title ?? ""
      };
      if (index >= 0) {
        sessions[index] = updated;
      }
      return jsonResponse(updated);
    }

    return jsonResponse({ message: `Unhandled ${method} ${url}` }, { status: 500, statusText: "Unhandled" });
  });

  return {
    api: { fetch: fetchMock as unknown as typeof fetch },
    fetchMock,
    sessions
  };
}

describe("useProductSessions", () => {
  beforeEach(() => {
    window.history.replaceState({}, "", "/web");
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("does not create a conversation when no sessions exist", async () => {
    const { api, fetchMock } = createApiHarness();

    const { result } = renderHook(() => useProductSessions({ api }));

    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.sessions).toEqual([]);
    expect(result.current.activeSessionId).toBeNull();
    expect(result.current.activeMessages).toEqual([]);
    expect(fetchMock.mock.calls).toHaveLength(1);
    expect(fetchMock).toHaveBeenCalledWith("/api/sessions", { headers: {} });
  });

  it("selects the URL session when it is present", async () => {
    const newer = session("newer", "Newer", 20);
    const requested = session("requested", "Requested", 10);
    const { api } = createApiHarness({
      sessions: [newer, requested],
      messages: {
        requested: [message("message-1", "requested", "loaded requested")]
      }
    });
    window.history.replaceState({}, "", "/web?session=requested");

    const { result } = renderHook(() => useProductSessions({ api }));

    await waitFor(() => expect(result.current.activeMessages).toHaveLength(1));

    expect(result.current.activeSessionId).toBe("requested");
    expect(result.current.activeMessages[0].parts[0].text).toBe("loaded requested");
    expect(window.location.search).toBe("?session=requested");
  });

  it("loads messages and updates the URL when selecting a session", async () => {
    const first = session("first", "First", 10);
    const second = session("second", "Second", 20);
    const { api } = createApiHarness({
      sessions: [first, second],
      messages: {
        first: [message("message-1", "first", "first message")],
        second: [message("message-2", "second", "second message")]
      }
    });

    const { result } = renderHook(() => useProductSessions({ api }));
    await waitFor(() => expect(result.current.activeSessionId).toBe("second"));

    await act(async () => {
      await result.current.selectSession("first");
    });

    expect(result.current.activeSessionId).toBe("first");
    expect(result.current.activeMessages[0].parts[0].text).toBe("first message");
    expect(window.location.search).toBe("?session=first");
  });

  it("selects a newly created conversation", async () => {
    const existing = session("existing", "Existing", 10);
    const created = session("created", "Created", 20);
    const { api } = createApiHarness({
      sessions: [existing],
      createdSession: created,
      messages: {
        existing: [message("message-1", "existing", "existing message")],
        created: [message("message-2", "created", "created message")]
      }
    });

    const { result } = renderHook(() => useProductSessions({ api }));
    await waitFor(() => expect(result.current.activeSessionId).toBe("existing"));

    await act(async () => {
      await result.current.createConversation();
    });

    expect(result.current.activeSessionId).toBe("created");
    expect(result.current.sessions.map((candidate) => candidate.id)).toContain("created");
    expect(result.current.activeMessages[0].parts[0].text).toBe("created message");
    expect(window.location.search).toBe("?session=created");
  });

  it("selects the next available session after deleting the active session", async () => {
    const active = session("active", "Active", 30);
    const next = session("next", "Next", 20);
    const { api } = createApiHarness({
      sessions: [active, next],
      messages: {
        active: [message("message-1", "active", "active message")],
        next: [message("message-2", "next", "next message")]
      }
    });
    window.history.replaceState({}, "", "/web?session=active");

    const { result } = renderHook(() => useProductSessions({ api }));
    await waitFor(() => expect(result.current.activeSessionId).toBe("active"));

    await act(async () => {
      await result.current.deleteConversation("active");
    });

    expect(result.current.activeSessionId).toBe("next");
    expect(result.current.sessions.map((candidate) => candidate.id)).toEqual(["next"]);
    expect(result.current.activeMessages[0].parts[0].text).toBe("next message");
    expect(window.location.search).toBe("?session=next");
  });

  it("updates a conversation title in the local session list", async () => {
    const existing = session("existing", "Existing", 10);
    const { api } = createApiHarness({
      sessions: [existing],
      messages: {
        existing: []
      }
    });

    const { result } = renderHook(() => useProductSessions({ api }));
    await waitFor(() => expect(result.current.activeSessionId).toBe("existing"));

    await act(async () => {
      await result.current.updateConversationTitle("existing", "Renamed");
    });

    expect(result.current.sessions).toEqual([{ ...existing, title: "Renamed" }]);
  });
});
