import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ChatView } from "./ChatView";
import { ToolActivity } from "./ToolActivity";

afterEach(() => {
  cleanup();
});

describe("ChatView", () => {
  it("renders the latest final answer prominently and calls back when copied", () => {
    const onCopyFinalAnswer = vi.fn();

    render(
      <ChatView
        messages={[
          { id: "m1", role: "user", text: "What changed?" },
          { id: "m2", role: "assistant", text: "I checked the device." }
        ]}
        finalAnswer="The mobile web panel now shows the useful answer first."
        composer={{
          value: "",
          onChange: vi.fn(),
          onSubmit: vi.fn(),
          disabled: false,
          busy: false,
          placeholder: "Ask about the device",
          submitLabel: "Send",
          busyLabel: "Sending"
        }}
        onCopyFinalAnswer={onCopyFinalAnswer}
      />
    );

    expect(screen.getByRole("region", { name: "Final answer" }).textContent).toContain(
      "The mobile web panel now shows the useful answer first."
    );
    fireEvent.click(screen.getByRole("button", { name: "Copy final answer" }));

    expect(onCopyFinalAnswer).toHaveBeenCalledTimes(1);
  });

  it("renders distinct message bubbles and an empty state", () => {
    const { rerender } = render(
      <ChatView
        messages={[
          { id: "m1", role: "user", text: "Run a check" },
          { id: "m2", role: "assistant", text: "The check is complete." }
        ]}
        composer={{
          value: "next prompt",
          onChange: vi.fn(),
          onSubmit: vi.fn(),
          disabled: false,
          busy: false
        }}
      />
    );

    expect(screen.getByText("Run a check").closest(".chat-view__bubble--user")).toBeTruthy();
    expect(screen.getByText("The check is complete.").closest(".chat-view__bubble--assistant")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Copy final answer" })).toBeNull();

    rerender(
      <ChatView
        messages={[]}
        composer={{
          value: "",
          onChange: vi.fn(),
          onSubmit: vi.fn(),
          disabled: false,
          busy: false
        }}
      />
    );

    expect(screen.getByText("No conversation yet")).toBeTruthy();
  });
});

describe("ToolActivity", () => {
  it("renders compact tool rows with collapsed stdout and distinct stderr details", () => {
    const { container } = render(
      <ToolActivity
        activities={[
          {
            id: "tool-1",
            status: "completed",
            title: "System probe",
            command: "uname -a",
            exitCode: 0,
            durationMs: 42,
            requiresApproval: true,
            stdout: "Linux test-host",
            stderr: "warning: simulated stderr\nsecond line"
          }
        ]}
      />
    );

    expect(screen.getByTitle("completed").textContent).toBe("已完成");
    expect(screen.getByText("System probe")).toBeTruthy();
    expect(screen.getByText("uname -a")).toBeTruthy();
    expect(screen.getByText("exit 0")).toBeTruthy();
    expect(screen.getByText("42 ms")).toBeTruthy();
    expect(screen.getByText("需要授权")).toBeTruthy();

    const stdoutDetails = screen.getByText("stdout · 1 line · 15 chars").closest("details");
    const stderrDetails = screen.getByText("stderr · 2 lines · 37 chars").closest("details");

    expect(stdoutDetails?.hasAttribute("open")).toBe(false);
    expect(stderrDetails?.hasAttribute("open")).toBe(false);
    const stderrStream = container.querySelector(".tool-activity__stream--stderr");
    expect(stderrStream?.textContent).toBe("warning: simulated stderr\nsecond line");
  });

  it("renders output details inside a bounded pre and an empty state", () => {
    const longOutput = Array.from({ length: 80 }, (_, index) => `line ${index + 1}`).join("\n");
    const { container, rerender } = render(
      <ToolActivity
        activities={[
          {
            id: "tool-2",
            status: "failed",
            command: "df -h",
            output: longOutput
          }
        ]}
      />
    );

    expect(screen.getByTitle("failed").textContent).toBe("失败");
    expect(screen.getByText("output · 80 lines · 630 chars").closest("details")?.hasAttribute("open")).toBe(false);
    const outputStream = container.querySelector("pre.tool-activity__stream");
    expect(outputStream?.textContent).toBe(longOutput);
    expect(outputStream?.className).toContain("tool-activity__stream");

    rerender(<ToolActivity activities={[]} />);

    expect(screen.getByText("工具执行细节会在这里出现，包括命令状态、授权结果和折叠的输出。")).toBeTruthy();
  });

  it("labels running and approval-waiting statuses for scanning", () => {
    render(
      <ToolActivity
        activities={[
          { id: "tool-3", status: "running", command: "uptime" },
          { id: "tool-4", status: "pending_approval", command: "apt update", requiresApproval: true },
          { id: "tool-5", status: "rejected", command: "rm -rf /tmp/demo" },
          { id: "tool-6", status: "stopped", command: "sleep 30" }
        ]}
      />
    );

    expect(screen.getByTitle("running").textContent).toBe("执行中");
    expect(screen.getByTitle("pending_approval").textContent).toBe("待授权");
    expect(screen.getByTitle("rejected").textContent).toBe("已拒绝");
    expect(screen.getByTitle("stopped").textContent).toBe("已停止");
  });
});
