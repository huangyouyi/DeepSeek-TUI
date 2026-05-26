import type { ChatItem, ToolActivity } from "./state";
import type { PendingApproval, SessionSummary } from "./types";

export type Conversation = {
  id: string;
  title: string;
  createdAt: number;
  updatedAt: number;
};

export type PermissionLike = {
  id: string;
  type: "bash";
  pattern: string;
  sessionID: string;
  title: string;
  metadata: {
    command: string;
    cwd?: string;
    risk_reason?: string;
    target?: string;
    target_label?: string;
  };
  time: {
    created: number;
  };
};

export type TextPartLike = {
  id: string;
  type: "text";
  text: string;
  time: {
    created: number;
  };
};

export type ReasoningPartLike = {
  id: string;
  type: "reasoning";
  text: string;
  time: {
    created: number;
  };
};

export type ToolPartLike = {
  id: string;
  type: "tool";
  tool: "bash";
  title: string;
  state: {
    status: ToolVisualStatus;
    input: {
      command: string;
    };
    output?: string;
    error?: string;
    metadata: {
      exit?: number | null;
      durationMs?: number;
      requiresApproval?: boolean;
    };
  };
  time: {
    created: number;
  };
};

export type MessagePartLike = TextPartLike | ReasoningPartLike | ToolPartLike;

export type ToolVisualStatus =
  | "pending"
  | "running"
  | "started"
  | "updated"
  | "completed"
  | "error"
  | "rejected"
  | "queued"
  | "in_progress";

export type SessionMessageLike = {
  info: {
    id: string;
    sessionID: string;
    role: ChatItem["role"];
    time: {
      created: number;
    };
    error?: string;
  };
  parts: MessagePartLike[];
  delta?: string;
};

export type TimelineItemLike =
  | {
      id: string;
      kind: "message";
      createdAtMs: number;
      sessionId: string;
      message: SessionMessageLike;
    }
  | {
      id: string;
      kind: "permission";
      createdAtMs: number;
      sessionId: string;
      permission: PermissionLike;
    }
  | {
      id: string;
      kind: "tool";
      createdAtMs: number;
      sessionId: string;
      tool: ToolPartLike;
    }
  | {
      id: string;
      kind: "loading";
      createdAtMs: number;
      sessionId: string;
    };

export type BuildTimelineInput = {
  sessionId: string;
  activeChatItems?: ChatItem[];
  pendingApprovals?: PendingApproval[];
  activeToolActivities?: ToolActivity[];
  isLoading?: boolean;
};

export function mapSessionToConversation(session: SessionSummary): Conversation {
  return {
    id: session.id,
    title: session.title,
    createdAt: session.created_at_ms,
    updatedAt: session.updated_at_ms
  };
}

export function buildConversationTitleHint(content: string): string {
  const firstLine = content.trim().split(/\r\n|\r|\n/, 1)[0] ?? "";
  const normalized = firstLine.replace(/^#{1,6}\s*/, "").replace(/\s+/g, " ").trim();

  if (normalized.length <= 32) {
    return normalized;
  }

  return `${normalized.slice(0, 29)}...`;
}

export function mapApprovalToPermission(approval: PendingApproval): PermissionLike {
  return {
    id: approval.id,
    type: "bash",
    pattern: approval.command,
    sessionID: approval.session_id,
    title: "Bash 命令执行请求",
    metadata: optionalMetadata({
      command: approval.command,
      cwd: approval.cwd,
      risk_reason: approval.risk_reason,
      target: approval.target,
      target_label: approval.target_label
    }),
    time: {
      created: approval.created_at_ms
    }
  };
}

export function mapToolActivityToToolPart(activity: ToolActivity): ToolPartLike {
  return {
    id: activity.id,
    type: "tool",
    tool: "bash",
    title: activity.title || activity.command || "bash",
    state: {
      status: normalizeToolStatus(activity.status),
      input: {
        command: activity.command
      },
      output: activity.output ?? activity.stdout,
      error: activity.stderr,
      metadata: optionalMetadata({
        exit: activity.exitCode,
        durationMs: activity.durationMs,
        requiresApproval: activity.requiresApproval
      })
    },
    time: {
      created: activity.createdAtMs
    }
  };
}

export function mapChatItemToSessionMessage(item: ChatItem, sessionId: string): SessionMessageLike {
  const info: SessionMessageLike["info"] = {
    id: item.id,
    sessionID: sessionId,
    role: item.role,
    time: {
      created: item.createdAtMs
    }
  };

  if (isErrorStatus(item.status)) {
    info.error = item.status;
  }

  return {
    info,
    parts: [
      {
        id: `${item.id}:text`,
        type: "text",
        text: item.text,
        time: {
          created: item.createdAtMs
        }
      }
    ],
    delta: undefined
  };
}

export function buildTimelineItems(input: BuildTimelineInput): TimelineItemLike[] {
  const messages = (input.activeChatItems ?? []).map((item): TimelineItemLike => {
    const message = mapChatItemToSessionMessage(item, input.sessionId);
    return {
      id: `message:${item.id}`,
      kind: "message",
      createdAtMs: item.createdAtMs,
      sessionId: input.sessionId,
      message
    };
  });

  const permissions = (input.pendingApprovals ?? [])
    .filter((approval) => approval.session_id === input.sessionId)
    .map((approval): TimelineItemLike => {
      const permission = mapApprovalToPermission(approval);
      return {
        id: `permission:${approval.id}`,
        kind: "permission",
        createdAtMs: approval.created_at_ms,
        sessionId: input.sessionId,
        permission
      };
    });

  const tools = (input.activeToolActivities ?? []).map((activity): TimelineItemLike => {
    const tool = mapToolActivityToToolPart(activity);
    return {
      id: `tool:${activity.id}`,
      kind: "tool",
      createdAtMs: activity.createdAtMs,
      sessionId: input.sessionId,
      tool
    };
  });

  return [...messages, ...permissions, ...tools].sort((left, right) => {
    const byTime = left.createdAtMs - right.createdAtMs;
    return byTime === 0 ? left.id.localeCompare(right.id) : byTime;
  });
}

export function buildProductTimeline(input: BuildTimelineInput): TimelineItemLike[] {
  const items = buildTimelineItems(input);

  if (!input.isLoading) {
    return items;
  }

  return [
    ...items,
    {
      id: `loading:${input.sessionId}`,
      kind: "loading",
      createdAtMs: Number.MAX_SAFE_INTEGER,
      sessionId: input.sessionId
    }
  ];
}

function normalizeToolStatus(status: string): ToolVisualStatus {
  switch (status) {
    case "pending_approval":
      return "pending";
    case "failed":
      return "error";
    case "pending":
    case "running":
    case "started":
    case "updated":
    case "completed":
    case "error":
    case "rejected":
    case "queued":
    case "in_progress":
      return status;
    default:
      return "updated";
  }
}

function isErrorStatus(status: string | undefined): boolean {
  return status === "error" || status === "failed";
}

function optionalMetadata<T extends Record<string, unknown>>(metadata: T): T {
  return Object.fromEntries(Object.entries(metadata).filter(([, value]) => value !== undefined)) as T;
}
