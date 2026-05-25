import type {
  AgentTurnRequest,
  AgentTurnResponse,
  ApprovalResponse,
  AuditEntry,
  CommandPrepareRequest,
  DiagnosticKey,
  DiagnosticPreset,
  DiagnosticRequest,
  DiagnosticResponse,
  HealthResponse,
  Message,
  PendingApproval,
  SessionSummary,
  SshCheckResponse,
  SshTarget,
  SshTargetUpdateRequest
} from "./types";

export type ApiContext = {
  fetch: typeof fetch;
  accessToken?: string;
};

const defaultApi: ApiContext = {
  fetch: ((input, init) => globalThis.fetch(input, init)) as typeof fetch
};

export const ACCESS_TOKEN_STORAGE_KEY = "deepseek.mobileWeb.accessToken";

async function requestJson<T>(path: string, init: RequestInit = {}, api: ApiContext = defaultApi): Promise<T> {
  const accessToken = resolveAccessToken(api);
  const response = await api.fetch(path, {
    ...init,
    headers: {
      ...(init.body ? { "content-type": "application/json" } : {}),
      ...(accessToken ? { "X-Mobile-Web-Token": accessToken } : {}),
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
    throw new Error(redactAccessToken(message, accessToken));
  }

  return (await response.json()) as T;
}

export function buildEventUrl(accessToken = getStoredAccessToken()): string {
  const token = normalizeAccessToken(accessToken);
  if (!token) {
    return "/event";
  }

  const params = new URLSearchParams({ access_token: token });
  return `/event?${params.toString()}`;
}

export function getStoredAccessToken(): string {
  try {
    return globalThis.localStorage?.getItem(ACCESS_TOKEN_STORAGE_KEY) ?? "";
  } catch {
    return "";
  }
}

export function saveAccessToken(accessToken: string): void {
  try {
    const token = normalizeAccessToken(accessToken);
    if (token) {
      globalThis.localStorage?.setItem(ACCESS_TOKEN_STORAGE_KEY, token);
    } else {
      globalThis.localStorage?.removeItem(ACCESS_TOKEN_STORAGE_KEY);
    }
  } catch {
    // Storage can be unavailable in private or restricted browser contexts.
  }
}

function resolveAccessToken(api: ApiContext): string {
  if (api.accessToken !== undefined) {
    return normalizeAccessToken(api.accessToken);
  }
  return getStoredAccessToken();
}

function normalizeAccessToken(accessToken: string): string {
  return accessToken.trim();
}

function redactAccessToken(message: string, accessToken: string): string {
  return accessToken ? message.split(accessToken).join("[access token]") : message;
}

export function getHealth(api?: ApiContext): Promise<HealthResponse> {
  return requestJson<HealthResponse>("/health", undefined, api);
}

export function getSshTarget(api?: ApiContext): Promise<SshTarget> {
  return requestJson<SshTarget>("/api/ssh/target", undefined, api);
}

export function updateSshTarget(input: SshTargetUpdateRequest, api?: ApiContext): Promise<SshTarget> {
  return requestJson<SshTarget>(
    "/api/ssh/target",
    { method: "PUT", body: JSON.stringify(input) },
    api
  );
}

export function checkSshTarget(api?: ApiContext): Promise<SshCheckResponse> {
  return requestJson<SshCheckResponse>("/api/ssh/check", { method: "POST" }, api);
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

export function getDiagnosticPresets(api?: ApiContext): Promise<DiagnosticPreset[]> {
  return requestJson<DiagnosticPreset[]>("/api/diagnostics/presets", undefined, api);
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

export function sendAgentTurn(sessionId: string, message: string, api?: ApiContext): Promise<AgentTurnResponse> {
  const body: AgentTurnRequest = { message };

  return requestJson<AgentTurnResponse>(
    `/api/sessions/${encodeURIComponent(sessionId)}/agent-turn`,
    { method: "POST", body: JSON.stringify(body) },
    api
  );
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
