import { describe, expect, it } from "vitest";
import {
  buildProductTimeline,
  buildConversationTitleHint,
  buildTimelineItems,
  mapApprovalToPermission,
  mapChatItemToSessionMessage,
  mapSessionToConversation,
  mapToolActivityToToolPart
} from "./opencodeAdapter";

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
    expect(buildConversationTitleHint("  ##   spaced    heading   \nnext")).toBe("spaced heading");
    expect(buildConversationTitleHint("a".repeat(32))).toBe("a".repeat(32));
    expect(buildConversationTitleHint("a".repeat(40))).toBe(`${"a".repeat(29)}...`);
    expect(buildConversationTitleHint(`### ${"b".repeat(40)}\n${"c".repeat(40)}`)).toBe(`${"b".repeat(29)}...`);
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
      title: "Bash 命令执行请求",
      time: { created: 10 }
    });
    expect(permission.metadata).toMatchObject({
      command: "ping -c 3 223.5.5.5",
      cwd: "/root",
      risk_reason: "Network diagnostic",
      target: "root@192.168.1.1:22",
      target_label: "router"
    });
  });

  it("maps tool activity to collapsed bash tool parts", () => {
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

    expect(part).toMatchObject({
      id: "tool-1",
      type: "tool",
      tool: "bash",
      title: "df -h",
      time: { created: 20 }
    });
    expect(part.state).toMatchObject({
      status: "completed",
      input: { command: "df -h" },
      output: "ok",
      error: "",
      metadata: { exit: 0, durationMs: 74 }
    });
  });

  it("normalizes approval and failure tool statuses", () => {
    expect(
      mapToolActivityToToolPart({
        id: "tool-pending",
        status: "pending_approval",
        command: "whoami",
        createdAtMs: 1
      }).state.status
    ).toBe("pending");

    expect(
      mapToolActivityToToolPart({
        id: "tool-failed",
        status: "failed",
        command: "false",
        stderr: "nope",
        createdAtMs: 2
      }).state.status
    ).toBe("error");

    expect(
      mapToolActivityToToolPart({
        id: "tool-unknown",
        status: "unexpected",
        command: "date",
        createdAtMs: 3
      }).state.status
    ).toBe("updated");
  });

  it("maps chat items to session messages with text parts", () => {
    expect(
      mapChatItemToSessionMessage(
        {
          id: "chat-1",
          role: "assistant",
          text: "Done",
          createdAtMs: 30,
          status: "completed",
          isFinal: true
        },
        "session-1"
      )
    ).toEqual({
      info: {
        id: "chat-1",
        sessionID: "session-1",
        role: "assistant",
        time: { created: 30 }
      },
      parts: [{ id: "chat-1:text", type: "text", text: "Done", time: { created: 30 } }],
      delta: undefined
    });
  });

  it("builds active-session timeline items in chronological order", () => {
    const timeline = buildTimelineItems({
      sessionId: "session-1",
      activeChatItems: [
        { id: "chat-2", role: "assistant", text: "second", createdAtMs: 30 },
        { id: "chat-1", role: "user", text: "first", createdAtMs: 10 }
      ],
      pendingApprovals: [
        {
          id: "approval-1",
          session_id: "session-1",
          command: "uptime",
          created_at_ms: 20,
          status: "pending"
        },
        {
          id: "approval-other",
          session_id: "session-2",
          command: "ignored",
          created_at_ms: 5,
          status: "pending"
        }
      ],
      activeToolActivities: [{ id: "tool-1", status: "completed", command: "uptime", createdAtMs: 25 }]
    });

    expect(timeline.map((item) => item.id)).toEqual(["message:chat-1", "permission:approval-1", "tool:tool-1", "message:chat-2"]);
  });

  it("documents active timeline inputs while filtering global approvals", () => {
    const timeline = buildTimelineItems({
      sessionId: "active-session",
      activeChatItems: [{ id: "active-chat", role: "user", text: "active", createdAtMs: 10 }],
      activeToolActivities: [{ id: "active-tool", status: "running", command: "uptime", createdAtMs: 30 }],
      pendingApprovals: [
        {
          id: "active-approval",
          session_id: "active-session",
          command: "uptime",
          created_at_ms: 20,
          status: "pending"
        },
        {
          id: "other-approval",
          session_id: "other-session",
          command: "ignored",
          created_at_ms: 25,
          status: "pending"
        }
      ]
    });

    expect(timeline.map((item) => item.id)).toEqual(["message:active-chat", "permission:active-approval", "tool:active-tool"]);
    expect(timeline.every((item) => item.sessionId === "active-session")).toBe(true);
  });

  it("builds a product timeline from active items and filtered global approvals", () => {
    const timeline = buildProductTimeline({
      sessionId: "active-session",
      activeChatItems: [
        { id: "chat-2", role: "assistant", text: "second", createdAtMs: 30 },
        { id: "chat-1", role: "user", text: "first", createdAtMs: 10 }
      ],
      activeToolActivities: [{ id: "tool-1", status: "running", command: "uptime", createdAtMs: 25 }],
      pendingApprovals: [
        {
          id: "approval-1",
          session_id: "active-session",
          command: "uptime",
          created_at_ms: 20,
          status: "pending"
        },
        {
          id: "approval-other",
          session_id: "other-session",
          command: "ignored",
          created_at_ms: 15,
          status: "pending"
        }
      ],
      isLoading: true
    });

    expect(timeline.map((item) => item.kind)).toEqual(["message", "permission", "tool", "message", "loading"]);
    expect(timeline.map((item) => item.id)).toEqual([
      "message:chat-1",
      "permission:approval-1",
      "tool:tool-1",
      "message:chat-2",
      "loading:active-session"
    ]);
    expect(timeline.every((item) => item.sessionId === "active-session")).toBe(true);
  });

  it("sorts equal timestamp product timeline items stably by id", () => {
    const timeline = buildProductTimeline({
      sessionId: "session-1",
      activeChatItems: [
        { id: "z-chat", role: "assistant", text: "chat", createdAtMs: 10 },
        { id: "a-chat", role: "user", text: "chat", createdAtMs: 10 }
      ],
      activeToolActivities: [{ id: "b-tool", status: "completed", command: "date", createdAtMs: 10 }],
      pendingApprovals: [
        {
          id: "c-approval",
          session_id: "session-1",
          command: "date",
          created_at_ms: 10,
          status: "pending"
        }
      ]
    });

    expect(timeline.map((item) => item.id)).toEqual([
      "message:a-chat",
      "message:z-chat",
      "permission:c-approval",
      "tool:b-tool"
    ]);
  });
});
