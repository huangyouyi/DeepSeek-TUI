export type ProductConnectionInput = {
  server: string;
  sse: string;
  service?: string | null;
  model?: string | null;
  targetLabel?: string | null;
};

export function formatConnectionStatus(input: ProductConnectionInput): string {
  const serviceLabel = input.service?.trim() || "unknown";
  const modelLabel = input.model?.trim() || "unknown";

  return `Agent Server ${input.server} | SSE ${input.sse} | Service ${serviceLabel} | Model ${modelLabel}`;
}

export function isConnectionHealthy(input: ProductConnectionInput): boolean {
  const server = input.server.toLowerCase();
  const sse = input.sse.toLowerCase();

  return server !== "error" && sse !== "error" && sse !== "disconnected";
}

export function formatSessionTime(value: number): string {
  if (!Number.isFinite(value) || value <= 0) {
    return "No activity yet";
  }

  return new Intl.DateTimeFormat(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit"
  }).format(new Date(value));
}

export function buildTargetLabel(target?: { host: string; user: string; port: number } | null): string {
  if (!target) {
    return "SSH target unknown";
  }

  return `${target.user}@${target.host}:${target.port}`;
}
