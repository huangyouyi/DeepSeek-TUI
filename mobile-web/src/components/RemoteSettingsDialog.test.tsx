import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { RemoteSettingsDialog } from "./RemoteSettingsDialog";
import type { SshCheckResponse, SshTarget } from "../types";

const target: SshTarget = {
  host: "192.168.30.244",
  user: "root",
  port: 22,
  key_present: true
};

const status = {
  server: "online",
  sse: "connected",
  service: "deepseek",
  model: "deepseek-v4-flash",
  targetLabel: "root@192.168.30.244:22"
};

const baseProps = {
  open: true,
  target,
  status,
  onOpenChange: vi.fn(),
  onSaveTarget: vi.fn().mockResolvedValue(undefined),
  onCheckSsh: vi.fn().mockResolvedValue(undefined)
};

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("RemoteSettingsDialog", () => {
  it("renders nothing when closed", () => {
    const { container } = render(<RemoteSettingsDialog {...baseProps} open={false} />);

    expect(container.firstChild).toBeNull();
  });

  it("renders fields and connection status when open", () => {
    render(<RemoteSettingsDialog {...baseProps} />);

    expect(screen.getByRole("dialog", { name: "Remote settings" })).toBeTruthy();
    expect(screen.getByLabelText("Host")).toHaveProperty("value", "192.168.30.244");
    expect(screen.getByLabelText("User")).toHaveProperty("value", "root");
    expect(screen.getByLabelText("Port")).toHaveProperty("value", "22");
    expect(screen.getByText("Server")).toBeTruthy();
    expect(screen.getByText("online")).toBeTruthy();
    expect(screen.getByText("SSE")).toBeTruthy();
    expect(screen.getByText("connected")).toBeTruthy();
    expect(screen.getByText("Service")).toBeTruthy();
    expect(screen.getByText("deepseek")).toBeTruthy();
    expect(screen.getByText("Model")).toBeTruthy();
    expect(screen.getByText("deepseek-v4-flash")).toBeTruthy();
    expect(screen.getByText("Current target")).toBeTruthy();
    expect(screen.getByText("root@192.168.30.244:22")).toBeTruthy();
  });

  it("calls onOpenChange(false) when closed", () => {
    render(<RemoteSettingsDialog {...baseProps} />);

    fireEvent.click(screen.getByRole("button", { name: "Close remote settings" }));

    expect(baseProps.onOpenChange).toHaveBeenCalledWith(false);
  });

  it("moves focus into the modal, traps tab focus, and closes on Escape", () => {
    const opener = document.createElement("button");
    document.body.appendChild(opener);
    opener.focus();
    const onOpenChange = vi.fn();

    render(<RemoteSettingsDialog {...baseProps} onOpenChange={onOpenChange} />);

    const closeButton = screen.getByRole("button", { name: "Close remote settings" });
    expect(document.activeElement).toBe(closeButton);

    const saveButton = screen.getByRole("button", { name: "Save target" });
    saveButton.focus();
    fireEvent.keyDown(screen.getByRole("dialog", { name: "Remote settings" }), { key: "Tab" });
    expect(document.activeElement).toBe(closeButton);

    fireEvent.keyDown(screen.getByRole("dialog", { name: "Remote settings" }), { key: "Escape" });
    expect(onOpenChange).toHaveBeenCalledWith(false);
    opener.remove();
  });

  it("parses port number before saving a valid target", async () => {
    const onSaveTarget = vi.fn().mockResolvedValue(undefined);
    render(<RemoteSettingsDialog {...baseProps} onSaveTarget={onSaveTarget} />);

    fireEvent.change(screen.getByLabelText("Host"), { target: { value: "router.local" } });
    fireEvent.change(screen.getByLabelText("User"), { target: { value: "admin" } });
    fireEvent.change(screen.getByLabelText("Port"), { target: { value: "2222" } });
    fireEvent.click(screen.getByRole("button", { name: "Save target" }));

    await waitFor(() => {
      expect(onSaveTarget).toHaveBeenCalledWith({ host: "router.local", user: "admin", port: 2222 });
    });
  });

  it("blocks save for empty host and invalid port and shows validation", () => {
    const onSaveTarget = vi.fn().mockResolvedValue(undefined);
    render(<RemoteSettingsDialog {...baseProps} onSaveTarget={onSaveTarget} />);

    fireEvent.change(screen.getByLabelText("Host"), { target: { value: "   " } });
    fireEvent.change(screen.getByLabelText("Port"), { target: { value: "70000" } });
    fireEvent.click(screen.getByRole("button", { name: "Save target" }));

    expect(onSaveTarget).not.toHaveBeenCalled();
    const hostError = screen.getByText("Host is required.");
    const portError = screen.getByText("Port must be an integer from 1 to 65535.");
    expect(hostError).toBeTruthy();
    expect(portError).toBeTruthy();
    expect(screen.getByLabelText("Host").getAttribute("aria-describedby")).toBe(hostError.id);
    expect(screen.getByLabelText("Port").getAttribute("aria-describedby")).toBe(portError.id);
  });

  it("calls onCheckSsh from the check button", async () => {
    const onCheckSsh = vi.fn().mockResolvedValue(undefined);
    render(<RemoteSettingsDialog {...baseProps} onCheckSsh={onCheckSsh} />);

    fireEvent.click(screen.getByRole("button", { name: "Check SSH" }));

    await waitFor(() => {
      expect(onCheckSsh).toHaveBeenCalledTimes(1);
    });
  });

  it("displays ssh check result details", () => {
    const sshCheck: SshCheckResponse = {
      status: "unreachable",
      target,
      check_id: "check-1",
      command: "ssh -o BatchMode=yes root@192.168.30.244 true",
      requires_approval: false,
      exit_code: 255,
      error_summary: "Connection refused",
      duration_ms: 1234,
      timed_out: false
    };

    render(<RemoteSettingsDialog {...baseProps} sshCheck={sshCheck} />);

    expect(screen.getByText("SSH check")).toBeTruthy();
    expect(screen.getByText("unreachable")).toBeTruthy();
    expect(screen.getByText("Connection refused")).toBeTruthy();
    expect(screen.getByText("Exit code 255")).toBeTruthy();
    expect(screen.getByText("1234 ms")).toBeTruthy();
  });
});
