export type ToolActivityItem = {
  id: string;
  status: string;
  title?: string;
  command?: string;
  exitCode?: number | null;
  durationMs?: number;
  stdout?: string;
  stderr?: string;
  output?: unknown;
  requiresApproval?: boolean;
};

export type ToolActivityProps = {
  activities: ToolActivityItem[];
};

function renderOutput(output: unknown): string {
  if (output === null || output === undefined) {
    return "";
  }

  if (typeof output === "string") {
    return output;
  }

  try {
    return JSON.stringify(output, null, 2);
  } catch {
    return String(output);
  }
}

function streamSummary(label: string, text: string): string {
  const lineCount = text.length === 0 ? 0 : text.split(/\r\n|\r|\n/).length;
  return `${label} · ${lineCount} ${lineCount === 1 ? "line" : "lines"} · ${text.length} chars`;
}

function statusLabel(status: string): string {
  switch (status) {
    case "running":
      return "执行中";
    case "pending_approval":
    case "waiting_for_approval":
    case "awaiting_approval":
      return "待授权";
    case "completed":
      return "已完成";
    case "failed":
      return "失败";
    case "rejected":
      return "已拒绝";
    case "stopped":
    case "cancelled":
    case "canceled":
      return "已停止";
    default:
      return status;
  }
}

function ToolStream({ label, value, variant }: { label: string; value: unknown; variant?: "stderr" }) {
  const text = renderOutput(value);

  if (!text) {
    return null;
  }

  return (
    <details className="tool-activity__details">
      <summary className="tool-activity__summary">{streamSummary(label, text)}</summary>
      <pre className={`tool-activity__stream${variant === "stderr" ? " tool-activity__stream--stderr" : ""}`}>
        {text}
      </pre>
    </details>
  );
}

export function ToolActivity({ activities }: ToolActivityProps) {
  return (
    <section className="tool-activity" aria-label="Tool activity">
      {activities.length === 0 ? (
        <div className="tool-activity__empty">工具执行细节会在这里出现，包括命令状态、授权结果和折叠的输出。</div>
      ) : (
        <div className="tool-activity__list">
          {activities.map((activity) => (
            <article className="tool-activity__row" key={activity.id}>
              <div className="tool-activity__meta">
                <span className="tool-activity__status" title={activity.status}>
                  {statusLabel(activity.status)}
                </span>
                {activity.exitCode !== undefined && activity.exitCode !== null ? (
                  <span className="tool-activity__exit-code">exit {activity.exitCode}</span>
                ) : null}
                {activity.durationMs !== undefined ? (
                  <span className="tool-activity__duration">{activity.durationMs} ms</span>
                ) : null}
                {activity.requiresApproval ? <span className="tool-activity__approval">需要授权</span> : null}
              </div>

              <strong className="tool-activity__title">{activity.title || "工具命令"}</strong>
              {activity.command ? <code className="tool-activity__command">{activity.command}</code> : null}

              <div className="tool-activity__streams">
                <ToolStream label="stdout" value={activity.stdout} />
                <ToolStream label="stderr" value={activity.stderr} variant="stderr" />
                <ToolStream label="output" value={activity.output} />
              </div>
            </article>
          ))}
        </div>
      )}
    </section>
  );
}
