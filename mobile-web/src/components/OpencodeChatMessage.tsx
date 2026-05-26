import type { SessionMessageLike, ToolPartLike, ToolVisualStatus } from "../opencodeAdapter";
import { renderMarkdown } from "../markdown";
import "../styles/markdown.css";
import "../styles/message.css";

type OpencodeChatMessageProps = {
  message: SessionMessageLike;
};

const activeStatuses = new Set<ToolVisualStatus>(["pending", "running", "started", "queued", "in_progress"]);
const warningStatuses = new Set<ToolVisualStatus>(["error", "rejected"]);

const statusLabels: Record<ToolVisualStatus, { icon: string; text: string }> = {
  pending: { icon: "o", text: "pending" },
  running: { icon: "*", text: "running" },
  started: { icon: "*", text: "started" },
  updated: { icon: "*", text: "updated" },
  completed: { icon: "v", text: "completed" },
  error: { icon: "!", text: "error" },
  rejected: { icon: "!", text: "rejected" },
  queued: { icon: "o", text: "queued" },
  in_progress: { icon: "*", text: "in progress" }
};

const shellStyle = `
.opencode-chat-message {
  display: flex;
  margin: 0.65rem 0;
}
.opencode-chat-message--user {
  justify-content: flex-end;
}
.opencode-chat-message--assistant {
  justify-content: flex-start;
}
.opencode-chat-message__stack {
  display: grid;
  gap: 0.45rem;
  max-width: min(88%, 48rem);
}
.opencode-chat-message--user .opencode-chat-message__stack {
  justify-items: end;
}
.opencode-chat-message__bubble {
  width: fit-content;
  max-width: 100%;
  border: 1px solid rgba(148, 163, 184, 0.24);
  border-radius: 0.8rem;
  padding: 0.65rem 0.8rem;
  background: rgba(15, 23, 42, 0.62);
  color: rgba(226, 232, 240, 0.96);
  overflow-wrap: anywhere;
}
.opencode-chat-message--user .opencode-chat-message__bubble {
  background: rgba(37, 99, 235, 0.82);
  border-color: rgba(147, 197, 253, 0.36);
  color: #ffffff;
}
.opencode-chat-message__reasoning {
  color: rgba(203, 213, 225, 0.66);
  font-size: 0.82rem;
}
.opencode-chat-message__tool {
  width: min(100%, 48rem);
  border: 1px solid rgba(148, 163, 184, 0.28);
  border-radius: 0.7rem;
  background: rgba(2, 6, 23, 0.62);
  color: rgba(226, 232, 240, 0.95);
  overflow: hidden;
}
.opencode-chat-message__tool--warning {
  border-color: rgba(248, 113, 113, 0.62);
  background: rgba(69, 10, 10, 0.42);
}
.opencode-chat-message__tool-summary {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 0.75rem;
  align-items: center;
  cursor: pointer;
  padding: 0.65rem 0.75rem;
}
.opencode-chat-message__tool-title {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace;
  font-size: 0.78rem;
}
.opencode-chat-message__tool-meta {
  display: flex;
  align-items: center;
  gap: 0.45rem;
  color: rgba(203, 213, 225, 0.72);
  font-size: 0.72rem;
  white-space: nowrap;
}
.opencode-chat-message__tool-status {
  display: inline-flex;
  align-items: center;
  gap: 0.25rem;
}
.opencode-chat-message__tool-body {
  display: grid;
  gap: 0.55rem;
  border-top: 1px solid rgba(148, 163, 184, 0.18);
  padding: 0.65rem 0.75rem 0.75rem;
}
.opencode-chat-message__tool-output {
  max-height: 16rem;
  margin: 0;
  overflow: auto;
  border-radius: 0.55rem;
  background: rgba(15, 23, 42, 0.84);
  padding: 0.65rem;
  color: rgba(226, 232, 240, 0.94);
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace;
  font-size: 0.76rem;
  line-height: 1.45;
  white-space: pre;
}
.opencode-chat-message__tool-output--error {
  color: #fecaca;
}
.opencode-chat-message__loading {
  min-width: 2.4rem;
  text-align: center;
  color: rgba(226, 232, 240, 0.72);
}
`;

export function OpencodeChatMessage({ message }: OpencodeChatMessageProps) {
  const role = message.info.role;
  const renderableParts = message.parts.filter((part) => {
    if (part.type === "text" || part.type === "reasoning") {
      return part.text.trim().length > 0;
    }

    return true;
  });
  const isEmptyAssistant = role === "assistant" && renderableParts.length === 0 && !message.delta?.trim();

  return (
    <article className={`opencode-chat-message opencode-chat-message--${role}`}>
      <style>{shellStyle}</style>
      <div className="opencode-chat-message__stack">
        {renderableParts.map((part) => {
          if (part.type === "text") {
            return <MarkdownBubble key={part.id} text={part.text} />;
          }

          if (part.type === "reasoning") {
            return <ReasoningBubble key={part.id} text={part.text} />;
          }

          return <ToolCard key={part.id} tool={part} />;
        })}
        {message.delta?.trim() ? <MarkdownBubble text={message.delta} /> : null}
        {isEmptyAssistant ? <div className="opencode-chat-message__bubble opencode-chat-message__loading">...</div> : null}
      </div>
    </article>
  );
}

function MarkdownBubble({ text }: { text: string }) {
  return (
    <div
      className="opencode-chat-message__bubble markdown-body"
      dangerouslySetInnerHTML={{ __html: renderMarkdown(text) }}
    />
  );
}

function ReasoningBubble({ text }: { text: string }) {
  return <div className="opencode-chat-message__bubble opencode-chat-message__reasoning">{text}</div>;
}

function ToolCard({ tool }: { tool: ToolPartLike }) {
  const status = tool.state.status;
  const statusLabel = statusLabels[status];
  const command = tool.state.input.command.trim();
  const title = command || tool.title || tool.tool;
  const hasWarning = warningStatuses.has(status);
  const output = tool.state.output?.trim();
  const error = tool.state.error?.trim();

  return (
    <details
      className={`opencode-chat-message__tool${hasWarning ? " opencode-chat-message__tool--warning" : ""}`}
      open={activeStatuses.has(status) || hasWarning}
    >
      <summary className="opencode-chat-message__tool-summary">
        <span className="opencode-chat-message__tool-title">{title}</span>
        <span className="opencode-chat-message__tool-meta">
          <span className="opencode-chat-message__tool-status" title={status}>
            <span aria-hidden="true">{statusLabel.icon}</span>
            <span>{statusLabel.text}</span>
          </span>
          {tool.state.metadata.exit !== undefined && tool.state.metadata.exit !== null ? (
            <span>exit {tool.state.metadata.exit}</span>
          ) : null}
          {tool.state.metadata.durationMs !== undefined ? <span>{tool.state.metadata.durationMs} ms</span> : null}
        </span>
      </summary>
      <div className="opencode-chat-message__tool-body">
        {output ? <pre className="opencode-chat-message__tool-output">{output}</pre> : null}
        {error ? (
          <pre className="opencode-chat-message__tool-output opencode-chat-message__tool-output--error">{error}</pre>
        ) : null}
      </div>
    </details>
  );
}
