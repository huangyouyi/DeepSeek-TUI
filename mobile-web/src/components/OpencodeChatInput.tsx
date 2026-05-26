import { useRef, useState, type CSSProperties, type KeyboardEvent } from "react";

export type OpencodeChatInputProps = {
  onSendMessage: (content: string) => void;
  onStop?: () => void;
  disabled?: boolean;
  isSending?: boolean;
};

const styles: Record<string, CSSProperties> = {
  root: {
    borderTop: "1px solid rgba(148, 163, 184, 0.18)",
    background: "#0b1020",
    padding: "14px"
  },
  form: {
    display: "flex",
    alignItems: "flex-end",
    gap: "10px",
    maxWidth: "860px",
    margin: "0 auto"
  },
  textarea: {
    flex: 1,
    minHeight: "46px",
    maxHeight: "132px",
    resize: "none",
    border: "1px solid rgba(148, 163, 184, 0.24)",
    borderRadius: "8px",
    padding: "12px 13px",
    background: "#111827",
    color: "#f8fafc",
    lineHeight: 1.45,
    outline: "none",
    font: "inherit"
  },
  button: {
    minWidth: "76px",
    minHeight: "46px",
    border: 0,
    borderRadius: "8px",
    padding: "0 14px",
    background: "#2563eb",
    color: "#ffffff",
    fontWeight: 700,
    cursor: "pointer"
  },
  stopButton: {
    background: "#b91c1c"
  },
  disabledButton: {
    opacity: 0.45,
    cursor: "not-allowed"
  },
  hint: {
    maxWidth: "860px",
    margin: "8px auto 0",
    color: "#94a3b8",
    fontSize: "12px",
    textAlign: "center"
  }
};

export function OpencodeChatInput({
  onSendMessage,
  onStop,
  disabled = false,
  isSending = false
}: OpencodeChatInputProps) {
  const [content, setContent] = useState("");
  const textareaRef = useRef<HTMLTextAreaElement | null>(null);

  const resizeTextarea = () => {
    const textarea = textareaRef.current;
    if (!textarea) return;
    textarea.style.height = "auto";
    textarea.style.height = `${Math.min(textarea.scrollHeight, 132)}px`;
  };

  const send = () => {
    if (disabled || isSending) return;
    const trimmed = content.trim();
    if (!trimmed) return;
    onSendMessage(content);
    setContent("");
    requestAnimationFrame(resizeTextarea);
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key !== "Enter" || event.shiftKey) return;
    event.preventDefault();
    send();
  };

  const handleAction = () => {
    if (isSending) {
      onStop?.();
      return;
    }
    send();
  };

  const buttonDisabled = disabled || (!isSending && !content.trim());

  return (
    <section style={styles.root} aria-label="消息输入区">
      <div style={styles.form}>
        <textarea
          ref={textareaRef}
          value={content}
          onChange={(event) => {
            setContent(event.target.value);
            requestAnimationFrame(resizeTextarea);
          }}
          onKeyDown={handleKeyDown}
          placeholder="输入消息..."
          disabled={disabled}
          rows={1}
          style={styles.textarea}
        />
        <button
          type="button"
          onClick={handleAction}
          disabled={buttonDisabled}
          aria-label={isSending ? "停止生成" : "发送消息"}
          style={{
            ...styles.button,
            ...(isSending ? styles.stopButton : null),
            ...(buttonDisabled ? styles.disabledButton : null)
          }}
        >
          {isSending ? "停止" : "发送"}
        </button>
      </div>
      <p style={styles.hint}>Enter 发送，Shift + Enter 换行</p>
    </section>
  );
}
