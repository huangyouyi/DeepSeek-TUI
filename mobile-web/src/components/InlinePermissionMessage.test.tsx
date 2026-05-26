import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { InlinePermissionMessage } from "./InlinePermissionMessage";
import type { PermissionLike } from "../opencodeAdapter";

afterEach(() => {
  cleanup();
});

const bashPermission: PermissionLike = {
  id: "approval-1",
  type: "bash",
  pattern: "pwd\nls -la",
  sessionID: "session-1",
  title: "Bash 命令执行请求",
  metadata: {
    command: "pwd\n\nls -la",
    cwd: "/tmp/project",
    risk_reason: "Lists working directory contents"
  },
  time: {
    created: Date.UTC(2026, 4, 26, 12, 30)
  }
};

const formattedCreatedAt = new Intl.DateTimeFormat("zh-CN", {
  year: "numeric",
  month: "2-digit",
  day: "2-digit",
  hour: "2-digit",
  minute: "2-digit"
}).format(new Date(bashPermission.time.created));

describe("InlinePermissionMessage", () => {
  it("renders bash approval with three actions and prefixed commands", () => {
    const { container } = render(<InlinePermissionMessage permission={bashPermission} onRespond={vi.fn()} />);

    expect(screen.getByText("需要授权")).toBeTruthy();
    expect(screen.getByText("bash")).toBeTruthy();
    expect(screen.getByText("会话 session-1")).toBeTruthy();
    expect(screen.getByText("Bash 命令执行请求")).toBeTruthy();
    expect(screen.getByText(formattedCreatedAt)).toBeTruthy();
    expect(screen.getByText("Lists working directory contents")).toBeTruthy();
    expect(container.querySelector("pre")?.textContent).toBe("$ pwd\n\n$ ls -la");
    expect(screen.getByRole("button", { name: "仅这次执行" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "本会话都允许" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "拒绝并停止" })).toBeTruthy();
  });

  it("maps 仅这次执行 to onRespond(\"once\")", async () => {
    const onRespond = vi.fn().mockResolvedValue(undefined);
    render(<InlinePermissionMessage permission={bashPermission} onRespond={onRespond} />);

    fireEvent.click(screen.getByRole("button", { name: "仅这次执行" }));

    await waitFor(() => expect(onRespond).toHaveBeenCalledWith("once"));
  });

  it("maps 本会话都允许 to onRespond(\"always\")", async () => {
    const onRespond = vi.fn().mockResolvedValue(undefined);
    render(<InlinePermissionMessage permission={bashPermission} onRespond={onRespond} />);

    fireEvent.click(screen.getByRole("button", { name: "本会话都允许" }));

    await waitFor(() => expect(onRespond).toHaveBeenCalledWith("always"));
  });

  it("maps 拒绝并停止 to onRespond(\"reject\")", async () => {
    const onRespond = vi.fn().mockResolvedValue(undefined);
    render(<InlinePermissionMessage permission={bashPermission} onRespond={onRespond} />);

    fireEvent.click(screen.getByRole("button", { name: "拒绝并停止" }));

    await waitFor(() => expect(onRespond).toHaveBeenCalledWith("reject"));
  });

  it("shows an inline error if onRespond rejects", async () => {
    const onRespond = vi.fn().mockRejectedValue(new Error("network unavailable"));
    render(<InlinePermissionMessage permission={bashPermission} onRespond={onRespond} />);

    fireEvent.click(screen.getByRole("button", { name: "仅这次执行" }));

    expect((await screen.findByRole("alert")).textContent).toContain("network unavailable");
  });

  it("disables buttons while a response is pending", async () => {
    let resolveResponse: () => void = () => {};
    const onRespond = vi.fn(
      () =>
        new Promise<void>((resolve) => {
          resolveResponse = resolve;
        })
    );
    render(<InlinePermissionMessage permission={bashPermission} onRespond={onRespond} />);

    fireEvent.click(screen.getByRole("button", { name: "拒绝并停止" }));

    expect(screen.getByRole("status").textContent).toBe("正在处理授权响应...");
    expect((screen.getByRole("button", { name: "仅这次执行" }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole("button", { name: "本会话都允许" }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole("button", { name: "拒绝并停止" }) as HTMLButtonElement).disabled).toBe(true);

    resolveResponse();
    await waitFor(() => {
      expect((screen.getByRole("button", { name: "仅这次执行" }) as HTMLButtonElement).disabled).toBe(false);
    });
    expect(screen.queryByRole("status")).toBeNull();
  });
});
