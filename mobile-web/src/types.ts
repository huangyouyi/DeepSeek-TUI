export type HealthResponse = {
  status: string;
  service: string;
  protocol: string;
  model: string;
};

export type SessionSummary = {
  id: string;
  title: string;
  created_at_ms: number;
  updated_at_ms: number;
};

export type MessagePart = {
  id: string;
  kind: string;
  text?: string;
  data?: unknown;
};

export type Message = {
  id: string;
  session_id: string;
  role: string;
  created_at_ms: number;
  parts: MessagePart[];
};

export type SshTarget = {
  host: string;
  user: string;
  port: number;
  key_present: boolean;
};

export type SshTargetUpdateRequest = {
  host: string;
  user: string;
  port: number;
};

export type SshCheckResponse = {
  status: "reachable" | "unreachable" | "timed_out" | "error";
  target: SshTarget;
  check_id: string;
  command: string;
  requires_approval: boolean;
  exit_code?: number;
  error_summary?: string;
  duration_ms?: number;
  timed_out: boolean;
};

export type DiagnosticKey =
  | "system_info"
  | "current_user"
  | "disk_usage"
  | "memory"
  | "network"
  | "working_directory";

export type DiagnosticPreset = {
  key: DiagnosticKey;
  label: string;
  command: string;
  requires_approval: boolean;
};

export type DiagnosticRequest = {
  session_id: string;
  diagnostic: DiagnosticKey;
};

export type DiagnosticResponse = {
  session_id: string;
  diagnostic: string;
  status: string;
  result: unknown;
};

export type CommandPrepareRequest = {
  session_id: string;
  command: string;
  cwd?: string;
};

export type PendingApproval = {
  id: string;
  session_id: string;
  command: string;
  cwd?: string;
  created_at_ms: number;
  status: string;
};

export type ApprovalRespondRequest = {
  response: "approve_once" | "reject";
};

export type ApprovalResponse = {
  approval: PendingApproval;
  status: string;
  result?: unknown;
};

export type AuditEntry = {
  id: string;
  session_id?: string;
  kind: string;
  created_at_ms: number;
  summary: string;
  metadata?: unknown;
};

export type ServerEventType =
  | "session.updated"
  | "message.updated"
  | "message.part.updated"
  | "tool.started"
  | "tool.stdout"
  | "tool.stderr"
  | "tool.completed"
  | "tool.failed"
  | "approval.asked"
  | "approval.replied"
  | "audit.updated"
  | "connection.updated";

export type ServerEvent = {
  type: ServerEventType;
  payload?: unknown;
};

export type ApiErrorBody = {
  code: string;
  message: string;
};
