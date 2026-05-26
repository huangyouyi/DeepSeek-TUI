import { describe, expect, it } from "vitest";
import {
  buildFinalAnswerReport,
  buildFeedbackReport,
  fallbackDiagnosticPresets,
  initialAppState,
  reduceEvent,
  resolveDiagnosticPresets,
  selectChatItems,
  selectExecutionStatus,
  selectFinalAnswer,
  selectToolActivities
} from "./state";
import type { PendingApproval, ServerEvent } from "./types";

describe("reduceEvent", () => {
  it("tracks connection state updates", () => {
    const next = reduceEvent(initialAppState, {
      type: "connection.updated",
      payload: { status: "connected" }
    });

    expect(next.connection.status).toBe("connected");
  });

  it("adds approval prompts with the three explicit response actions", () => {
    const approval: PendingApproval = {
      id: "approval-1",
      session_id: "session-1",
      command: "uptime",
      created_at_ms: 100,
      status: "pending"
    };

    const next = reduceEvent(initialAppState, {
      type: "approval.asked",
      payload: approval
    });

    expect(next.pendingApprovals).toHaveLength(1);
    expect(next.pendingApprovals[0].id).toBe("approval-1");
    expect(next.approvalActions).toEqual(["approve_once", "approve_session", "reject_stop"]);
  });

  it("removes pending approval when a reply arrives", () => {
    const asked: ServerEvent = {
      type: "approval.asked",
      payload: {
        id: "approval-1",
        session_id: "session-1",
        command: "uptime",
        created_at_ms: 100,
        status: "pending"
      }
    };

    const replied: ServerEvent = {
      type: "approval.replied",
      payload: {
        id: "approval-1",
        status: "rejected"
      }
    };

    const next = reduceEvent(reduceEvent(initialAppState, asked), replied);

    expect(next.pendingApprovals).toHaveLength(0);
  });

  it("keeps long stdout text in timeline entries", () => {
    const output = "x".repeat(500);
    const next = reduceEvent(initialAppState, {
      type: "tool.stdout",
      payload: { id: "tool-1", text: output }
    });

    expect(next.timeline[0].kind).toBe("stdout");
    expect(next.timeline[0].text).toBe(output);
  });

  it("deduplicates repeated timeline events from rest responses and sse", () => {
    const event: ServerEvent = {
      type: "message.updated",
      payload: { role: "user", text: "请问当前运行在什么系统？" }
    };

    const next = reduceEvent(reduceEvent(initialAppState, event), event);

    expect(next.timeline).toHaveLength(1);
    expect(next.timeline[0].title).toBe("You");
  });

  it("deduplicates repeated chat messages from rest responses and sse without matching ids", () => {
    const restEvent: ServerEvent = {
      type: "message.updated",
      payload: { role: "assistant", text: "Linux check complete." }
    };
    const sseEvent: ServerEvent = {
      type: "message.updated",
      payload: {
        id: "server-message-1",
        role: "assistant",
        parts: [{ id: "part-1", kind: "text", text: "Linux check complete." }]
      }
    };

    const next = reduceEvent(reduceEvent(initialAppState, restEvent), sseEvent);

    expect(selectChatItems(next)).toHaveLength(1);
    expect(selectChatItems(next)[0].text).toBe("Linux check complete.");
  });

  it("does not promote approval-waiting assistant text into the final answer", () => {
    const waiting = reduceEvent(initialAppState, {
      type: "message.updated",
      payload: {
        role: "assistant",
        status: "waiting_for_approval",
        text: "I need approval before running the requested command."
      }
    });

    expect(selectFinalAnswer(waiting)).toBeUndefined();

    const completed = reduceEvent(waiting, {
      type: "message.updated",
      payload: {
        role: "assistant",
        status: "completed",
        text: "本轮远程命令已全部执行完成。"
      }
    });

    expect(selectFinalAnswer(completed)?.text).toBe("本轮远程命令已全部执行完成。");
  });

  it("creates a tool timeline entry from message part updates", () => {
    const next = reduceEvent(initialAppState, {
      type: "message.part.updated",
      payload: {
        session_id: "session-1",
        message_id: "message-1",
        part: {
          id: "part-tool-1",
          kind: "tool",
          data: {
            command: "uname -a",
            status: "completed",
            output: "Linux test-host"
          }
        }
      }
    });

    expect(next.timeline).toHaveLength(1);
    expect(next.timeline[0]).toMatchObject({
      id: "part-tool-1",
      kind: "tool",
      title: "completed",
      text: "uname -a\ncompleted\nLinux test-host"
    });
  });

  it("updates repeated tool message part updates instead of appending duplicates", () => {
    const started: ServerEvent = {
      type: "message.part.updated",
      payload: {
        session_id: "session-1",
        message_id: "message-1",
        part: {
          id: "part-tool-1",
          kind: "tool",
          data: {
            command: "uname -a",
            status: "running"
          }
        }
      }
    };
    const completed: ServerEvent = {
      type: "message.part.updated",
      payload: {
        session_id: "session-1",
        message_id: "message-1",
        part: {
          id: "part-tool-1",
          kind: "tool",
          data: {
            command: "uname -a",
            status: "completed",
            output: "Linux test-host"
          }
        }
      }
    };

    const next = reduceEvent(reduceEvent(initialAppState, started), completed);

    expect(next.timeline).toHaveLength(1);
    expect(next.timeline[0]).toMatchObject({
      id: "part-tool-1",
      kind: "tool",
      title: "completed",
      text: "uname -a\ncompleted\nLinux test-host"
    });
  });

  it("retains stdout and stderr from tool data when part text is absent", () => {
    const next = reduceEvent(initialAppState, {
      type: "message.part.updated",
      payload: {
        session_id: "session-1",
        message_id: "message-1",
        part: {
          id: "part-tool-1",
          kind: "tool",
          data: {
            command: "sh -c 'echo out; echo err >&2'",
            status: "failed",
            stdout: "out",
            stderr: "err"
          }
        }
      }
    });

    expect(next.timeline[0].text).toBe("sh -c 'echo out; echo err >&2'\nfailed\nout\nerr");
  });

  it("removes a deleted session from sessions", () => {
    const state = {
      ...initialAppState,
      sessions: [
        { id: "session-1", title: "Keep", created_at_ms: 100, updated_at_ms: 100 },
        { id: "session-2", title: "Delete", created_at_ms: 200, updated_at_ms: 200 }
      ]
    };

    const next = reduceEvent(state, {
      type: "session.deleted",
      payload: { id: "session-2", deleted_at_ms: 300 }
    });

    expect(next.sessions.map((session) => session.id)).toEqual(["session-1"]);
    expect(next.timeline[0]).toMatchObject({
      kind: "session",
      title: "Session deleted",
      text: "session-2"
    });
  });

  it("clears active session when the deleted session is active", () => {
    const state = {
      ...initialAppState,
      activeSessionId: "session-2",
      sessions: [
        { id: "session-2", title: "Delete", created_at_ms: 200, updated_at_ms: 200 }
      ]
    };

    const next = reduceEvent(state, {
      type: "session.deleted",
      payload: { session_id: "session-2" }
    });

    expect(next.activeSessionId).toBeUndefined();
  });

  it("removes pending approvals and stored messages for a deleted session", () => {
    const state = {
      ...initialAppState,
      pendingApprovals: [
        { id: "approval-1", session_id: "session-1", command: "uptime", created_at_ms: 100, status: "pending" },
        { id: "approval-2", session_id: "session-2", command: "df -h", created_at_ms: 200, status: "pending" }
      ],
      messages: [
        { id: "message-1", session_id: "session-1", role: "user", created_at_ms: 100, parts: [] },
        { id: "message-2", session_id: "session-2", role: "assistant", created_at_ms: 200, parts: [] }
      ]
    };

    const next = reduceEvent(state, {
      type: "session.deleted",
      payload: { id: "session-2", removed_pending_approvals: 1 }
    });

    expect(next.pendingApprovals.map((approval) => approval.id)).toEqual(["approval-1"]);
    expect(next.messages.map((message) => message.id)).toEqual(["message-1"]);
  });

  it("does not crash when a session deletion payload is malformed", () => {
    const state = {
      ...initialAppState,
      sessions: [
        { id: "session-1", title: "Keep", created_at_ms: 100, updated_at_ms: 100 }
      ]
    };

    expect(() => reduceEvent(state, {
      type: "session.deleted",
      payload: null
    })).not.toThrow();
  });
});

describe("buildFeedbackReport", () => {
  it("builds a compact copyable report with connection and timeline", () => {
    const state = {
      ...initialAppState,
      connection: {
        ...initialAppState.connection,
        server: "ok" as const,
        sse: "connected" as const,
        target: {
          host: "192.168.30.244",
          user: "root",
          port: 22,
          key_present: false
        }
      },
      pendingApprovals: [{
        id: "approval-1",
        session_id: "session-1",
        command: "ping -c 4 8.8.8.8",
        created_at_ms: 100,
        status: "pending"
      }],
      timeline: [{
        id: "entry-1",
        kind: "assistant-message" as const,
        title: "Assistant",
        text: "本轮所有审批已处理完成。",
        createdAtMs: 1000
      }]
    };

    const report = buildFeedbackReport(state);

    expect(report).toContain("Server: ok");
    expect(report).toContain("SSE: connected");
    expect(report).toContain("Target: root@192.168.30.244:22");
    expect(report).toContain("Pending approvals: 1");
    expect(report).toContain("ping -c 4 8.8.8.8");
    expect(report).toContain("[Assistant]");
    expect(report).toContain("本轮所有审批已处理完成。");
  });
});

describe("product view selectors", () => {
  it("derives a clear execution status from connection, approvals, busy state, and running tools", () => {
    expect(selectExecutionStatus({ ...initialAppState, connection: { ...initialAppState.connection, sse: "error" } }, null)).toEqual({
      state: "error",
      label: "连接中断"
    });

    expect(selectExecutionStatus({
      ...initialAppState,
      pendingApprovals: [{
        id: "approval-1",
        session_id: "session-1",
        command: "uname -a",
        created_at_ms: 100,
        status: "pending"
      }]
    }, null)).toEqual({
      state: "waiting",
      label: "等待授权"
    });

    expect(selectExecutionStatus({
      ...initialAppState,
      toolActivities: [{
        id: "tool-1",
        status: "running",
        command: "uname -a",
        createdAtMs: 100
      }]
    }, null)).toEqual({
      state: "running",
      label: "执行中"
    });

    expect(selectExecutionStatus(initialAppState, "agent")).toEqual({
      state: "running",
      label: "执行中"
    });

    expect(selectExecutionStatus(initialAppState, null)).toEqual({
      state: "idle",
      label: "空闲"
    });
  });

  it("keeps chat messages as structured state and updates repeated message events", () => {
    const user: ServerEvent = {
      type: "message.updated",
      payload: {
        id: "message-user-1",
        role: "user",
        text: "What system is this?",
        created_at_ms: 1000
      }
    };
    const assistantDraft: ServerEvent = {
      type: "message.updated",
      payload: {
        id: "message-assistant-1",
        role: "assistant",
        text: "Checking",
        created_at_ms: 2000
      }
    };
    const assistantFinal: ServerEvent = {
      type: "message.updated",
      payload: {
        id: "message-assistant-1",
        role: "assistant",
        text: "It is Linux.",
        created_at_ms: 2000
      }
    };

    const state = [user, assistantDraft, assistantFinal].reduce(reduceEvent, initialAppState);

    expect(selectChatItems(state)).toEqual([
      {
        id: "message-user-1",
        role: "user",
        text: "What system is this?",
        createdAtMs: 1000
      },
      {
        id: "message-assistant-1",
        role: "assistant",
        text: "It is Linux.",
        createdAtMs: 2000,
        isFinal: true
      }
    ]);
    expect(selectFinalAnswer(state)).toMatchObject({
      id: "message-assistant-1",
      text: "It is Linux."
    });
  });

  it("turns typed tool parts into tool activities without appending stdout to assistant chat text", () => {
    const state = [
      {
        type: "message.updated",
        payload: {
          id: "message-assistant-1",
          role: "assistant",
          text: "The command finished.",
          created_at_ms: 2000
        }
      },
      {
        type: "message.part.updated",
        payload: {
          session_id: "session-1",
          message_id: "message-assistant-1",
          part: {
            id: "part-tool-1",
            kind: "tool",
            data: {
              command: "uname -a",
              title: "System info",
              status: "completed",
              stdout: "Linux test-host",
              stderr: "warning",
              exit_code: 0,
              duration_ms: 42,
              requires_approval: false
            }
          }
        }
      }
    ].reduce((next, event) => reduceEvent(next, event as ServerEvent), initialAppState);

    expect(selectToolActivities(state)).toEqual([
      {
        id: "part-tool-1",
        status: "completed",
        command: "uname -a",
        title: "System info",
        stdout: "Linux test-host",
        stderr: "warning",
        exitCode: 0,
        durationMs: 42,
        requiresApproval: false,
        createdAtMs: expect.any(Number)
      }
    ]);
    expect(selectFinalAnswer(state)?.text).toBe("The command finished.");
    expect(buildFinalAnswerReport(state)).toBe("The command finished.");
  });

  it("does not turn tool-only message parts into chat or final answer text", () => {
    const state = reduceEvent(initialAppState, {
      type: "message.updated",
      payload: {
        id: "message-assistant-tool-only",
        role: "assistant",
        created_at_ms: 2000,
        parts: [{
          id: "part-tool-1",
          kind: "tool",
          data: {
            command: "uname -a",
            status: "completed",
            stdout: "Linux test-host"
          }
        }]
      }
    });

    expect(selectChatItems(state)).toEqual([]);
    expect(selectFinalAnswer(state)).toBeUndefined();
  });

  it("populates tool activities from legacy tool stream events", () => {
    const events: ServerEvent[] = [
      {
        type: "tool.started",
        payload: { id: "tool-1", command: "df -h", status: "running", requires_approval: true }
      },
      {
        type: "tool.stdout",
        payload: { id: "tool-1", text: "Filesystem Size Used" }
      },
      {
        type: "tool.stderr",
        payload: { id: "tool-1", text: "df: permission warning" }
      },
      {
        type: "tool.completed",
        payload: { id: "tool-1", command: "df -h", status: "completed", exit_code: 0, duration_ms: 31 }
      }
    ];

    const state = events.reduce(reduceEvent, initialAppState);

    expect(selectToolActivities(state)).toEqual([
      {
        id: "tool-1",
        status: "completed",
        command: "df -h",
        stdout: "Filesystem Size Used",
        stderr: "df: permission warning",
        exitCode: 0,
        durationMs: 31,
        requiresApproval: true,
        createdAtMs: expect.any(Number)
      }
    ]);
  });
});

describe("diagnostic presets", () => {
  it("uses loaded presets when present and falls back to local presets when loading fails", () => {
    const loaded = [
      { key: "system_info" as const, label: "Kernel", command: "uname -a", requires_approval: false }
    ];

    expect(resolveDiagnosticPresets(loaded)).toEqual(loaded);
    expect(resolveDiagnosticPresets(undefined)).toEqual(fallbackDiagnosticPresets);
    expect(resolveDiagnosticPresets([])).toEqual(fallbackDiagnosticPresets);
  });
});
