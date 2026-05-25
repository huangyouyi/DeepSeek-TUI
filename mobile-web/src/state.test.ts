import { describe, expect, it } from "vitest";
import { initialAppState, reduceEvent } from "./state";
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
});
