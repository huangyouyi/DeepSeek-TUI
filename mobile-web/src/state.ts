import type {
  AuditEntry,
  DiagnosticPreset,
  Message,
  PendingApproval,
  ServerEvent,
  SessionSummary,
  SshCheckResponse,
  SshTarget
} from "./types";

export type SseStatus = "connecting" | "connected" | "disconnected" | "error";

export type TimelineEntry = {
  id: string;
  kind: "session" | "message" | "user-message" | "assistant-message" | "tool" | "stdout" | "stderr" | "audit" | "connection";
  title: string;
  text: string;
  createdAtMs: number;
};

export type AppState = {
  connection: {
    server: "unknown" | "ok" | "error";
    sse: SseStatus;
    status: string;
    target?: SshTarget;
  };
  sessions: SessionSummary[];
  activeSessionId?: string;
  messages: Message[];
  pendingApprovals: PendingApproval[];
  audit: AuditEntry[];
  timeline: TimelineEntry[];
  approvalActions: Array<"approve_once" | "reject">;
};

export const initialAppState: AppState = {
  connection: {
    server: "unknown",
    sse: "connecting",
    status: "connecting"
  },
  sessions: [],
  messages: [],
  pendingApprovals: [],
  audit: [],
  timeline: [],
  approvalActions: ["approve_once", "reject"]
};

export const fallbackDiagnosticPresets: DiagnosticPreset[] = [
  { key: "system_info", label: "System info", command: "uname -a", requires_approval: false },
  { key: "current_user", label: "Current user", command: "id", requires_approval: false },
  { key: "disk_usage", label: "Disk usage", command: "df -h", requires_approval: false },
  { key: "memory", label: "Memory", command: "free -m || cat /proc/meminfo", requires_approval: false },
  { key: "network", label: "Network", command: "ip addr || ifconfig", requires_approval: false },
  { key: "working_directory", label: "Working directory", command: "pwd", requires_approval: false }
];

export function resolveDiagnosticPresets(presets: DiagnosticPreset[] | undefined): DiagnosticPreset[] {
  return presets && presets.length > 0 ? presets : fallbackDiagnosticPresets;
}

export function reduceEvent(state: AppState, event: ServerEvent): AppState {
  switch (event.type) {
    case "connection.updated": {
      const payload = objectPayload(event.payload);
      const status = stringField(payload, "status", state.connection.status);
      return {
        ...state,
        connection: {
          ...state.connection,
          sse: status === "connected" ? "connected" : state.connection.sse,
          status
        },
        timeline: prependTimeline(state.timeline, {
          kind: "connection",
          title: "Connection",
          text: status
        })
      };
    }
    case "session.updated": {
      const session = event.payload as SessionSummary;
      return {
        ...state,
        activeSessionId: session.id,
        sessions: upsertById(state.sessions, session),
        timeline: prependTimeline(state.timeline, {
          kind: "session",
          title: session.title || "Session updated",
          text: session.id
        })
      };
    }
    case "message.updated":
    case "message.part.updated": {
      const payload = objectPayload(event.payload);
      const role = stringField(payload, "role", "");
      if (role === "user" || role === "assistant") {
        return {
          ...state,
          timeline: prependTimeline(state.timeline, {
            kind: role === "user" ? "user-message" : "assistant-message",
            title: role === "user" ? "You" : "Assistant",
            text: messageText(payload, event.payload)
          })
        };
      }

      return {
        ...state,
        timeline: prependTimeline(state.timeline, {
          kind: "message",
          title: event.type,
          text: stringifyPayload(event.payload)
        })
      };
    }
    case "tool.started":
    case "tool.completed":
    case "tool.failed": {
      const payload = objectPayload(event.payload);
      return {
        ...state,
        timeline: prependTimeline(state.timeline, {
          kind: "tool",
          title: event.type.replace(".", " "),
          text: stringField(payload, "command", stringifyPayload(event.payload))
        })
      };
    }
    case "tool.stdout":
    case "tool.stderr": {
      const payload = objectPayload(event.payload);
      const kind = event.type === "tool.stdout" ? "stdout" : "stderr";
      return {
        ...state,
        timeline: prependTimeline(state.timeline, {
          kind,
          title: kind,
          text: stringField(payload, "text", stringifyPayload(event.payload))
        })
      };
    }
    case "approval.asked": {
      const approval = event.payload as PendingApproval;
      return {
        ...state,
        pendingApprovals: upsertById(state.pendingApprovals, approval),
        timeline: prependTimeline(state.timeline, {
          kind: "tool",
          title: "Approval required",
          text: approval.command
        })
      };
    }
    case "approval.replied": {
      const payload = objectPayload(event.payload);
      const id = stringField(payload, "id", "");
      return {
        ...state,
        pendingApprovals: state.pendingApprovals.filter((approval) => approval.id !== id),
        timeline: prependTimeline(state.timeline, {
          kind: "tool",
          title: "Approval replied",
          text: stringField(payload, "status", "updated")
        })
      };
    }
    case "audit.updated": {
      const entry = event.payload as AuditEntry;
      return {
        ...state,
        audit: upsertById(state.audit, entry).slice(0, 30),
        timeline: prependTimeline(state.timeline, {
          kind: "audit",
          title: entry.kind,
          text: entry.summary
        })
      };
    }
    default:
      return state;
  }
}

export function withSseStatus(state: AppState, sse: SseStatus): AppState {
  return {
    ...state,
    connection: {
      ...state.connection,
      sse
    }
  };
}

function upsertById<T extends { id: string }>(items: T[], item: T): T[] {
  const existing = items.filter((candidate) => candidate.id !== item.id);
  return [item, ...existing];
}

function prependTimeline(items: TimelineEntry[], input: Omit<TimelineEntry, "id" | "createdAtMs">): TimelineEntry[] {
  const deduped = items.filter((item, index) => {
    if (index >= 8) {
      return true;
    }
    return !(item.kind === input.kind && item.title === input.title && item.text === input.text);
  });
  return [
    {
      id: `${Date.now()}-${Math.random().toString(36).slice(2)}`,
      createdAtMs: Date.now(),
      ...input
    },
    ...deduped
  ].slice(0, 80);
}

export function buildFeedbackReport(state: AppState, sshCheck?: SshCheckResponse | null): string {
  const target = state.connection.target
    ? `${state.connection.target.user}@${state.connection.target.host}:${state.connection.target.port}`
    : "unknown";
  const lines = [
    "DeepSeek Mobile SSH report",
    "",
    `Server: ${state.connection.server}`,
    `SSE: ${state.connection.sse}`,
    `Target: ${target}`,
    `Pending approvals: ${state.pendingApprovals.length}`
  ];

  if (sshCheck) {
    lines.push(`SSH check: ${sshCheck.status}`);
  }

  if (state.pendingApprovals.length > 0) {
    lines.push("", "Pending approval commands:");
    state.pendingApprovals.forEach((approval, index) => {
      lines.push(`${index + 1}. ${approval.command}`);
    });
  }

  lines.push("", "Recent timeline:");
  if (state.timeline.length === 0) {
    lines.push("(empty)");
  } else {
    state.timeline.slice(0, 30).forEach((entry) => {
      lines.push("", `[${entry.title}] ${new Date(entry.createdAtMs).toLocaleString()}`);
      lines.push(entry.text);
    });
  }

  return lines.join("\n");
}

function objectPayload(payload: unknown): Record<string, unknown> {
  return typeof payload === "object" && payload !== null ? (payload as Record<string, unknown>) : {};
}

function stringField(payload: Record<string, unknown>, field: string, fallback: string): string {
  const value = payload[field];
  return typeof value === "string" ? value : fallback;
}

function stringifyPayload(payload: unknown): string {
  if (typeof payload === "string") {
    return payload;
  }

  try {
    return JSON.stringify(payload ?? {});
  } catch {
    return String(payload);
  }
}

function messageText(payload: Record<string, unknown>, fallbackPayload: unknown): string {
  const text = payload.text;
  if (typeof text === "string") {
    return text;
  }

  const content = payload.content;
  if (typeof content === "string") {
    return content;
  }

  const parts = payload.parts;
  if (Array.isArray(parts)) {
    const joined = parts
      .map((part) => {
        if (typeof part !== "object" || part === null) {
          return "";
        }
        const value = (part as Record<string, unknown>).text;
        return typeof value === "string" ? value : "";
      })
      .filter(Boolean)
      .join("\n");
    if (joined) {
      return joined;
    }
  }

  return stringifyPayload(fallbackPayload);
}
