import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import type { SessionMessageLike, ToolPartLike } from "../opencodeAdapter";
import { OpencodeChatMessage } from "./OpencodeChatMessage";

afterEach(() => {
  cleanup();
});

function message(role: SessionMessageLike["info"]["role"], parts: SessionMessageLike["parts"]): SessionMessageLike {
  return {
    info: {
      id: `${role}-message`,
      sessionID: "session-1",
      role,
      time: {
        created: 1
      }
    },
    parts
  };
}

function tool(status: ToolPartLike["state"]["status"], fields: Partial<ToolPartLike["state"]> = {}): ToolPartLike {
  return {
    id: `tool-${status}`,
    type: "tool",
    tool: "bash",
    title: "System check",
    state: {
      status,
      input: {
        command: "uname -a"
      },
      metadata: {},
      ...fields
    },
    time: {
      created: 1
    }
  };
}

describe("OpencodeChatMessage", () => {
  it("renders assistant markdown as sanitized HTML", () => {
    const { container } = render(
      <OpencodeChatMessage
        message={message("assistant", [{
          id: "text-1",
          type: "text",
          text: "**完成**",
          time: { created: 1 }
        }])}
      />
    );

    expect(container.querySelector("strong")?.textContent).toBe("完成");
  });

  it("keeps completed tool output hidden until the card is opened", () => {
    render(
      <OpencodeChatMessage
        message={message("assistant", [
          tool("completed", {
            output: "Linux test-host",
            metadata: {
              exit: 0,
              durationMs: 42
            }
          })
        ])}
      />
    );

    const details = screen.getByText("uname -a").closest("details");
    expect(details?.hasAttribute("open")).toBe(false);
    expect(screen.getByText("Linux test-host").closest(".opencode-chat-message__tool-body")?.hasAttribute("hidden")).toBe(true);

    fireEvent.click(screen.getByText("uname -a"));

    expect(details?.hasAttribute("open")).toBe(true);
    expect(screen.getByText("Linux test-host").closest(".opencode-chat-message__tool-body")?.hasAttribute("hidden")).toBe(false);
  });

  it("expands pending and running tool output by default", () => {
    render(
      <OpencodeChatMessage
        message={message("assistant", [
          tool("running", {
            output: "still working"
          }),
          tool("pending", {
            output: "waiting for approval"
          })
        ])}
      />
    );

    const details = screen.getAllByText("uname -a").map((node) => node.closest("details"));
    expect(details.every((detail) => detail?.hasAttribute("open"))).toBe(true);
    expect(screen.getByText("still working")).toBeTruthy();
    expect(screen.getByText("waiting for approval")).toBeTruthy();
  });

  it("shows error text with warning styling", () => {
    const { container } = render(
      <OpencodeChatMessage
        message={message("assistant", [
          tool("error", {
            error: "permission denied"
          })
        ])}
      />
    );

    expect(screen.getByText("permission denied")).toBeTruthy();
    expect(container.querySelector(".opencode-chat-message__tool--warning")).toBeTruthy();
  });

  it("marks user messages with user alignment", () => {
    const { container } = render(
      <OpencodeChatMessage
        message={message("user", [{
          id: "text-1",
          type: "text",
          text: "Run a check",
          time: { created: 1 }
        }])}
      />
    );

    const root = container.querySelector(".opencode-chat-message");
    expect(root?.className).toContain("opencode-chat-message--user");
    expect(screen.getByText("Run a check")).toBeTruthy();
  });

  it("renders a loading bubble for empty assistant messages", () => {
    const { container } = render(<OpencodeChatMessage message={message("assistant", [])} />);

    const bubble = container.querySelector(".opencode-chat-message__loading");
    expect(bubble?.textContent).toBe("...");
  });
});
