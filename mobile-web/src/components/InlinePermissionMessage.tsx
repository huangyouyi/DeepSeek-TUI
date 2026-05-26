import { useState } from "react";
import type { PermissionLike } from "../opencodeAdapter";
import "../styles/permission-alert.css";

export type InlinePermissionMessageProps = {
  permission: PermissionLike;
  onRespond: (response: "once" | "always" | "reject") => Promise<void>;
};

function formatCreatedAt(createdAt: number): string {
  return new Intl.DateTimeFormat("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit"
  }).format(new Date(createdAt));
}

function formatCommand(permission: PermissionLike): string {
  const value = permission.metadata.command || permission.pattern;

  if (permission.type !== "bash") {
    return value;
  }

  return value
    .split(/\r\n|\r|\n/)
    .map((line) => (line.trim().length > 0 ? `$ ${line}` : ""))
    .join("\n");
}

function errorMessage(error: unknown): string {
  if (error instanceof Error && error.message.trim().length > 0) {
    return error.message;
  }

  return "授权响应失败，请重试。";
}

export function InlinePermissionMessage({ permission, onRespond }: InlinePermissionMessageProps) {
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const reason =
    permission.metadata.risk_reason ?? "即将执行 shell 命令，可能对系统产生影响。请确认命令内容后再授权。";

  async function handleRespond(response: "once" | "always" | "reject") {
    setPending(true);
    setError(null);

    try {
      await onRespond(response);
    } catch (caughtError) {
      setError(errorMessage(caughtError));
    } finally {
      setPending(false);
    }
  }

  return (
    <article className="PermissionDialogCard PermissionDialogCard--accent" aria-busy={pending} aria-label="需要授权">
      <div className="PermissionDialogHeader">
        <span className="PermissionDialogTag">需要授权</span>
        <h3 className="PermissionDialogTitle">{permission.title}</h3>
        <p className="PermissionDialogDescription">
          <span>{permission.type}</span> · <span>会话 {permission.sessionID}</span> ·{" "}
          <span>{formatCreatedAt(permission.time.created)}</span>
        </p>
      </div>

      <div className="PermissionDialogBody">
        <pre className="PermissionDialogCard PermissionDialogCode">{formatCommand(permission)}</pre>
        <div className="PermissionDialogCard">{reason}</div>
        {error ? (
          <div className="PermissionDialogError" role="alert">
            {error}
          </div>
        ) : null}
      </div>

      <div className="PermissionDialogActions" aria-busy={pending}>
        <button
          className="PermissionDialogButton PermissionDialogButton--allow"
          type="button"
          disabled={pending}
          onClick={() => void handleRespond("once")}
        >
          仅这次执行
        </button>
        <button
          className="PermissionDialogButton PermissionDialogButton--always"
          type="button"
          disabled={pending}
          onClick={() => void handleRespond("always")}
        >
          本会话都允许
        </button>
        <button
          className="PermissionDialogButton PermissionDialogButton--reject"
          type="button"
          disabled={pending}
          onClick={() => void handleRespond("reject")}
        >
          拒绝并停止
        </button>
      </div>
      {pending ? <div role="status">正在处理授权响应...</div> : null}
    </article>
  );
}
