import { FormEvent, useEffect, useMemo, useReducer, useState } from "react";
import { ChatView } from "./components/ChatView";
import { InlinePermissionMessage } from "./components/InlinePermissionMessage";
import { OpencodeChatHeader } from "./components/OpencodeChatHeader";
import { OpencodeChatInput } from "./components/OpencodeChatInput";
import { OpencodeChatMessage } from "./components/OpencodeChatMessage";
import { OpencodeSidebar } from "./components/OpencodeSidebar";
import { OpencodeWelcomePage } from "./components/OpencodeWelcomePage";
import { RemoteSettingsDialog } from "./components/RemoteSettingsDialog";
import { ToolActivity } from "./components/ToolActivity";
import {
  approveCommand,
  approveCommandForSession,
  buildEventUrl,
  checkSshTarget,
  createSession,
  getDiagnosticPresets,
  getHealth,
  getRecentAudit,
  getStoredAccessToken,
  getSshTarget,
  prepareCommand,
  rejectStopCommand,
  runDiagnostic,
  saveAccessToken,
  sendAgentTurn,
  stopAgentTurn,
  updateSshTarget
} from "./api";
import {
  buildConversationTitleHint,
  buildProductTimeline,
  mapSessionToConversation,
  type SessionMessageLike,
  type ToolPartLike
} from "./opencodeAdapter";
import { buildTargetLabel, formatConnectionStatus } from "./productStatus";
import {
  buildFeedbackReport,
  buildFinalAnswerReport,
  fallbackDiagnosticPresets,
  type ChatItem,
  initialAppState,
  reduceEvent,
  resolveDiagnosticPresets,
  selectChatItems,
  selectExecutionStatus,
  selectFinalAnswer,
  selectToolActivities,
  type ToolActivity as ToolActivityModel,
  withSseStatus
} from "./state";
import type { AgentTurnMode, AgentTurnResponse, ApprovalAction, DiagnosticKey, DiagnosticPreset, HealthResponse, Message, PendingApproval, ServerEvent, SshCheckResponse } from "./types";
import { useProductSessions } from "./useProductSessions";

const CONTINUATION_PROMPT = "请继续上一轮任务。";

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
  const [health, setHealth] = useState<HealthResponse | null>(null);
  const [sshCheck, setSshCheck] = useState<SshCheckResponse | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [copyStatus, setCopyStatus] = useState<string | null>(null);
  const [productSettingsOpen, setProductSettingsOpen] = useState(false);
  const [isProductSidebarOpen, setIsProductSidebarOpen] = useState(true);
  const [productLocalChatItems, setProductLocalChatItems] = useState<Record<string, ChatItem[]>>({});
  const [productLocalToolActivities, setProductLocalToolActivities] = useState<Record<string, ToolActivityModel[]>>({});
  const [latestAgentTurnId, setLatestAgentTurnId] = useState<string>("");
  const route = globalThis.location?.pathname ?? "/web";
  const isDebugRoute = route.startsWith("/debug");
  const productSessions = useProductSessions();

  useEffect(() => {
    if (route === "/") {
      window.history.replaceState({}, "", "/web");
    }
  }, [route]);

  useEffect(() => {
    if (isDebugRoute || !error) {
      return;
    }

    const timeout = window.setTimeout(() => setError(null), 4200);
    return () => window.clearTimeout(timeout);
  }, [error, isDebugRoute]);

  useEffect(() => {
    let cancelled = false;

    async function load() {
      try {
        const nextHealth = await getHealth();
        if (!cancelled) {
          setHealth(nextHealth);
          dispatch({ type: "server", status: "ok" });
        }
      } catch (err) {
        if (!cancelled) {
          setHealth(null);
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

      if (isDebugRoute) {
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
    }

    load();

    return () => {
      cancelled = true;
    };
  }, [accessToken, isDebugRoute]);

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
      "session.deleted",
      "message.updated",
      "message.part.updated",
      "assistant.started",
      "tool.started",
      "tool.stdout",
      "tool.stderr",
      "tool.completed",
      "tool.failed",
      "agent.tool.completed",
      "agent.tool.failed",
      "approval.asked",
      "approval.replied",
      "audit.updated",
      "connection.updated"
    ];
    const handleNamedEvent = (message: MessageEvent<string>) => {
      try {
        const event = JSON.parse(message.data) as ServerEvent;
        const turnId = turnIdFromEvent(event);
        if (turnId) {
          setLatestAgentTurnId(turnId);
        }
        dispatch({ type: "event", event });
        if (event.type === "session.updated" || event.type === "session.deleted") {
          void productSessions.reloadSessions().catch((err) => setError(messageFromError(err)));
        }
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
  }, [accessToken, productSessions.reloadSessions]);

  function handleAccessTokenSave(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const token = accessTokenInput.trim();
    saveAccessToken(token);
    setAccessTokenInput(token);
    setAccessToken(token);
    setError(null);
  }

  const targetLabel = useMemo(() => buildTargetLabel(state.connection.target), [state.connection.target]);
  const serviceLabel = health?.service || "unknown";
  const modelLabel = health?.model || "unknown";
  const executionStatus = useMemo(() => selectExecutionStatus(state, busy), [state, busy]);
  const productConnectionStatus = formatConnectionStatus({
    server: state.connection.server,
    sse: state.connection.sse,
    service: serviceLabel,
    model: modelLabel,
    targetLabel
  });

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
  const finalAnswerReport = useMemo(() => buildFinalAnswerReport(state), [state]);
  const chatItems = useMemo(() => selectChatItems(state), [state]);
  const finalAnswer = useMemo(() => selectFinalAnswer(state), [state]);
  const toolActivities = useMemo(() => selectToolActivities(state), [state]);
  const hasRunningTool = toolActivities.some((activity) => ["running", "started", "updated", "pending", "queued", "in_progress"].includes(activity.status.toLowerCase()));
  const canStopAgentTurn = Boolean(latestAgentTurnId) && (busy === "agent" || state.pendingApprovals.length > 0 || hasRunningTool);

  async function saveTarget(input: { host: string; user: string; port: number }) {
    const { host, user, port } = input;
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

  async function handleTargetSave(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    await saveTarget({
      host: targetForm.host.trim(),
      user: targetForm.user.trim(),
      port: Number(targetForm.port)
    });
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

  async function handleAgentTurn(event?: FormEvent<HTMLFormElement>, mode: AgentTurnMode = "normal") {
    event?.preventDefault();
    const activeSessionId = sessionId || state.activeSessionId;
    const trimmed = agentMessage.trim() || (mode === "continue" ? CONTINUATION_PROMPT : "");

    if (!activeSessionId || (!trimmed && mode === "normal") || busy === "agent") {
      return;
    }

    setBusy("agent");
    setError(null);
    if (trimmed) {
      dispatch({ type: "event", event: { type: "message.updated", payload: { role: "user", text: trimmed } } });
    }
    try {
      const response = await sendAgentTurn(
        activeSessionId,
        trimmed,
        mode === "normal" ? undefined : { mode, retryTurnId: mode === "retry" ? latestAgentTurnId : undefined }
      );
      setLatestAgentTurnId(latestTurnIdFromResponse(response) || latestAgentTurnId);
      response.executed_tools.forEach((tool, index) => {
        dispatch({
          type: "event",
          event: {
            type: tool.status === "failed" ? "tool.failed" : "tool.completed",
            payload: {
              id: `${response.turn_id}:tool:${index}`,
              command: tool.command,
              status: tool.status,
              exit_code: tool.exit_code,
              requires_approval: tool.requires_approval
            }
          }
        });
      });
      if (response.assistant_text.trim()) {
        dispatch({
          type: "event",
          event: {
            type: "message.updated",
            payload: { role: "assistant", status: response.status, text: response.assistant_text }
          }
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

  async function handleProductSend(content: string, mode: AgentTurnMode = "normal") {
    const trimmed = content.trim() || (mode === "continue" ? CONTINUATION_PROMPT : "");
    if ((!trimmed && mode === "normal") || busy === "agent") {
      return;
    }

    setBusy("agent");
    setError(null);

    let activeSessionId = productSessions.activeSessionId;
    try {
      if (!activeSessionId) {
        const created = await productSessions.createConversation(buildConversationTitleHint(trimmed));
        activeSessionId = created.id;
        dispatch({ type: "event", event: { type: "session.updated", payload: created } });
      }

      if (trimmed) {
        appendProductLocalChatItem(activeSessionId, {
          id: `local:${activeSessionId}:user:${Date.now()}`,
          role: "user",
          text: trimmed,
          createdAtMs: Date.now()
        });
      }

      const response = await sendAgentTurn(
        activeSessionId,
        trimmed,
        mode === "normal" ? undefined : { mode, retryTurnId: mode === "retry" ? latestAgentTurnId : undefined }
      );
      const nextTurnId = latestTurnIdFromResponse(response);
      if (nextTurnId) {
        setLatestAgentTurnId(nextTurnId);
      }
      appendProductLocalToolActivities(
        activeSessionId,
        response.executed_tools.map((tool, index) => ({
          id: `${response.turn_id}:tool:${index}`,
          command: tool.command,
          status: tool.status,
          exitCode: tool.exit_code,
          requiresApproval: tool.requires_approval,
          createdAtMs: Date.now() + index
        }))
      );
      if (response.assistant_text.trim()) {
        appendProductLocalChatItem(activeSessionId, {
          id: `local:${activeSessionId}:assistant:${Date.now()}`,
          role: "assistant",
          status: response.status,
          text: response.assistant_text,
          createdAtMs: Date.now(),
          isFinal: response.status !== "awaiting_approval"
        });
      }
      response.pending_approvals.forEach((approval) => {
        dispatch({ type: "event", event: { type: "approval.asked", payload: approval } });
      });
    } catch (err) {
      setError(messageFromError(err));
    } finally {
      setBusy(null);
    }
  }

  async function handleApproval(id: string, response: ApprovalAction) {
    setBusy(id);
    setError(null);
    try {
      const result =
        response === "approve_once"
          ? await approveCommand(id)
          : response === "approve_session"
            ? await approveCommandForSession(id)
            : await rejectStopCommand(id);
      dispatch({
        type: "event",
        event: {
          type: "approval.replied",
          payload: { id, status: result.status }
        }
      });
      const summary = approvalResponseSummary(result.result);
      if (summary) {
        dispatch({
          type: "event",
          event: { type: "message.updated", payload: { role: "assistant", text: summary } }
        });
      }
    } catch (err) {
      setError(messageFromError(err));
    } finally {
      setBusy(null);
    }
  }

  async function handleStopAgentTurn() {
    const activeSessionId = isDebugRoute ? sessionId || state.activeSessionId : productSessions.activeSessionId;
    if (!activeSessionId || !latestAgentTurnId) {
      return;
    }

    setBusy("agent-control");
    setError(null);
    try {
      await stopAgentTurn(activeSessionId, latestAgentTurnId);
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

  async function handleCopyFullReport() {
    await handleCopyReport();
  }

  async function handleCopyFinalAnswer() {
    if (!finalAnswerReport.trim()) {
      setCopyStatus("No final answer");
      setError("No final answer is available to copy yet.");
      return;
    }

    try {
      await navigator.clipboard.writeText(finalAnswerReport);
      setCopyStatus("Final answer copied");
      setError(null);
    } catch {
      setCopyStatus("Copy failed");
      setError("Clipboard copy failed. Select the final answer text and copy it manually.");
    }
  }

  async function handleNewProductConversation() {
    setError(null);
    try {
      const session = await productSessions.createConversation();
      dispatch({ type: "event", event: { type: "session.updated", payload: session } });
    } catch (err) {
      setError(messageFromError(err));
    }
  }

  function handleSelectProductConversation(selectedSessionId: string) {
    void productSessions.selectSession(selectedSessionId).catch((err) => setError(messageFromError(err)));
  }

  function appendProductLocalChatItem(sessionId: string, item: ChatItem) {
    setProductLocalChatItems((current) => ({
      ...current,
      [sessionId]: [...(current[sessionId] ?? []), item]
    }));
  }

  function appendProductLocalToolActivities(sessionId: string, activities: ToolActivityModel[]) {
    if (activities.length === 0) {
      return;
    }

    setProductLocalToolActivities((current) => ({
      ...current,
      [sessionId]: [...(current[sessionId] ?? []), ...activities]
    }));
  }

  if (!isDebugRoute) {
    const activeProductSessionId = productSessions.activeSessionId;
    const historicalChatItems = activeProductSessionId ? messagesToChatItems(productSessions.activeMessages) : [];
    const scopedLocalChatItems = activeProductSessionId
      ? dedupeLocalChatItems(productLocalChatItems[activeProductSessionId] ?? [], historicalChatItems)
      : [];
    const scopedToolActivities = activeProductSessionId
      ? [...(productLocalToolActivities[activeProductSessionId] ?? []), ...toolActivities]
      : [];
    const productTimeline = activeProductSessionId
      ? buildProductTimeline({
          sessionId: activeProductSessionId,
          activeChatItems: [...historicalChatItems, ...scopedLocalChatItems],
          pendingApprovals: state.pendingApprovals,
          activeToolActivities: scopedToolActivities,
          isLoading: busy === "agent"
        })
      : [];
    const conversations = productSessions.sessions.map(mapSessionToConversation);

    return (
      <main className="product-entry">
        {busyMessage ? (
          <div className="busy-banner product-entry__banner" aria-live="polite">
            {busyMessage}
          </div>
        ) : null}

        {error ? <div className="error-banner product-entry__banner">{error}</div> : null}

        {copyStatus ? (
          <p className={`copy-status product-entry__banner ${copyStatus === "Copy failed" ? "copy-status-error" : ""}`} aria-live="polite">
            {copyStatus}
          </p>
        ) : null}

        {copyStatus === "Copy failed" ? (
          <textarea
            className="product-entry__manual-report"
            aria-label="Manual copy report"
            readOnly
            value={feedbackReport}
            rows={8}
          />
        ) : null}

        <div className="opencode-product-shell">
          <OpencodeSidebar
            conversations={conversations}
            currentConversationId={activeProductSessionId ?? ""}
            onSelectConversation={handleSelectProductConversation}
            onNewConversation={() => void handleNewProductConversation()}
            onDeleteConversation={(id) => void productSessions.deleteConversation(id).catch((err) => setError(messageFromError(err)))}
            isOpen={isProductSidebarOpen}
          />
          <section className="opencode-product-workspace">
            <OpencodeChatHeader
              onStop={() => void handleStopAgentTurn()}
              messageCount={productTimeline.filter((item) => item.kind === "message").length}
              onToggleSidebar={() => setIsProductSidebarOpen((current) => !current)}
              isSidebarOpen={isProductSidebarOpen}
              onOpenRemoteSettings={() => setProductSettingsOpen(true)}
              connectionStatusText={productConnectionStatus}
              isSending={busy === "agent" || canStopAgentTurn}
            />
            <div className={`opencode-product-status opencode-product-status--${executionStatus.state}`} role="status" aria-label="执行状态">
              {executionStatus.label}
            </div>
            <section className="opencode-product-main" aria-label="远程 Linux 对话工作区">
              {productTimeline.length === 0 ? (
                <OpencodeWelcomePage onSuggestedQuestion={(question) => void handleProductSend(question)} />
              ) : (
                <div className="opencode-product-timeline" aria-live="polite">
                  {productTimeline.map((item) => {
                    if (item.kind === "message") {
                      return <OpencodeChatMessage key={item.id} message={item.message} />;
                    }
                    if (item.kind === "permission") {
                      return (
                        <InlinePermissionMessage
                          key={item.id}
                          permission={item.permission}
                          onRespond={(response) => handleApproval(item.permission.id, mapInlinePermissionResponse(response))}
                        />
                      );
                    }
                    if (item.kind === "tool") {
                      return (
                        <OpencodeChatMessage
                          key={item.id}
                          message={toolMessageFromPart(item.sessionId, item.tool)}
                        />
                      );
                    }
                    return <OpencodeChatMessage key={item.id} message={loadingMessage(item.sessionId)} />;
                  })}
                </div>
              )}
            </section>
            <OpencodeChatInput
              onSendMessage={(content) => void handleProductSend(content)}
              onStop={() => void handleStopAgentTurn()}
              disabled={Boolean(productSessions.error)}
              isSending={busy === "agent"}
            />
          </section>
        </div>

        <RemoteSettingsDialog
          open={productSettingsOpen}
          target={state.connection.target}
          status={{
            server: state.connection.server,
            sse: state.connection.sse,
            service: serviceLabel,
            model: modelLabel,
            targetLabel
          }}
          sshCheck={sshCheck}
          busy={busy !== null}
          onOpenChange={setProductSettingsOpen}
          onSaveTarget={saveTarget}
          onCheckSsh={handleSshCheck}
        />
      </main>
    );
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

      {copyStatus ? (
        <p className={`copy-status ${copyStatus === "Copy failed" ? "copy-status-error" : ""}`} aria-live="polite">
          {copyStatus}
        </p>
      ) : null}

      <section className="panel agent-panel product-chat-panel" aria-labelledby="agent-heading">
        <div className="section-heading">
          <h2 id="agent-heading">Agent Chat</h2>
          <span>remote Linux assistant</span>
        </div>
        <ChatView
          messages={chatItems}
          finalAnswer={finalAnswer?.text}
          onCopyFinalAnswer={handleCopyFinalAnswer}
          composer={{
            value: agentMessage,
            onChange: setAgentMessage,
            onSubmit: () => void handleAgentTurn(),
            disabled: false,
            busy: busy === "agent",
            placeholder: "Ask the remote Linux device...",
            submitLabel: "Send",
            busyLabel: "Sending"
          }}
        />
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
                <div className="approval-card__details">
                  <div className="approval-card__field">
                    <span>Command</span>
                    <pre>{approval.command}</pre>
                  </div>
                  <ApprovalField label="Reason" value={approval.risk_reason || "Server needs this command to continue the current request."} />
                  <ApprovalField label="Risk" value={approval.risk_level || "unknown"} />
                  <ApprovalField label="Target" value={debugApprovalTargetLabel(approval)} />
                </div>
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
                    className="secondary-button"
                    disabled={busy !== null}
                    onClick={() => handleApproval(approval.id, "approve_session")}
                    type="button"
                  >
                    Approve session
                  </button>
                  <button
                    className="danger-button"
                    disabled={busy !== null}
                    onClick={() => handleApproval(approval.id, "reject_stop")}
                    type="button"
                  >
                    Reject and stop
                  </button>
                </div>
              </article>
            ))}
          </div>
        )}
      </section>

      <section className="panel tool-activity-panel" aria-labelledby="tool-activity-heading">
        <div className="section-heading">
          <h2 id="tool-activity-heading">Tool Activity</h2>
          <span>{toolActivities.length}</span>
        </div>
        <ToolActivity activities={toolActivities} />
      </section>

      <details className="panel diagnostics-panel secondary-panel">
        <summary>
          <span>Diagnostics</span>
          <span>{diagnostics.length} presets</span>
        </summary>
        <div className="section-heading">
          <h2 id="diagnostics-heading">Diagnostics</h2>
          <span>preset commands</span>
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
      </details>

      <details className="panel command-panel secondary-panel">
        <summary>
          <span>Advanced Command</span>
          <span>approval required</span>
        </summary>
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
      </details>

      <details className="panel timeline-panel secondary-panel">
        <summary>
          <span>Raw Timeline</span>
          <div className="section-actions">
            <span>{state.timeline.length}</span>
            <button
              className="secondary-button compact-button"
              onClick={(event) => {
                event.preventDefault();
                event.stopPropagation();
                void handleCopyReport();
              }}
              type="button"
            >
              Copy report
            </button>
          </div>
        </summary>
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
      </details>

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

function ApprovalField({ label, value }: { label: string; value: string }) {
  return (
    <div className="approval-card__field">
      <span>{label}</span>
      <p>{value}</p>
    </div>
  );
}

function debugApprovalTargetLabel(approval: PendingApproval): string {
  if (approval.target_label && approval.target) {
    return `${approval.target_label} (${approval.target})`;
  }
  return approval.target_label || approval.target || "current server target";
}

function messagesToChatItems(messages: Message[]): ChatItem[] {
  return messages.flatMap((message) => {
    if (message.role !== "user" && message.role !== "assistant") {
      return [];
    }

    const text = message.parts
      .map((part) => (part.kind === "text" && typeof part.text === "string" ? part.text : ""))
      .filter(Boolean)
      .join("\n");

    if (!text.trim()) {
      return [];
    }

    return [{
      id: message.id,
      role: message.role,
      text,
      createdAtMs: message.created_at_ms,
      isFinal: message.role === "assistant" ? true : undefined
    }];
  });
}

function dedupeLocalChatItems(localItems: ChatItem[], historicalItems: ChatItem[]): ChatItem[] {
  return localItems.filter((localItem) =>
    !historicalItems.some(
      (historicalItem) => historicalItem.role === localItem.role && historicalItem.text.trim() === localItem.text.trim()
    )
  );
}

function mapInlinePermissionResponse(response: "once" | "always" | "reject"): ApprovalAction {
  switch (response) {
    case "once":
      return "approve_once";
    case "always":
      return "approve_session";
    case "reject":
      return "reject_stop";
    default:
      return "reject_stop";
  }
}

function toolMessageFromPart(sessionId: string, tool: ToolPartLike): SessionMessageLike {
  return {
    info: {
      id: `tool-message:${tool.id}`,
      sessionID: sessionId,
      role: "assistant",
      time: {
        created: tool.time.created
      }
    },
    parts: [tool]
  };
}

function loadingMessage(sessionId: string): SessionMessageLike {
  return {
    info: {
      id: `loading:${sessionId}`,
      sessionID: sessionId,
      role: "assistant",
      time: {
        created: Date.now()
      }
    },
    parts: []
  };
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

function approvalResponseSummary(result: unknown): string {
  if (!result || typeof result !== "object") {
    return "";
  }
  const record = result as Record<string, unknown>;
  const text = record.assistant_text ?? record.summary;
  return typeof text === "string" ? text.trim() : "";
}

function latestTurnIdFromResponse(response: AgentTurnResponse): string {
  const approvalTurnId = response.pending_approvals
    .map((approval) => approval.agent_turn_id)
    .find((turnId): turnId is string => Boolean(turnId));
  return approvalTurnId || response.turn_id || "";
}

function turnIdFromEvent(event: ServerEvent): string {
  if (event.type !== "assistant.started" && event.type !== "approval.asked") {
    return "";
  }

  const payload = typeof event.payload === "object" && event.payload !== null
    ? event.payload as Record<string, unknown>
    : {};
  const turnId = payload.agent_turn_id ?? payload.turn_id ?? payload.id;
  return typeof turnId === "string" ? turnId : "";
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
