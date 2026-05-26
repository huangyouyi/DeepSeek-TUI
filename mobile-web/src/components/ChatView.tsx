import type { FormEvent } from "react";

export type ChatMessage = {
  id: string;
  role: "user" | "assistant" | string;
  text: string;
};

export type ChatComposerProps = {
  value: string;
  onChange: (value: string) => void;
  onSubmit: () => void;
  disabled?: boolean;
  busy?: boolean;
  placeholder?: string;
  submitLabel?: string;
  busyLabel?: string;
};

export type ChatViewProps = {
  messages: ChatMessage[];
  finalAnswer?: string | null;
  composer: ChatComposerProps;
  onCopyFinalAnswer?: () => void;
};

export function ChatView({ messages, finalAnswer, composer, onCopyFinalAnswer }: ChatViewProps) {
  const hasFinalAnswer = Boolean(finalAnswer?.trim());
  const submitLabel = composer.busy ? composer.busyLabel ?? "Sending" : composer.submitLabel ?? "Send";

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    composer.onSubmit();
  }

  return (
    <section className="chat-view" aria-label="Mobile web chat">
      {hasFinalAnswer ? (
        <section className="chat-view__final-answer" aria-label="Final answer">
          <div className="chat-view__final-label">Final answer</div>
          <p className="chat-view__final-text">{finalAnswer}</p>
          <button
            className="chat-view__copy-final"
            type="button"
            onClick={onCopyFinalAnswer}
            disabled={!onCopyFinalAnswer}
          >
            Copy final answer
          </button>
        </section>
      ) : null}

      <div className="chat-view__messages" aria-live="polite">
        {messages.length === 0 ? (
          <div className="chat-view__empty">No conversation yet</div>
        ) : (
          messages.map((message) => {
            const isUser = message.role === "user";
            const bubbleClass = isUser ? "chat-view__bubble--user" : "chat-view__bubble--assistant";
            const label = isUser ? "You" : "Assistant";

            return (
              <article className={`chat-view__message ${bubbleClass}`} key={message.id}>
                <div className="chat-view__message-label">{label}</div>
                <p className="chat-view__message-text">{message.text}</p>
              </article>
            );
          })
        )}
      </div>

      <form className="chat-view__composer" onSubmit={handleSubmit}>
        <textarea
          className="chat-view__composer-input"
          value={composer.value}
          onChange={(event) => composer.onChange(event.currentTarget.value)}
          disabled={composer.disabled || composer.busy}
          placeholder={composer.placeholder ?? "Ask about this session"}
          rows={3}
        />
        <button
          className="chat-view__composer-submit"
          type="submit"
          disabled={composer.disabled || composer.busy || composer.value.trim().length === 0}
        >
          {submitLabel}
        </button>
      </form>
    </section>
  );
}
