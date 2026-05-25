import type {
  ApprovalResponse,
  AuditEntry,
  CommandPrepareRequest,
  DiagnosticKey,
  DiagnosticRequest,
  DiagnosticResponse,
  HealthResponse,
  Message,
  PendingApproval,
  SessionSummary,
  SshTarget
} from "./types";

export type ApiContext = {
  fetch: typeof fetch;
};

const defaultApi: ApiContext = {
  fetch: globalThis.fetch.bind(globalThis)
};

async function requestJson<T>(path: string, init: RequestInit = {}, api: ApiContext = defaultApi): Promise<T> {
  const response = await api.fetch(path, {
    ...init,
    headers: {
      ...(init.body ? { "content-type": "application/json" } : {}),
      ...init.headers
    }
  });

  if (!response.ok) {
    let message = `${response.status} ${response.statusText}`;
    try {
      const body = (await response.json()) as { message?: string };
      message = body.message ?? message;
    } catch {
      // Keep the HTTP status fallback.
    }
    throw new Error(message);
  }

  return (await response.json()) as T;
}

export function getHealth(api?: ApiContext): Promise<HealthResponse> {
  return requestJson<HealthResponse>("/health", undefined, api);
}

export function getSshTarget(api?: ApiContext): Promise<SshTarget> {
  return requestJson<SshTarget>("/api/ssh/target", undefined, api);
}

export function listSessions(api?: ApiContext): Promise<SessionSummary[]> {
  return requestJson<SessionSummary[]>("/api/sessions", undefined, api);
}

export function createSession(api?: ApiContext): Promise<SessionSummary> {
  return requestJson<SessionSummary>("/api/sessions", { method: "POST" }, api);
}

export function listMessages(sessionId: string, api?: ApiContext): Promise<Message[]> {
  return requestJson<Message[]>(`/api/sessions/${encodeURIComponent(sessionId)}/messages`, undefined, api);
}

export function runDiagnostic(
  input: { sessionId: string; diagnostic: DiagnosticKey },
  api?: ApiContext
): Promise<DiagnosticResponse> {
  const body: DiagnosticRequest = {
    session_id: input.sessionId,
    diagnostic: input.diagnostic
  };

  return requestJson<DiagnosticResponse>(
    "/api/diagnostics/run",
    { method: "POST", body: JSON.stringify(body) },
    api
  );
}

export function prepareCommand(
  input: { sessionId: string; command: string; cwd?: string },
  api?: ApiContext
): Promise<PendingApproval> {
  const body: CommandPrepareRequest = {
    session_id: input.sessionId,
    command: input.command,
    cwd: input.cwd || undefined
  };

  return requestJson<PendingApproval>("/api/commands/prepare", { method: "POST", body: JSON.stringify(body) }, api);
}

export function approveCommand(id: string, api?: ApiContext): Promise<ApprovalResponse> {
  return respondToApproval(id, "approve_once", api);
}

export function rejectCommand(id: string, api?: ApiContext): Promise<ApprovalResponse> {
  return respondToApproval(id, "reject", api);
}

function respondToApproval(
  id: string,
  response: "approve_once" | "reject",
  api?: ApiContext
): Promise<ApprovalResponse> {
  return requestJson<ApprovalResponse>(
    `/api/approvals/${encodeURIComponent(id)}/respond`,
    { method: "POST", body: JSON.stringify({ response }) },
    api
  );
}

export function getRecentAudit(api?: ApiContext): Promise<AuditEntry[]> {
  return requestJson<AuditEntry[]>("/api/audit/recent", undefined, api);
}
