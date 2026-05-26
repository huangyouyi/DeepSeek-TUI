import type {
  AuditEntry,
  ApprovalAction,
  DiagnosticPreset,
  MessagePart,
  Message,
  PendingApproval,
  ServerEvent,
  SessionDeletedPayload,
  SessionSummary,
  SshCheckResponse,
  SshTarget,
  ToolPartData
} from "./types";

export type SseStatus = "connecting" | "connected" | "disconnected" | "error";

export type TimelineEntry = {
  id: string;
  kind: "session" | "message" | "user-message" | "assistant-message" | "tool" | "stdout" | "stderr" | "audit" | "connection";
  title: string;
  text: string;
  createdAtMs: number;
};

export type ChatItem = {
  id: string;
  role: "user" | "assistant";
  text: string;
  createdAtMs: number;
  status?: string;
  isFinal?: boolean;
};

export type ToolActivity = {
  id: string;
  status: string;
  command: string;
  title?: string;
  stdout?: string;
  stderr?: string;
  output?: string;
  exitCode?: number | null;
  durationMs?: number;
  requiresApproval?: boolean;
  createdAtMs: number;
};

export type ExecutionStatus = {
  state: "idle" | "running" | "waiting" | "error";
  label: string;
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
  chatItems: ChatItem[];
  toolActivities: ToolActivity[];
  approvalActions: ApprovalAction[];
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
  chatItems: [],
  toolActivities: [],
  approvalActions: ["approve_once", "approve_session", "reject_stop"]
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
    case "session.deleted": {
      const deletion = sessionDeletedPayload(event.payload);
      const sessionId = deletion.id ?? deletion.session_id;
      if (!sessionId) {
        return {
          ...state,
          timeline: prependTimeline(state.timeline, {
            kind: "session",
            title: "Session deleted",
            text: stringifyPayload(event.payload)
          })
        };
      }

      return {
        ...state,
        activeSessionId: state.activeSessionId === sessionId ? undefined : state.activeSessionId,
        sessions: state.sessions.filter((session) => session.id !== sessionId),
        messages: state.messages.filter((message) => message.session_id !== sessionId),
        pendingApprovals: state.pendingApprovals.filter((approval) => approval.session_id !== sessionId),
        timeline: prependTimeline(state.timeline, {
          kind: "session",
          title: "Session deleted",
          text: sessionId
        })
      };
    }
    case "message.updated": {
      const payload = objectPayload(event.payload);
      const role = stringField(payload, "role", "");
      if (role === "user" || role === "assistant") {
        const text = messageText(payload, event.payload);
        if (!text.trim()) {
          return state;
        }
        return {
          ...state,
          chatItems: upsertChatItem(state.chatItems, {
            id: messageId(payload, role, text),
            role,
            text,
            createdAtMs: numberField(payload, "created_at_ms", numberField(payload, "createdAtMs", Date.now())),
            status: stringField(payload, "status", undefined),
            isFinal: role === "assistant" && isFinalAssistantPayload(payload, text) ? true : undefined
          }),
          timeline: prependTimeline(state.timeline, {
            kind: role === "user" ? "user-message" : "assistant-message",
            title: role === "user" ? "You" : "Assistant",
            text
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
    case "message.part.updated": {
      const payload = objectPayload(event.payload);
      const part = messagePartField(payload, "part");
      if (part?.kind === "tool") {
        const activity = toolActivityFromPart(part);
        return {
          ...state,
          toolActivities: upsertToolActivity(state.toolActivities, activity),
          timeline: upsertTimeline(state.timeline, {
            id: part.id,
            kind: "tool",
            title: toolPartTitle(part),
            text: toolPartText(part)
          })
        };
      }

      const role = stringField(payload, "role", "");
      if (role === "user" || role === "assistant") {
        const text = messageText(payload, event.payload);
        if (!text.trim()) {
          return state;
        }
        return {
          ...state,
          chatItems: upsertChatItem(state.chatItems, {
            id: messageId(payload, role, text),
            role,
            text,
            createdAtMs: numberField(payload, "created_at_ms", numberField(payload, "createdAtMs", Date.now())),
            status: stringField(payload, "status", undefined),
            isFinal: role === "assistant" && isFinalAssistantPayload(payload, text) ? true : undefined
          }),
          timeline: prependTimeline(state.timeline, {
            kind: role === "user" ? "user-message" : "assistant-message",
            title: role === "user" ? "You" : "Assistant",
            text
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
      const activity = toolActivityFromLegacyEvent(state.toolActivities, event.type, payload);
      return {
        ...state,
        toolActivities: upsertToolActivity(state.toolActivities, activity),
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
      const activity = toolActivityFromLegacyEvent(state.toolActivities, event.type, payload);
      return {
        ...state,
        toolActivities: upsertToolActivity(state.toolActivities, activity),
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

export function selectChatItems(state: AppState): ChatItem[] {
  return [...state.chatItems].sort((left, right) => left.createdAtMs - right.createdAtMs);
}

export function selectFinalAnswer(state: AppState): ChatItem | undefined {
  return [...selectChatItems(state)]
    .reverse()
    .find((item) => item.role === "assistant" && item.isFinal === true && item.text.trim().length > 0);
}

export function selectToolActivities(state: AppState): ToolActivity[] {
  return state.toolActivities;
}

export function selectExecutionStatus(state: AppState, busy: string | null): ExecutionStatus {
  if (state.connection.sse === "error" || state.connection.sse === "disconnected") {
    return { state: "error", label: "连接中断" };
  }

  if (state.pendingApprovals.length > 0) {
    return { state: "waiting", label: "等待授权" };
  }

  if (busy === "agent" || state.toolActivities.some((activity) => isRunningToolActivity(activity))) {
    return { state: "running", label: "执行中" };
  }

  return { state: "idle", label: "空闲" };
}

export function buildFinalAnswerReport(state: AppState): string {
  return selectFinalAnswer(state)?.text ?? "";
}

function upsertById<T extends { id: string }>(items: T[], item: T): T[] {
  const existing = items.filter((candidate) => candidate.id !== item.id);
  return [item, ...existing];
}

function upsertChatItem(items: ChatItem[], item: ChatItem): ChatItem[] {
  const existing = items.filter((candidate) => candidate.id !== item.id && !sameChatContent(candidate, item));
  return [...existing, item];
}

function sameChatContent(left: ChatItem, right: ChatItem): boolean {
  return left.role === right.role && left.text.trim() === right.text.trim();
}

function upsertToolActivity(items: ToolActivity[], item: ToolActivity): ToolActivity[] {
  const existing = items.find((candidate) => candidate.id === item.id);
  const withoutExisting = items.filter((candidate) => candidate.id !== item.id);
  return [
    {
      ...existing,
      ...item,
      stdout: appendStreamText(existing?.stdout, item.stdout),
      stderr: appendStreamText(existing?.stderr, item.stderr),
      createdAtMs: existing?.createdAtMs ?? item.createdAtMs
    },
    ...withoutExisting
  ];
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

function upsertTimeline(items: TimelineEntry[], input: Omit<TimelineEntry, "createdAtMs">): TimelineEntry[] {
  const existing = items.find((item) => item.id === input.id);
  const withoutExisting = items.filter((item) => item.id !== input.id);
  return [
    {
      createdAtMs: existing?.createdAtMs ?? Date.now(),
      ...input
    },
    ...withoutExisting
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

function sessionDeletedPayload(payload: unknown): SessionDeletedPayload {
  const value = objectPayload(payload);
  return {
    id: stringField(value, "id", undefined),
    session_id: stringField(value, "session_id", undefined),
    removed_pending_approvals: numberField(value, "removed_pending_approvals", undefined),
    deleted_at_ms: numberField(value, "deleted_at_ms", undefined)
  };
}

function stringField<T extends string | undefined>(payload: Record<string, unknown>, field: string, fallback: T): string | T {
  const value = payload[field];
  return typeof value === "string" ? value : fallback;
}

function numberField<T extends number | undefined>(payload: Record<string, unknown>, field: string, fallback: T): number | T {
  const value = payload[field];
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

function messagePartField(payload: Record<string, unknown>, field: string): MessagePart | undefined {
  const value = payload[field];
  if (typeof value !== "object" || value === null) {
    return undefined;
  }

  const part = value as Record<string, unknown>;
  if (typeof part.id !== "string" || typeof part.kind !== "string") {
    return undefined;
  }

  return {
    id: part.id,
    kind: part.kind,
    text: typeof part.text === "string" ? part.text : undefined,
    data: part.data
  };
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
        const partPayload = part as Record<string, unknown>;
        if (partPayload.kind === "tool") {
          return "";
        }
        const value = partPayload.text;
        return typeof value === "string" ? value : "";
      })
      .filter(Boolean)
      .join("\n");
    if (joined) {
      return joined;
    }
    return "";
  }

  return stringifyPayload(fallbackPayload);
}

function messageId(payload: Record<string, unknown>, role: "user" | "assistant", text: string): string {
  return stringField(payload, "id", stringField(payload, "message_id", `${role}:${text}`));
}

function isFinalAssistantPayload(payload: Record<string, unknown>, text: string): boolean {
  if (!text.trim()) {
    return false;
  }

  const status = stringField(payload, "status", "");
  if (status === "waiting_for_approval" || status === "awaiting_approval" || status === "pending_approval") {
    return false;
  }

  return true;
}

function isRunningToolActivity(activity: ToolActivity): boolean {
  return ["running", "started", "pending", "queued", "in_progress"].includes(activity.status.toLowerCase());
}

function toolActivityFromPart(part: MessagePart): ToolActivity {
  const data = toolData(part.data);
  return {
    id: part.id,
    status: data.status ?? "updated",
    command: data.command ?? data.tool ?? "",
    title: data.title,
    stdout: data.stdout,
    stderr: data.stderr,
    output: valueText(data.output),
    exitCode: data.exit_code,
    durationMs: data.duration_ms,
    requiresApproval: data.requires_approval,
    createdAtMs: Date.now()
  };
}

function toolActivityFromLegacyEvent(
  existing: ToolActivity[],
  eventType: ServerEvent["type"],
  payload: Record<string, unknown>
): ToolActivity {
  const id = toolActivityId(existing, payload);
  const current = existing.find((activity) => activity.id === id);
  const command = stringField(payload, "command", current?.command ?? "");
  const status = stringField(payload, "status", legacyToolStatus(eventType));
  const text = stringField(payload, "text", "");

  return {
    id,
    status,
    command,
    stdout: eventType === "tool.stdout" ? text : undefined,
    stderr: eventType === "tool.stderr" ? text : undefined,
    output: valueText(payload.output),
    exitCode: nullableNumberField(payload, "exit_code", current?.exitCode),
    durationMs: numberField(payload, "duration_ms", current?.durationMs ?? 0) || current?.durationMs,
    requiresApproval: booleanField(payload, "requires_approval", current?.requiresApproval),
    createdAtMs: Date.now()
  };
}

function toolActivityId(existing: ToolActivity[], payload: Record<string, unknown>): string {
  const id = stringField(payload, "id", "");
  if (id) {
    return id;
  }

  const command = stringField(payload, "command", "");
  if (command) {
    return `command:${command}`;
  }

  return existing[0]?.id ?? "tool";
}

function legacyToolStatus(eventType: ServerEvent["type"]): string {
  switch (eventType) {
    case "tool.started":
      return "running";
    case "tool.completed":
      return "completed";
    case "tool.failed":
      return "failed";
    default:
      return "updated";
  }
}

function appendStreamText(existing: string | undefined, next: string | undefined): string | undefined {
  if (!next) {
    return existing;
  }
  if (!existing) {
    return next;
  }
  return `${existing}\n${next}`;
}

function nullableNumberField(
  payload: Record<string, unknown>,
  field: string,
  fallback: number | null | undefined
): number | null | undefined {
  const value = payload[field];
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

function booleanField(
  payload: Record<string, unknown>,
  field: string,
  fallback: boolean | undefined
): boolean | undefined {
  const value = payload[field];
  return typeof value === "boolean" ? value : fallback;
}

function toolPartTitle(part: MessagePart): string {
  const data = toolData(part.data);
  return data.status ?? data.title ?? data.tool ?? "tool";
}

function toolPartText(part: MessagePart): string {
  const data = toolData(part.data);
  const lines = [
    data.command,
    data.status,
    part.text ?? valueText(data.output),
    data.stdout,
    data.stderr
  ];

  const text = lines.filter((line): line is string => typeof line === "string" && line.length > 0).join("\n");
  return text || stringifyPayload(part.data ?? part);
}

function toolData(data: unknown): ToolPartData {
  return typeof data === "object" && data !== null ? (data as ToolPartData) : {};
}

function valueText(value: unknown): string | undefined {
  if (typeof value === "string") {
    return value;
  }

  if (value === undefined || value === null) {
    return undefined;
  }

  return stringifyPayload(value);
}
