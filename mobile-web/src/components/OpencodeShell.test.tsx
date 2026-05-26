import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { OpencodeChatHeader } from "./OpencodeChatHeader";
import { OpencodeChatInput } from "./OpencodeChatInput";
import { OpencodeSidebar } from "./OpencodeSidebar";
import { OpencodeWelcomePage } from "./OpencodeWelcomePage";

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("Opencode standalone shell components", () => {
  it("sends ChatInput content on Enter and keeps Shift+Enter as a newline", () => {
    const onSendMessage = vi.fn();
    render(<OpencodeChatInput onSendMessage={onSendMessage} />);

    const input = screen.getByPlaceholderText("输入消息...");
    fireEvent.change(input, { target: { value: "first line" } });
    fireEvent.keyDown(input, { key: "Enter", code: "Enter", shiftKey: true });
    expect(onSendMessage).not.toHaveBeenCalled();

    fireEvent.keyDown(input, { key: "Enter", code: "Enter" });
    expect(onSendMessage).toHaveBeenCalledWith("first line");
  });

  it("calls onStop from ChatInput when sending and the action button is clicked", () => {
    const onStop = vi.fn();
    render(<OpencodeChatInput onSendMessage={vi.fn()} onStop={onStop} isSending />);

    fireEvent.click(screen.getByRole("button", { name: "停止生成" }));

    expect(onStop).toHaveBeenCalledTimes(1);
  });

  it("shows a visible replacement focus style on the ChatInput textarea", () => {
    render(<OpencodeChatInput onSendMessage={vi.fn()} />);

    const input = screen.getByPlaceholderText("输入消息...");
    fireEvent.focus(input);

    expect((input as HTMLTextAreaElement).style.outline).toBe("2px solid #5eead4");
  });

  it("renders Sidebar conversations and disables delete when only one remains", () => {
    const onDeleteConversation = vi.fn();
    render(
      <OpencodeSidebar
        conversations={[
          {
            id: "conversation-1",
            title: "Router health",
            createdAt: 1710000000000,
            updatedAt: 1710000060000
          }
        ]}
        currentConversationId="conversation-1"
        onSelectConversation={vi.fn()}
        onNewConversation={vi.fn()}
        onDeleteConversation={onDeleteConversation}
        isOpen
      />
    );

    expect(screen.getByText("Router health")).toBeTruthy();
    const deleteButton = screen.getByRole("button", { name: "删除 Router health" }) as HTMLButtonElement;
    expect(deleteButton.disabled).toBe(true);

    fireEvent.click(deleteButton);
    expect(onDeleteConversation).not.toHaveBeenCalled();
  });

  it("removes closed Sidebar from layout and the accessibility tree", () => {
    render(
      <OpencodeSidebar
        conversations={[
          {
            id: "conversation-1",
            title: "Router health",
            createdAt: 1710000000000,
            updatedAt: 1710000060000
          }
        ]}
        currentConversationId="conversation-1"
        onSelectConversation={vi.fn()}
        onNewConversation={vi.fn()}
        onDeleteConversation={vi.fn()}
        isOpen={false}
      />
    );

    const sidebar = screen.getByLabelText("会话列表", { selector: "aside" });
    expect(sidebar.getAttribute("aria-hidden")).toBe("true");
    expect((sidebar as HTMLElement).style.display).toBe("none");
    expect(screen.queryByRole("button", { name: "新建" })).toBeNull();
  });

  it("calls Sidebar new and select callbacks", () => {
    const onNewConversation = vi.fn();
    const onSelectConversation = vi.fn();
    render(
      <OpencodeSidebar
        conversations={[
          {
            id: "conversation-1",
            title: "Router health",
            createdAt: 1710000000000,
            updatedAt: 1710000060000
          },
          {
            id: "conversation-2",
            title: "Network check",
            createdAt: 1710000100000,
            updatedAt: 1710000160000
          }
        ]}
        currentConversationId="conversation-1"
        onSelectConversation={onSelectConversation}
        onNewConversation={onNewConversation}
        onDeleteConversation={vi.fn()}
        isOpen
      />
    );

    fireEvent.click(screen.getByRole("button", { name: "新建" }));
    fireEvent.click(screen.getByText("Network check").closest("button")!);

    expect(onNewConversation).toHaveBeenCalledTimes(1);
    expect(onSelectConversation).toHaveBeenCalledWith("conversation-2");
  });

  it("calls the welcome suggestion callback with the remote Linux health prompt", () => {
    const onSuggestedQuestion = vi.fn();
    render(<OpencodeWelcomePage onSuggestedQuestion={onSuggestedQuestion} />);

    fireEvent.click(screen.getByRole("button", { name: "检查远程 Linux 设备状态" }));

    expect(onSuggestedQuestion).toHaveBeenCalledWith("检查远程 Linux 设备状态");
  });

  it("calls settings and stop callbacks from Header controls", () => {
    const onOpenRemoteSettings = vi.fn();
    const onStop = vi.fn();
    render(
      <OpencodeChatHeader
        onStop={onStop}
        messageCount={3}
        onToggleSidebar={vi.fn()}
        isSidebarOpen={false}
        onOpenRemoteSettings={onOpenRemoteSettings}
        connectionStatusText="connected"
        isSending
      />
    );

    fireEvent.click(screen.getByRole("button", { name: "远程设置" }));
    fireEvent.click(screen.getByRole("button", { name: "停止生成" }));

    expect(onOpenRemoteSettings).toHaveBeenCalledTimes(1);
    expect(onStop).toHaveBeenCalledTimes(1);
  });
});
