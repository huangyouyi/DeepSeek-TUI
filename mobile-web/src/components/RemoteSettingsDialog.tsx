import { useEffect, useState } from "react";
import type { CSSProperties, FormEvent } from "react";
import type { SshCheckResponse, SshTarget } from "../types";

export type RemoteSettingsDialogProps = {
  open: boolean;
  target?: SshTarget | null;
  status: {
    server: string;
    sse: string;
    service: string;
    model: string;
    targetLabel?: string;
  };
  sshCheck?: SshCheckResponse | null;
  busy?: boolean;
  onOpenChange: (open: boolean) => void;
  onSaveTarget: (input: { host: string; user: string; port: number }) => Promise<void>;
  onCheckSsh: () => Promise<void>;
};

type ValidationErrors = {
  host?: string;
  user?: string;
  port?: string;
};

export function RemoteSettingsDialog({
  open,
  target,
  status,
  sshCheck,
  busy = false,
  onOpenChange,
  onSaveTarget,
  onCheckSsh
}: RemoteSettingsDialogProps) {
  const [host, setHost] = useState("");
  const [user, setUser] = useState("");
  const [port, setPort] = useState("22");
  const [errors, setErrors] = useState<ValidationErrors>({});
  const [saving, setSaving] = useState(false);
  const [checking, setChecking] = useState(false);

  useEffect(() => {
    if (!open) {
      return;
    }

    setHost(target?.host ?? "");
    setUser(target?.user ?? "");
    setPort(String(target?.port ?? 22));
    setErrors({});
  }, [open, target]);

  if (!open) {
    return null;
  }

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    const nextErrors: ValidationErrors = {};
    const trimmedHost = host.trim();
    const trimmedUser = user.trim();
    const parsedPort = Number(port);

    if (!trimmedHost) {
      nextErrors.host = "Host is required.";
    }

    if (!trimmedUser) {
      nextErrors.user = "User is required.";
    }

    if (!Number.isInteger(parsedPort) || parsedPort < 1 || parsedPort > 65535) {
      nextErrors.port = "Port must be an integer from 1 to 65535.";
    }

    setErrors(nextErrors);

    if (Object.keys(nextErrors).length > 0) {
      return;
    }

    setSaving(true);
    try {
      await onSaveTarget({ host: trimmedHost, user: trimmedUser, port: parsedPort });
    } finally {
      setSaving(false);
    }
  }

  async function handleCheckSsh() {
    setChecking(true);
    try {
      await onCheckSsh();
    } finally {
      setChecking(false);
    }
  }

  const disabled = busy || saving || checking;

  return (
    <div style={styles.backdrop}>
      <section role="dialog" aria-label="Remote settings" aria-modal="true" style={styles.dialog}>
        <header style={styles.header}>
          <div>
            <p style={styles.eyebrow}>Remote SSH target</p>
            <h2 style={styles.title}>Settings</h2>
          </div>
          <button
            type="button"
            aria-label="Close remote settings"
            onClick={() => onOpenChange(false)}
            style={styles.iconButton}
          >
            x
          </button>
        </header>

        <div style={styles.statusGrid} aria-label="Connection status">
          <StatusItem label="Server" value={status.server} />
          <StatusItem label="SSE" value={status.sse} />
          <StatusItem label="Service" value={status.service} />
          <StatusItem label="Model" value={status.model} />
          <StatusItem label="Current target" value={status.targetLabel ?? "SSH target unknown"} />
        </div>

        <form onSubmit={handleSubmit} style={styles.form}>
          <label style={styles.field}>
            <span style={styles.label}>Host</span>
            <input
              aria-invalid={Boolean(errors.host)}
              value={host}
              onChange={(event) => setHost(event.currentTarget.value)}
              disabled={disabled}
              style={styles.input}
            />
            {errors.host ? <span style={styles.error}>{errors.host}</span> : null}
          </label>

          <label style={styles.field}>
            <span style={styles.label}>User</span>
            <input
              aria-invalid={Boolean(errors.user)}
              value={user}
              onChange={(event) => setUser(event.currentTarget.value)}
              disabled={disabled}
              style={styles.input}
            />
            {errors.user ? <span style={styles.error}>{errors.user}</span> : null}
          </label>

          <label style={styles.field}>
            <span style={styles.label}>Port</span>
            <input
              aria-invalid={Boolean(errors.port)}
              inputMode="numeric"
              value={port}
              onChange={(event) => setPort(event.currentTarget.value)}
              disabled={disabled}
              style={styles.input}
            />
            {errors.port ? <span style={styles.error}>{errors.port}</span> : null}
          </label>

          <div style={styles.actions}>
            <button type="button" onClick={handleCheckSsh} disabled={disabled} style={styles.secondaryButton}>
              Check SSH
            </button>
            <button type="submit" disabled={disabled} style={styles.primaryButton}>
              Save target
            </button>
          </div>
        </form>

        {sshCheck ? <SshCheckResult result={sshCheck} /> : null}
      </section>
    </div>
  );
}

function StatusItem({ label, value }: { label: string; value: string }) {
  return (
    <div style={styles.statusItem}>
      <span style={styles.statusLabel}>{label}</span>
      <span style={styles.statusValue}>{value}</span>
    </div>
  );
}

function SshCheckResult({ result }: { result: SshCheckResponse }) {
  return (
    <section aria-label="SSH check result" style={styles.checkResult}>
      <h3 style={styles.checkTitle}>SSH check</h3>
      <div style={styles.checkList}>
        <StatusItem label="Status" value={result.status} />
        {result.error_summary ? <StatusItem label="Error" value={result.error_summary} /> : null}
        {typeof result.exit_code === "number" ? <StatusItem label="Exit code" value={`Exit code ${result.exit_code}`} /> : null}
        {typeof result.duration_ms === "number" ? <StatusItem label="Duration" value={`${result.duration_ms} ms`} /> : null}
      </div>
    </section>
  );
}

const styles = {
  backdrop: {
    alignItems: "center",
    background: "rgba(15, 23, 42, 0.45)",
    display: "flex",
    inset: 0,
    justifyContent: "center",
    padding: 16,
    position: "fixed",
    zIndex: 30
  },
  dialog: {
    background: "#ffffff",
    border: "1px solid #d8dee9",
    borderRadius: 8,
    boxShadow: "0 24px 80px rgba(15, 23, 42, 0.24)",
    color: "#172033",
    maxWidth: 560,
    width: "100%"
  },
  header: {
    alignItems: "center",
    borderBottom: "1px solid #e6eaf2",
    display: "flex",
    justifyContent: "space-between",
    padding: "18px 20px"
  },
  eyebrow: {
    color: "#5b6475",
    fontSize: 12,
    fontWeight: 700,
    margin: "0 0 4px",
    textTransform: "uppercase" as const
  },
  title: {
    fontSize: 20,
    lineHeight: 1.2,
    margin: 0
  },
  iconButton: {
    background: "#f4f6f9",
    border: "1px solid #d8dee9",
    borderRadius: 6,
    color: "#253047",
    cursor: "pointer",
    fontSize: 16,
    height: 34,
    width: 34
  },
  statusGrid: {
    display: "grid",
    gap: 10,
    gridTemplateColumns: "repeat(auto-fit, minmax(150px, 1fr))",
    margin: 0,
    padding: 20
  },
  statusItem: {
    background: "#f8fafc",
    border: "1px solid #e6eaf2",
    borderRadius: 6,
    margin: 0,
    padding: 10
  },
  statusLabel: {
    color: "#687386",
    display: "block",
    fontSize: 12,
    fontWeight: 700,
    margin: "0 0 4px"
  },
  statusValue: {
    color: "#172033",
    display: "block",
    fontSize: 14,
    margin: 0,
    overflowWrap: "anywhere" as const
  },
  form: {
    borderTop: "1px solid #e6eaf2",
    display: "grid",
    gap: 14,
    padding: 20
  },
  field: {
    display: "grid",
    gap: 6
  },
  label: {
    color: "#253047",
    fontSize: 13,
    fontWeight: 700
  },
  input: {
    border: "1px solid #cfd7e6",
    borderRadius: 6,
    color: "#172033",
    fontSize: 16,
    padding: "10px 12px"
  },
  error: {
    color: "#b42318",
    fontSize: 13
  },
  actions: {
    display: "flex",
    gap: 10,
    justifyContent: "flex-end"
  },
  secondaryButton: {
    background: "#ffffff",
    border: "1px solid #cfd7e6",
    borderRadius: 6,
    color: "#253047",
    cursor: "pointer",
    fontWeight: 700,
    padding: "10px 14px"
  },
  primaryButton: {
    background: "#172033",
    border: "1px solid #172033",
    borderRadius: 6,
    color: "#ffffff",
    cursor: "pointer",
    fontWeight: 700,
    padding: "10px 14px"
  },
  checkResult: {
    borderTop: "1px solid #e6eaf2",
    padding: 20
  },
  checkTitle: {
    fontSize: 15,
    margin: "0 0 10px"
  },
  checkList: {
    display: "grid",
    gap: 10,
    gridTemplateColumns: "repeat(auto-fit, minmax(150px, 1fr))",
    margin: 0
  }
} satisfies Record<string, CSSProperties>;
