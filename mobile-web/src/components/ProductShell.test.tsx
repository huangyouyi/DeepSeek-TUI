import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ProductShell } from "./ProductShell";

const sessions = [
  { id: "session-1", title: "Router health", created_at_ms: 1710000000000, updated_at_ms: 1710000060000 },
  { id: "session-2", title: "Package update", created_at_ms: 1710000100000, updated_at_ms: 1710000160000 }
];

const baseProps = {
  sessions,
  activeSessionId: "session-1",
  connectionStatusText: "connected",
  targetLabel: "root@192.168.30.244:22",
  messages: [
    { id: "message-1", role: "user" as const, text: "Check the router", createdAtMs: 1710000200000 },
    { id: "message-2", role: "assistant" as const, text: "I can inspect the target.", createdAtMs: 1710000210000 }
  ],
  pendingApprovals: [],
  toolActivities: [],
  finalAnswer: null,
  composer: {
    value: "",
    busy: false,
    onChange: vi.fn()
  },
  onSend: vi.fn(),
  onNewConversation: vi.fn(),
  onSelectConversation: vi.fn(),
  onApproveApproval: vi.fn(),
  onRejectApproval: vi.fn(),
  onContinue: vi.fn(),
  onRetry: vi.fn(),
  onStop: vi.fn(),
  onOpenSettings: vi.fn(),
  onCopyFinalAnswer: vi.fn(),
  onCopyFullReport: vi.fn()
};

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("ProductShell", () => {
  it("renders the product header, conversation sidebar, chat surface, and composer", () => {
    render(<ProductShell {...baseProps} composer={{ ...baseProps.composer, value: "Run diagnostics" }} />);

    expect(screen.getByRole("banner").textContent).toContain("DeepSeek 远程 Linux");
    expect(screen.getByText("服务器执行工具，浏览器只负责交互")).toBeTruthy();
    expect(screen.getByRole("navigation", { name: "会话列表" })).toBeTruthy();
    expect(screen.getByRole("main", { name: "远程 Linux 对话工作区" })).toBeTruthy();
    expect(screen.getByRole("form", { name: "消息输入区" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "新建会话" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "目标设备设置" }).textContent).toContain("root@192.168.30.244:22");
    expect(screen.getByDisplayValue("Run diagnostics")).toBeTruthy();
  });

  it("toggles the conversation drawer from the mobile header button", () => {
    render(<ProductShell {...baseProps} />);

    const sidebar = screen.getByLabelText("Conversation drawer");
    expect(sidebar.className).toContain("product-shell__sidebar--closed");

    fireEvent.click(screen.getByRole("button", { name: "打开会话侧栏" }));
    expect(sidebar.className).not.toContain("product-shell__sidebar--closed");

    fireEvent.click(screen.getByRole("button", { name: "关闭会话侧栏" }));
    expect(sidebar.className).toContain("product-shell__sidebar--closed");
  });

  it("opens settings and fills a suggested prompt", () => {
    render(<ProductShell {...baseProps} messages={[]} />);

    fireEvent.click(screen.getByRole("button", { name: "目标设备设置" }));
    fireEvent.click(screen.getByRole("button", { name: "检查远程 Linux 设备状态" }));

    expect(baseProps.onOpenSettings).toHaveBeenCalledTimes(1);
    expect(baseProps.composer.onChange).toHaveBeenCalledWith("检查远程 Linux 设备状态");
  });

  it("fills the composer from every common diagnostics entry without sending", () => {
    render(<ProductShell {...baseProps} />);

    const diagnostics = [
      ["当前系统", "系统"],
      ["网络", "网络"],
      ["DNS", "DNS"],
      ["磁盘", "磁盘"],
      ["内存/CPU", "内存"],
      ["服务", "服务"],
      ["Docker", "Docker"],
      ["OpenWrt/路由器", "OpenWrt"],
      ["日志摘要", "日志"]
    ] as const;

    diagnostics.forEach(([label, expectedText]) => {
      fireEvent.click(screen.getByRole("button", { name: label }));
      expect(baseProps.composer.onChange).toHaveBeenLastCalledWith(expect.stringContaining(expectedText));
    });

    expect(baseProps.composer.onChange).toHaveBeenCalledTimes(diagnostics.length);
    expect(baseProps.onSend).not.toHaveBeenCalled();
  });

  it("calls the final answer copy callback", () => {
    render(<ProductShell {...baseProps} finalAnswer="The router is reachable." />);

    fireEvent.click(screen.getByRole("button", { name: "复制最终答案" }));

    expect(baseProps.onCopyFinalAnswer).toHaveBeenCalledTimes(1);
  });

  it("calls the full report copy callback", () => {
    render(<ProductShell {...baseProps} finalAnswer="The router is reachable." />);

    fireEvent.click(screen.getByRole("button", { name: "复制完整报告" }));

    expect(baseProps.onCopyFullReport).toHaveBeenCalledTimes(1);
  });

  it("renders continue, retry, and stop controls with callbacks", () => {
    render(
      <ProductShell
        {...baseProps}
        canStop
        controlsBusy={false}
      />
    );

    fireEvent.click(screen.getByRole("button", { name: "继续" }));
    fireEvent.click(screen.getByRole("button", { name: "重试" }));
    fireEvent.click(screen.getByRole("button", { name: "停止" }));

    expect(baseProps.onContinue).toHaveBeenCalledTimes(1);
    expect(baseProps.onRetry).toHaveBeenCalledTimes(1);
    expect(baseProps.onStop).toHaveBeenCalledTimes(1);
  });

  it("disables stop while idle and disables all control buttons while busy", () => {
    const { rerender } = render(<ProductShell {...baseProps} canStop={false} controlsBusy={false} />);

    expect((screen.getByRole("button", { name: "继续" }) as HTMLButtonElement).disabled).toBe(false);
    expect((screen.getByRole("button", { name: "重试" }) as HTMLButtonElement).disabled).toBe(false);
    expect((screen.getByRole("button", { name: "停止" }) as HTMLButtonElement).disabled).toBe(true);

    rerender(<ProductShell {...baseProps} canStop controlsBusy />);

    expect((screen.getByRole("button", { name: "继续" }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole("button", { name: "重试" }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole("button", { name: "停止" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("shows approval context and calls three explicit response actions", () => {
    render(
      <ProductShell
        {...baseProps}
        pendingApprovals={[
          {
            id: "approval-1",
            session_id: "session-1",
            command: "opkg update",
            cwd: "/tmp",
            created_at_ms: 1710000300000,
            status: "pending",
            risk_level: "medium",
            risk_reason: "Updates remote package indexes and may contact configured mirrors.",
            target: "ssh://root@192.168.30.244:22",
            target_label: "OpenWrt router"
          }
        ]}
      />
    );

    expect(screen.getByText("opkg update")).toBeTruthy();
    expect(within(screen.getByRole("list", { name: "对话消息" })).queryByText("opkg update")).toBeNull();
    expect(within(screen.getByRole("complementary", { name: "工具活动" })).getByText("opkg update")).toBeTruthy();
    expect(screen.getByText("为什么需要执行")).toBeTruthy();
    expect(screen.getByText("Updates remote package indexes and may contact configured mirrors.")).toBeTruthy();
    expect(screen.getByText("风险等级")).toBeTruthy();
    expect(screen.getByText("medium")).toBeTruthy();
    expect(screen.getAllByText("目标设备").length).toBeGreaterThan(0);
    expect(screen.getByText(/OpenWrt router/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "仅这次执行" }));
    fireEvent.click(screen.getByRole("button", { name: "本会话都允许" }));
    fireEvent.click(screen.getByRole("button", { name: "拒绝并停止" }));

    expect(baseProps.onApproveApproval).toHaveBeenCalledWith("approval-1", "approve_once");
    expect(baseProps.onApproveApproval).toHaveBeenCalledWith("approval-1", "approve_session");
    expect(baseProps.onRejectApproval).toHaveBeenCalledWith("approval-1", "reject_stop");
  });

  it("keeps tool activity out of chat messages and shows it in the independent activity panel", () => {
    render(
      <ProductShell
        {...baseProps}
        toolActivities={[
          {
            id: "tool-1",
            status: "completed",
            command: "uname -a",
            title: "System info",
            stdout: "Linux test-host",
            stderr: "warning",
            exitCode: 0,
            durationMs: 42,
            createdAtMs: 1710000400000
          }
        ]}
      />
    );

    const chatThread = screen.getByRole("list", { name: "对话消息" });
    const toolPanel = screen.getByRole("complementary", { name: "工具活动" });

    expect(within(chatThread).queryByText("System info")).toBeNull();
    expect(within(chatThread).queryByText("uname -a")).toBeNull();
    expect(within(toolPanel).getByText("System info")).toBeTruthy();
    expect(within(toolPanel).getByText("uname -a")).toBeTruthy();
  });

  it("renders stdout and stderr streams collapsed by default", () => {
    render(
      <ProductShell
        {...baseProps}
        toolActivities={[
          {
            id: "tool-1",
            status: "completed",
            command: "uname -a",
            title: "System info",
            stdout: "Linux test-host",
            stderr: "warning",
            exitCode: 0,
            durationMs: 42,
            createdAtMs: 1710000400000
          }
        ]}
      />
    );

    const toolPanel = screen.getByRole("complementary", { name: "工具活动" });
    const stdoutDetails = within(toolPanel).getByText(/stdout/).closest("details");
    const stderrDetails = within(toolPanel).getByText(/stderr/).closest("details");

    expect(stdoutDetails?.hasAttribute("open")).toBe(false);
    expect(stderrDetails?.hasAttribute("open")).toBe(false);
    expect(within(toolPanel).getByText("Linux test-host")).toBeTruthy();
  });

  it("renders running, waiting, and error execution statuses", () => {
    const { rerender } = render(
      <ProductShell
        {...baseProps}
        executionStatus={{ label: "命令执行中", detail: "正在运行 uname -a", tone: "running" }}
      />
    );

    expect(screen.getByRole("status", { name: "执行状态" }).textContent).toContain("命令执行中");
    expect(screen.getByRole("status", { name: "执行状态" }).className).toContain("product-shell__execution-status--running");

    rerender(
      <ProductShell
        {...baseProps}
        executionStatus={{ label: "等待授权", detail: "需要你确认命令", tone: "waiting" }}
      />
    );
    expect(screen.getByRole("status", { name: "执行状态" }).textContent).toContain("等待授权");
    expect(screen.getByRole("status", { name: "执行状态" }).className).toContain("product-shell__execution-status--waiting");

    rerender(
      <ProductShell
        {...baseProps}
        executionStatus={{ label: "SSE 连接错误", detail: "请稍后重试", tone: "error" }}
      />
    );
    expect(screen.getByRole("status", { name: "执行状态" }).textContent).toContain("SSE 连接错误");
    expect(screen.getByRole("status", { name: "执行状态" }).className).toContain("product-shell__execution-status--error");
  });

  it("keeps the mobile-friendly product structure available", () => {
    render(<ProductShell {...baseProps} finalAnswer="The router is reachable." />);

    expect(document.querySelector(".product-shell__workbench")).toBeTruthy();
    expect(document.querySelector(".product-shell__chat-column")).toBeTruthy();
    expect(document.querySelector(".product-shell__tool-activity-panel")).toBeTruthy();
    expect(document.querySelector(".product-shell__final-answer--sticky")).toBeTruthy();
  });
});
