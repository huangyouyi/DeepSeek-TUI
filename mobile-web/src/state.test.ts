import { describe, expect, it } from "vitest";
import {
  buildFeedbackReport,
  fallbackDiagnosticPresets,
  initialAppState,
  reduceEvent,
  resolveDiagnosticPresets
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

  it("adds approval prompts without adding always approve actions", () => {
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
    expect(next.approvalActions).toEqual(["approve_once", "reject"]);
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
