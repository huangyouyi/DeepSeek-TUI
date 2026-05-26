import { describe, expect, it } from "vitest";
import {
  buildTargetLabel,
  formatConnectionStatus,
  formatSessionTime,
  isConnectionHealthy
} from "./productStatus";

describe("productStatus", () => {
  it("formats connection status with server, SSE, service, and model", () => {
    expect(
      formatConnectionStatus({
        server: "connected",
        sse: "streaming",
        service: "deepseek",
        model: "deepseek-v4-flash"
      })
    ).toBe("Agent Server connected | SSE streaming | Service deepseek | Model deepseek-v4-flash");
  });

  it("uses fallback labels for missing service and model", () => {
    expect(
      formatConnectionStatus({
        server: "connected",
        sse: "streaming",
        service: null
      })
    ).toBe("Agent Server connected | SSE streaming | Service unknown | Model unknown");
  });

  it("detects healthy connection status", () => {
    expect(
      isConnectionHealthy({
        server: "connected",
        sse: "streaming",
        service: "deepseek",
        model: "deepseek-v4-pro"
      })
    ).toBe(true);
  });

  it("marks server errors as unhealthy", () => {
    expect(
      isConnectionHealthy({
        server: "error",
        sse: "streaming"
      })
    ).toBe(false);
  });

  it("marks SSE errors and disconnected status as unhealthy", () => {
    expect(isConnectionHealthy({ server: "connected", sse: "error" })).toBe(false);
    expect(isConnectionHealthy({ server: "connected", sse: "disconnected" })).toBe(false);
  });

  it("formats empty session activity", () => {
    expect(formatSessionTime(0)).toBe("No activity yet");
    expect(formatSessionTime(Number.NaN)).toBe("No activity yet");
    expect(formatSessionTime(-1)).toBe("No activity yet");
  });

  it("formats session timestamps with the product shell date format", () => {
    const value = Date.UTC(2026, 4, 26, 14, 30);

    expect(formatSessionTime(value)).toBe(
      new Intl.DateTimeFormat(undefined, {
        month: "short",
        day: "numeric",
        hour: "2-digit",
        minute: "2-digit"
      }).format(new Date(value))
    );
  });

  it("builds target labels", () => {
    expect(buildTargetLabel(null)).toBe("SSH target unknown");
    expect(buildTargetLabel(undefined)).toBe("SSH target unknown");
    expect(buildTargetLabel({ user: "root", host: "192.168.1.10", port: 22 })).toBe("root@192.168.1.10:22");
  });
});
