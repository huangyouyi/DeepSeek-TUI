import { FormEvent, useEffect, useMemo, useReducer, useState } from "react";
import {
  approveCommand,
  buildEventUrl,
  checkSshTarget,
  createSession,
  getDiagnosticPresets,
  getHealth,
  getRecentAudit,
  getStoredAccessToken,
  getSshTarget,
  prepareCommand,
  rejectCommand,
  runDiagnostic,
  saveAccessToken,
  sendAgentTurn,
  updateSshTarget
} from "./api";
import {
  buildFeedbackReport,
  fallbackDiagnosticPresets,
  initialAppState,
  reduceEvent,
  resolveDiagnosticPresets,
  withSseStatus
} from "./state";
import type { DiagnosticKey, DiagnosticPreset, ServerEvent, SshCheckResponse } from "./types";

type LocalAction =
  | { type: "event"; event: ServerEvent }
  | { type: "sse"; status: "connecting" | "connected" | "disconnected" | "error" }
  | { type: "server"; status: "ok" | "error" }
  | { type: "target"; target: NonNullable<typeof initialAppState.connection.target> }
  | { type: "audit"; entries: typeof initialAppState.audit };

function appReducer(state: typeof initialAppState, action: LocalAction): typeof initialAppState {
  switch (action.type) {
    case "event":
      return reduceEvent(state, action.event);
    case "sse":
      return withSseStatus(state, action.status);
    case "server":
      return { ...state, connection: { ...state.connection, server: action.status } };
    case "target":
      return { ...state, connection: { ...state.connection, target: action.target } };
    case "audit":
      return { ...state, audit: action.entries };
    default:
      return state;
  }
}

export default function App() {
  const [state, dispatch] = useReducer(appReducer, initialAppState);
  const [sessionId, setSessionId] = useState<string>("");
  const [agentMessage, setAgentMessage] = useState("");
  const [command, setCommand] = useState("");
  const [cwd, setCwd] = useState("");
  const [targetForm, setTargetForm] = useState({ host: "", user: "", port: "22" });
  const [accessTokenInput, setAccessTokenInput] = useState(() => getStoredAccessToken());
  const [accessToken, setAccessToken] = useState(() => getStoredAccessToken());
  const [diagnostics, setDiagnostics] = useState<DiagnosticPreset[]>(fallbackDiagnosticPresets);
  const [sshCheck, setSshCheck] = useState<SshCheckResponse | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [copyStatus, setCopyStatus] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function load() {
      try {
        await getHealth();
        if (!cancelled) {
          dispatch({ type: "server", status: "ok" });
        }
      } catch (err) {
        if (!cancelled) {
          dispatch({ type: "server", status: "error" });
          setError(messageFromError(err));
        }
      }

      try {
        const presets = await getDiagnosticPresets();
        if (!cancelled) {
          setDiagnostics(resolveDiagnosticPresets(presets));
        }
      } catch {
        if (!cancelled) {
          setDiagnostics(fallbackDiagnosticPresets);
        }
      }

      try {
        const [target, audit] = await Promise.all([getSshTarget(), getRecentAudit()]);
        if (!cancelled) {
          dispatch({ type: "target", target });
          dispatch({ type: "audit", entries: audit });
        }
      } catch (err) {
        if (!cancelled) {
          setError(messageFromError(err));
        }
      }

      try {
        const session = await createSession();
        if (!cancelled) {
          setSessionId(session.id);
          dispatch({ type: "event", event: { type: "session.updated", payload: session } });
        }
      } catch (err) {
        if (!cancelled) {
          setError(messageFromError(err));
        }
      }
    }

    load();

    return () => {
      cancelled = true;
    };
  }, [accessToken]);

  useEffect(() => {
    const target = state.connection.target;
    if (target) {
      setTargetForm({ host: target.host, user: target.user, port: String(target.port) });
    }
  }, [state.connection.target]);

  useEffect(() => {
    dispatch({ type: "sse", status: "connecting" });
    const source = new EventSource(buildEventUrl(accessToken));
    const eventTypes: ServerEvent["type"][] = [
      "session.updated",
      "message.updated",
      "message.part.updated",
      "tool.started",
      "tool.stdout",
      "tool.stderr",
      "tool.completed",
      "tool.failed",
      "approval.asked",
      "approval.replied",
      "audit.updated",
      "connection.updated"
    ];
    const handleNamedEvent = (message: MessageEvent<string>) => {
      try {
        dispatch({ type: "event", event: JSON.parse(message.data) as ServerEvent });
      } catch {
        dispatch({
          type: "event",
          event: { type: "connection.updated", payload: { status: "ignored malformed event" } }
        });
      }
    };

    source.onopen = () => dispatch({ type: "sse", status: "connected" });
    source.onerror = () => dispatch({ type: "sse", status: "error" });
    source.onmessage = handleNamedEvent;
    eventTypes.forEach((type) => source.addEventListener(type, handleNamedEvent));

    return () => {
      eventTypes.forEach((type) => source.removeEventListener(type, handleNamedEvent));
      source.close();
      dispatch({ type: "sse", status: "disconnected" });
    };
  }, [accessToken]);

  function handleAccessTokenSave(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const token = accessTokenInput.trim();
    saveAccessToken(token);
    setAccessTokenInput(token);
    setAccessToken(token);
    setError(null);
  }

  const targetLabel = useMemo(() => {
    const target = state.connection.target;
    if (!target) {
      return "SSH target unknown";
    }
    return `${target.user}@${target.host}:${target.port}`;
  }, [state.connection.target]);

  const busyMessage = useMemo(() => {
    if (!busy) {
      return null;
    }
    if (busy === "command") {
      return "Preparing approval";
    }
    if (busy === "agent") {
      return "Waiting for Agent response";
    }
    if (busy === "target") {
      return "Saving target";
    }
    if (busy === "ssh-check") {
      return "Checking SSH target";
    }
    const diagnostic = diagnostics.find((item) => item.key === busy);
    if (diagnostic) {
      return `Running ${diagnostic.label}`;
    }
    const approval = state.pendingApprovals.find((item) => item.id === busy);
    return approval ? "Sending approval response" : "Working";
  }, [busy, diagnostics, state.pendingApprovals]);

  const feedbackReport = useMemo(() => buildFeedbackReport(state, sshCheck), [state, sshCheck]);

  async function handleTargetSave(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    const host = targetForm.host.trim();
    const user = targetForm.user.trim();
    const port = Number(targetForm.port);

    if (!host || !user || !Number.isInteger(port) || port < 1 || port > 65535) {
      setError("Enter a host, user, and port from 1 to 65535.");
      return;
    }

    setBusy("target");
    setError(null);
    try {
      const target = await updateSshTarget({ host, user, port });
      dispatch({ type: "target", target });
      setSshCheck(null);
      dispatch({
        type: "event",
        event: { type: "connection.updated", payload: { status: `target ${target.user}@${target.host}:${target.port}` } }
      });
    } catch (err) {
      setError(messageFromError(err));
    } finally {
      setBusy(null);
    }
  }

  async function handleSshCheck() {
    setBusy("ssh-check");
    setError(null);
    try {
      const result = await checkSshTarget();
      setSshCheck(result);
    } catch (err) {
      setError(messageFromError(err));
    } finally {
      setBusy(null);
    }
  }

  async function handleDiagnostic(key: DiagnosticKey) {
    const activeSessionId = sessionId || state.activeSessionId;
    if (!activeSessionId) {
      setError("Session is not ready yet.");
      return;
    }

    setBusy(key);
    setError(null);
    try {
      const response = await runDiagnostic({ sessionId: activeSessionId, diagnostic: key });
      dispatch({
        type: "event",
        event: {
          type: "tool.completed",
          payload: { command: key, text: JSON.stringify(response.result), status: response.status }
        }
      });
    } catch (err) {
      setError(messageFromError(err));
    } finally {
      setBusy(null);
    }
  }

  async function handleCommand(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const activeSessionId = sessionId || state.activeSessionId;
    const trimmed = command.trim();

    if (!activeSessionId || !trimmed) {
      return;
    }

    setBusy("command");
    setError(null);
    try {
      const approval = await prepareCommand({
        sessionId: activeSessionId,
        command: trimmed,
        cwd: cwd.trim() || undefined
      });
      dispatch({ type: "event", event: { type: "approval.asked", payload: approval } });
      setCommand("");
    } catch (err) {
      setError(messageFromError(err));
    } finally {
      setBusy(null);
    }
  }

  async function handleAgentTurn(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const activeSessionId = sessionId || state.activeSessionId;
    const trimmed = agentMessage.trim();

    if (!activeSessionId || !trimmed || busy === "agent") {
      return;
    }

    setBusy("agent");
    setError(null);
    dispatch({ type: "event", event: { type: "message.updated", payload: { role: "user", text: trimmed } } });
    try {
      const response = await sendAgentTurn(activeSessionId, trimmed);
      if (response.assistant_text.trim()) {
        dispatch({
          type: "event",
          event: { type: "message.updated", payload: { role: "assistant", text: response.assistant_text } }
        });
      }
      response.pending_approvals.forEach((approval) => {
        dispatch({ type: "event", event: { type: "approval.asked", payload: approval } });
      });
      setAgentMessage("");
    } catch (err) {
      setError(messageFromError(err));
    } finally {
      setBusy(null);
    }
  }

  async function handleApproval(id: string, response: "approve_once" | "reject") {
    setBusy(id);
    setError(null);
    try {
      const result = response === "approve_once" ? await approveCommand(id) : await rejectCommand(id);
      dispatch({
        type: "event",
        event: {
          type: "approval.replied",
          payload: { id, status: result.status }
        }
      });
    } catch (err) {
      setError(messageFromError(err));
    } finally {
      setBusy(null);
    }
  }

  async function handleCopyReport() {
    try {
      await navigator.clipboard.writeText(feedbackReport);
      setCopyStatus("Report copied");
      setError(null);
    } catch {
      setCopyStatus("Copy failed");
      setError("Clipboard copy failed. Select the report text and copy it manually.");
    }
  }

  return (
    <main className="app-shell">
      <header className="connection-header">
        <div>
          <p className="eyebrow">DeepSeek Mobile SSH</p>
          <h1>Linux Web Control</h1>
        </div>
        <div className="status-grid" aria-label="Connection status">
          <StatusPill label="Server" value={state.connection.server} />
          <StatusPill label="SSE" value={state.connection.sse} />
          <StatusPill label="Target" value={targetLabel} />
        </div>
        <form className="access-token-form" onSubmit={handleAccessTokenSave}>
          <label>
            Access token
            <input
              autoCapitalize="none"
              autoComplete="off"
              autoCorrect="off"
              onChange={(event) => setAccessTokenInput(event.target.value)}
              placeholder="optional"
              spellCheck={false}
              type="password"
              value={accessTokenInput}
            />
          </label>
          <button className="secondary-button" type="submit">
            Save token
          </button>
        </form>
        <form className="target-form" onSubmit={handleTargetSave}>
          <label>
            Host
            <input
              autoCapitalize="none"
              autoComplete="off"
              autoCorrect="off"
              onChange={(event) => setTargetForm((current) => ({ ...current, host: event.target.value }))}
              placeholder="192.168.30.244"
              spellCheck={false}
              type="text"
              value={targetForm.host}
            />
          </label>
          <label>
            User
            <input
              autoCapitalize="none"
              autoComplete="username"
              autoCorrect="off"
              onChange={(event) => setTargetForm((current) => ({ ...current, user: event.target.value }))}
              placeholder="root"
              spellCheck={false}
              type="text"
              value={targetForm.user}
            />
          </label>
          <label>
            Port
            <input
              inputMode="numeric"
              max="65535"
              min="1"
              onChange={(event) => setTargetForm((current) => ({ ...current, port: event.target.value }))}
              type="number"
              value={targetForm.port}
            />
          </label>
          <button className="secondary-button" disabled={busy !== null} type="submit">
            {busy === "target" ? "Saving" : "Save target"}
          </button>
        </form>
        <div className="ssh-check-row">
          <button className="secondary-button" disabled={busy !== null} onClick={handleSshCheck} type="button">
            {busy === "ssh-check" ? "Checking" : "Check SSH"}
          </button>
          {sshCheck ? (
            <div className={`ssh-check-result ${sshCheck.status}`} aria-live="polite">
              <strong>{sshCheck.status}</strong>
              <span>{formatSshCheckResult(sshCheck)}</span>
            </div>
          ) : (
            <div className="ssh-check-result idle" aria-live="polite">
              <strong>not checked</strong>
              <span>Run a target reachability check.</span>
            </div>
          )}
        </div>
      </header>

      {busyMessage ? (
        <div className="busy-banner" aria-live="polite">
          {busyMessage}
        </div>
      ) : null}

      {error ? <div className="error-banner">{error}</div> : null}

      <section className="panel agent-panel" aria-labelledby="agent-heading">
        <div className="section-heading">
          <h2 id="agent-heading">Agent Chat</h2>
          <span>remote Linux assistant</span>
        </div>
        <form className="agent-form" onSubmit={handleAgentTurn}>
          <label>
            Message
            <textarea
              autoCapitalize="sentences"
              autoComplete="off"
              autoCorrect="on"
              disabled={busy === "agent"}
              onChange={(event) => setAgentMessage(event.target.value)}
              placeholder="Ask the remote Linux device..."
              rows={3}
              value={agentMessage}
            />
          </label>
          <button className="primary-button agent-send-button" disabled={busy === "agent" || !agentMessage.trim()} type="submit">
            {busy === "agent" ? "Sending" : "Send"}
          </button>
        </form>
      </section>

      <section className="panel approval-panel" aria-labelledby="approval-heading">
        <div className="section-heading">
          <h2 id="approval-heading">Pending Approvals</h2>
          <span>{state.pendingApprovals.length}</span>
        </div>
        {state.pendingApprovals.length === 0 ? (
          <p className="empty-state">No commands awaiting approval.</p>
        ) : (
          <div className="approval-list">
            {state.pendingApprovals.map((approval) => (
              <article className="approval-card" key={approval.id}>
                <div>
                  <strong>{approval.status}</strong>
                  {approval.cwd ? <span>{approval.cwd}</span> : null}
                </div>
                <pre>{approval.command}</pre>
                <div className="approval-actions">
                  <button
                    className="primary-button"
                    disabled={busy !== null}
                    onClick={() => handleApproval(approval.id, "approve_once")}
                    type="button"
                  >
                    {busy === approval.id ? "Sending" : "Approve once"}
                  </button>
                  <button
                    className="danger-button"
                    disabled={busy !== null}
                    onClick={() => handleApproval(approval.id, "reject")}
                    type="button"
                  >
                    Reject
                  </button>
                </div>
              </article>
            ))}
          </div>
        )}
      </section>

      <section className="panel diagnostics-panel secondary-panel" aria-labelledby="diagnostics-heading">
        <div className="section-heading">
          <h2 id="diagnostics-heading">Diagnostics</h2>
          <span>{diagnostics.length} presets</span>
        </div>
        <div className="diagnostic-grid">
          {diagnostics.map((diagnostic) => (
            <button
              className="diagnostic-button"
              disabled={busy !== null}
              key={diagnostic.key}
              onClick={() => handleDiagnostic(diagnostic.key)}
              type="button"
            >
              <span>{diagnostic.label}</span>
              <code>{busy === diagnostic.key ? "Running..." : diagnostic.command}</code>
            </button>
          ))}
        </div>
      </section>

      <section className="panel command-panel secondary-panel" aria-labelledby="command-heading">
        <div className="section-heading">
          <h2 id="command-heading">Advanced Command</h2>
          <span>approval required</span>
        </div>
        <form onSubmit={handleCommand}>
          <label>
            Command
            <textarea
              autoCapitalize="none"
              autoComplete="off"
              autoCorrect="off"
              onChange={(event) => setCommand(event.target.value)}
              placeholder="journalctl -u ssh --no-pager | tail -80"
              rows={4}
              spellCheck={false}
              value={command}
            />
          </label>
          <label>
            Working directory
            <input
              autoCapitalize="none"
              autoComplete="off"
              autoCorrect="off"
              onChange={(event) => setCwd(event.target.value)}
              placeholder="/root"
              spellCheck={false}
              type="text"
              value={cwd}
            />
          </label>
          <button className="primary-button" disabled={busy !== null || !command.trim()} type="submit">
            {busy === "command" ? "Preparing" : "Request approval"}
          </button>
        </form>
      </section>

      <section className="panel timeline-panel" aria-labelledby="timeline-heading">
        <div className="section-heading">
          <h2 id="timeline-heading">Timeline</h2>
          <div className="section-actions">
            <span>{state.timeline.length}</span>
            <button className="secondary-button compact-button" onClick={handleCopyReport} type="button">
              Copy report
            </button>
          </div>
        </div>
        {copyStatus ? (
          <p className={`copy-status ${copyStatus === "Copy failed" ? "copy-status-error" : ""}`} aria-live="polite">
            {copyStatus}
          </p>
        ) : null}
        <label className="report-copy-field">
          Copyable report
          <textarea readOnly rows={8} value={feedbackReport} />
        </label>
        <div className="timeline-list">
          {state.timeline.length === 0 ? (
            <p className="empty-state">Waiting for server events.</p>
          ) : (
            state.timeline.map((entry) => (
              <article className={`timeline-entry ${entry.kind}`} key={entry.id}>
                <div>
                  <strong>{entry.title}</strong>
                  <time>{new Date(entry.createdAtMs).toLocaleTimeString()}</time>
                </div>
                <pre>{entry.text}</pre>
              </article>
            ))
          )}
        </div>
      </section>

      <details className="panel audit-panel" open>
        <summary>
          <span>Recent Audit</span>
          <span>{state.audit.length}</span>
        </summary>
        <div className="audit-list">
          {state.audit.length === 0 ? (
            <p className="empty-state">No audit entries yet.</p>
          ) : (
            state.audit.map((entry) => (
              <article className="audit-entry" key={entry.id}>
                <strong>{entry.kind}</strong>
                <p>{entry.summary}</p>
                <time>{new Date(entry.created_at_ms).toLocaleString()}</time>
              </article>
            ))
          )}
        </div>
      </details>
    </main>
  );
}

function StatusPill({ label, value }: { label: string; value: string }) {
  return (
    <div className="status-pill">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function messageFromError(error: unknown): string {
  return error instanceof Error ? error.message : "Unexpected request failure";
}

function formatSshCheckResult(result: SshCheckResponse): string {
  const parts = [`${result.target.user}@${result.target.host}:${result.target.port}`];
  if (result.exit_code !== undefined) {
    parts.push(`exit ${result.exit_code}`);
  }
  if (result.duration_ms !== undefined) {
    parts.push(`${result.duration_ms} ms`);
  }
  if (result.error_summary) {
    parts.push(result.error_summary);
  }
  return parts.join(" | ");
}
