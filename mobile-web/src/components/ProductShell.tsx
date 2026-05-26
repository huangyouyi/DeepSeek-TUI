import { useState, type CSSProperties, type FormEvent } from "react";
import { ToolActivity as ToolActivityView } from "./ToolActivity";
import type { ChatItem, ToolActivity as ToolActivityModel } from "../state";
import type { ApprovalAction, PendingApproval, SessionSummary } from "../types";

export type ProductShellComposer = {
  value: string;
  busy: boolean;
  onChange: (value: string) => void;
  disabled?: boolean;
  placeholder?: string;
};

export type ProductShellExecutionStatus = {
  label: string;
  detail?: string;
  tone: "idle" | "running" | "waiting" | "error";
};

export type ProductShellProps = {
  sessions: SessionSummary[];
  activeSessionId?: string;
  connectionStatusText: string;
  targetLabel: string;
  messages: ChatItem[];
  pendingApprovals: PendingApproval[];
  toolActivities: ToolActivityModel[];
  finalAnswer?: string | null;
  executionStatus?: ProductShellExecutionStatus;
  composer: ProductShellComposer;
  onSend: () => void;
  onContinue: () => void;
  onRetry: () => void;
  onStop: () => void;
  canStop?: boolean;
  controlsBusy?: boolean;
  continueDisabled?: boolean;
  retryDisabled?: boolean;
  onNewConversation: () => void;
  onSelectConversation: (sessionId: string) => void;
  onApproveApproval: (approvalId: string, action: Extract<ApprovalAction, "approve_once" | "approve_session">) => void;
  onRejectApproval: (approvalId: string, action: Extract<ApprovalAction, "reject_stop">) => void;
  onOpenSettings: () => void;
  onCopyFinalAnswer: () => void;
  onCopyFullReport?: () => void;
};

const suggestedPrompts = [
  "检查远程 Linux 设备状态",
  "总结刚才的工具输出",
  "排查磁盘或内存压力"
];

const commonDiagnostics = [
  {
    label: "当前系统",
    prompt: "请诊断当前系统概况：发行版/内核、运行时间、负载、关键硬件信息和明显异常，只给出建议的检查步骤和结论。",
    hint: "系统概况"
  },
  {
    label: "网络",
    prompt: "请诊断网络连通性：接口状态、IP/路由、网关可达性、外网连通性和丢包/延迟线索，只给出建议的检查步骤和结论。",
    hint: "连通性"
  },
  {
    label: "DNS",
    prompt: "请诊断 DNS 解析问题：解析器配置、常见域名解析、上游 DNS 可达性、缓存/劫持线索和修复建议。",
    hint: "解析"
  },
  {
    label: "磁盘",
    prompt: "请诊断磁盘空间和 IO 压力：分区使用率、inode、挂载状态、读写错误、最大目录和清理建议。",
    hint: "空间/IO"
  },
  {
    label: "内存/CPU",
    prompt: "请诊断内存/CPU 压力：负载、CPU 占用、内存/Swap、OOM 迹象、热点进程和缓解建议。",
    hint: "资源压力"
  },
  {
    label: "服务",
    prompt: "请诊断系统服务状态：失败服务、关键服务日志、启动项、监听端口和需要优先处理的服务异常。",
    hint: "systemd/init"
  },
  {
    label: "Docker",
    prompt: "请诊断 Docker 运行状态：daemon、容器健康、镜像/卷占用、网络、最近错误日志和清理建议。",
    hint: "容器"
  },
  {
    label: "OpenWrt/路由器",
    prompt: "请诊断 OpenWrt/路由器状态：版本、接口、WAN/LAN、DNS/DHCP、防火墙、无线和最近系统日志摘要。",
    hint: "路由"
  },
  {
    label: "日志摘要",
    prompt: "请汇总最近日志中的异常：内核、系统服务、认证、网络和应用错误，按影响程度排序并给出下一步排查建议。",
    hint: "异常排序"
  }
];

const styles: Record<string, CSSProperties> = {
  shell: {
    minHeight: "100vh",
    display: "flex",
    background: "#070a11",
    color: "#eef2ff",
    fontFamily: "Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif"
  },
  sidebar: {
    width: "min(332px, 36vw)",
    minWidth: "260px",
    display: "flex",
    flexDirection: "column",
    gap: "14px",
    padding: "18px",
    background: "#0b1020",
    borderRight: "1px solid rgba(148, 163, 184, 0.16)"
  },
  sidebarHeader: {
    display: "flex",
    alignItems: "center",
    gap: "12px"
  },
  brandMark: {
    width: "42px",
    height: "42px",
    borderRadius: "8px",
    display: "grid",
    placeItems: "center",
    background: "linear-gradient(135deg, #2563eb, #0f766e)",
    color: "#ffffff",
    fontWeight: 800,
    letterSpacing: "0"
  },
  brandName: {
    fontWeight: 760,
    fontSize: "15px",
    lineHeight: 1.2
  },
  muted: {
    color: "#94a3b8",
    fontSize: "12px"
  },
  primaryButton: {
    border: 0,
    borderRadius: "8px",
    padding: "12px 14px",
    background: "#1d4ed8",
    color: "#ffffff",
    fontWeight: 700,
    cursor: "pointer"
  },
  conversationList: {
    display: "flex",
    flexDirection: "column",
    gap: "8px",
    overflowY: "auto"
  },
  conversation: {
    border: "1px solid rgba(148, 163, 184, 0.14)",
    borderRadius: "8px",
    padding: "12px",
    background: "rgba(15, 23, 42, 0.74)",
    color: "#dbeafe",
    cursor: "pointer",
    textAlign: "left"
  },
  activeConversation: {
    borderColor: "rgba(20, 184, 166, 0.55)",
    background: "rgba(20, 184, 166, 0.12)"
  },
  workspace: {
    flex: 1,
    minWidth: 0,
    display: "flex",
    flexDirection: "column"
  },
  header: {
    minHeight: "76px",
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    gap: "14px",
    padding: "14px 18px",
    background: "rgba(10, 15, 26, 0.96)",
    borderBottom: "1px solid rgba(148, 163, 184, 0.14)"
  },
  secondaryButton: {
    border: "1px solid rgba(148, 163, 184, 0.18)",
    borderRadius: "8px",
    padding: "10px 12px",
    background: "rgba(15, 23, 42, 0.78)",
    color: "#e2e8f0",
    cursor: "pointer"
  },
  headerTitle: {
    flex: 1,
    minWidth: 0
  },
  eyebrow: {
    color: "#5eead4",
    fontSize: "12px",
    textTransform: "uppercase",
    fontWeight: 700
  },
  title: {
    margin: 0,
    fontSize: "18px",
    lineHeight: 1.2
  },
  settingsButton: {
    border: "1px solid rgba(45, 212, 191, 0.32)",
    borderRadius: "8px",
    padding: "9px 12px",
    background: "rgba(20, 184, 166, 0.1)",
    color: "#ccfbf1",
    display: "flex",
    flexDirection: "column",
    alignItems: "flex-start",
    gap: "2px",
    cursor: "pointer",
    minWidth: "0",
    maxWidth: "300px"
  },
  scroll: {
    flex: 1,
    overflowY: "auto",
    padding: "22px",
    background: "linear-gradient(180deg, #080d17 0%, #101827 100%)"
  },
  welcome: {
    maxWidth: "760px",
    margin: "7vh auto",
    display: "flex",
    flexDirection: "column",
    gap: "14px"
  },
  welcomeTitle: {
    margin: 0,
    fontSize: "36px",
    lineHeight: 1.08
  },
  suggestions: {
    display: "flex",
    flexWrap: "wrap",
    gap: "10px"
  },
  suggestion: {
    border: "1px solid rgba(148, 163, 184, 0.18)",
    borderRadius: "8px",
    padding: "10px 12px",
    background: "rgba(15, 23, 42, 0.82)",
    color: "#dbeafe",
    cursor: "pointer"
  },
  thread: {
    maxWidth: "860px",
    display: "flex",
    flexDirection: "column",
    gap: "14px"
  },
  workbench: {
    width: "min(1280px, 100%)",
    margin: "0 auto",
    display: "grid",
    gridTemplateColumns: "minmax(0, 1fr) minmax(320px, 380px)",
    gap: "18px",
    alignItems: "start"
  },
  chatColumn: {
    minWidth: 0,
    display: "flex",
    flexDirection: "column",
    gap: "14px"
  },
  toolActivityPanel: {
    minWidth: 0,
    maxHeight: "calc(100vh - 170px)",
    overflowY: "auto",
    border: "1px solid rgba(148, 163, 184, 0.14)",
    borderRadius: "8px",
    padding: "14px",
    background: "rgba(8, 13, 23, 0.86)"
  },
  panelHeader: {
    display: "flex",
    justifyContent: "space-between",
    gap: "12px",
    alignItems: "flex-start",
    marginBottom: "12px"
  },
  executionStatus: {
    border: "1px solid rgba(148, 163, 184, 0.18)",
    borderRadius: "8px",
    padding: "10px 12px",
    background: "rgba(15, 23, 42, 0.7)",
    color: "#e2e8f0",
    overflowWrap: "anywhere"
  },
  message: {
    border: "1px solid rgba(148, 163, 184, 0.14)",
    borderRadius: "8px",
    padding: "13px 15px",
    background: "rgba(15, 23, 42, 0.76)"
  },
  userMessage: {
    alignSelf: "flex-end",
    maxWidth: "78%",
    background: "rgba(37, 99, 235, 0.2)",
    borderColor: "rgba(96, 165, 250, 0.28)"
  },
  assistantMessage: {
    alignSelf: "flex-start",
    maxWidth: "82%"
  },
  label: {
    color: "#93c5fd",
    fontSize: "12px",
    fontWeight: 700,
    marginBottom: "6px"
  },
  approval: {
    border: "1px solid rgba(251, 191, 36, 0.38)",
    borderRadius: "8px",
    padding: "14px",
    background: "rgba(146, 64, 14, 0.18)"
  },
  code: {
    display: "block",
    margin: "10px 0",
    padding: "10px",
    borderRadius: "8px",
    background: "rgba(2, 6, 23, 0.72)",
    color: "#fde68a",
    whiteSpace: "pre-wrap",
    overflowWrap: "anywhere"
  },
  row: {
    display: "flex",
    flexWrap: "wrap",
    gap: "10px",
    alignItems: "center"
  },
  controlBar: {
    display: "flex",
    flexWrap: "wrap",
    gap: "8px",
    alignItems: "center"
  },
  dangerButton: {
    border: "1px solid rgba(248, 113, 113, 0.4)",
    borderRadius: "8px",
    padding: "9px 12px",
    background: "rgba(127, 29, 29, 0.34)",
    color: "#fecaca",
    cursor: "pointer"
  },
  finalAnswer: {
    border: "1px solid rgba(20, 184, 166, 0.35)",
    borderRadius: "8px",
    padding: "16px",
    background: "rgba(13, 148, 136, 0.13)",
    display: "flex",
    justifyContent: "space-between",
    gap: "14px",
    alignItems: "flex-start"
  },
  diagnosticPanel: {
    padding: "12px 18px 14px",
    background: "#0b0f19",
    borderTop: "1px solid rgba(148, 163, 184, 0.12)"
  },
  diagnosticHeader: {
    display: "flex",
    alignItems: "baseline",
    justifyContent: "space-between",
    gap: "10px",
    marginBottom: "9px"
  },
  diagnosticGrid: {
    display: "grid",
    gridTemplateColumns: "repeat(9, minmax(92px, 1fr))",
    gap: "8px",
    overflowX: "auto",
    paddingBottom: "2px"
  },
  diagnosticButton: {
    border: "1px solid rgba(148, 163, 184, 0.18)",
    borderRadius: "8px",
    padding: "9px 10px",
    background: "rgba(15, 23, 42, 0.82)",
    color: "#e2e8f0",
    cursor: "pointer",
    display: "grid",
    gap: "3px",
    minHeight: "50px",
    minWidth: "92px",
    textAlign: "left"
  },
  composer: {
    display: "flex",
    gap: "12px",
    padding: "16px 18px",
    background: "#0b0f19",
    borderTop: "1px solid rgba(148, 163, 184, 0.14)"
  },
  textarea: {
    flex: 1,
    minHeight: "56px",
    maxHeight: "160px",
    resize: "vertical",
    border: "1px solid rgba(148, 163, 184, 0.18)",
    borderRadius: "8px",
    padding: "12px",
    background: "#111827",
    color: "#f8fafc"
  }
};

function formatSessionTime(value: number): string {
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

function resolveExecutionStatus(
  provided: ProductShellExecutionStatus | undefined,
  connectionStatusText: string,
  pendingApprovals: PendingApproval[],
  toolActivities: ToolActivityModel[]
): ProductShellExecutionStatus {
  if (provided) {
    return provided;
  }

  const connectionText = connectionStatusText.toLowerCase();
  if (connectionText.includes("error") || connectionText.includes("断") || connectionText.includes("disconnected")) {
    return { label: "SSE 连接异常", detail: connectionStatusText, tone: "error" };
  }

  if (pendingApprovals.length > 0) {
    return { label: "等待授权", detail: "服务器请求执行命令，请先选择授权方式。", tone: "waiting" };
  }

  const runningActivity = toolActivities.find((activity) => ["running", "started", "updated"].includes(activity.status));
  if (runningActivity) {
    return {
      label: "命令执行中",
      detail: runningActivity.title || runningActivity.command || "服务器工具正在执行。",
      tone: "running"
    };
  }

  return { label: "空闲", detail: "可以继续输入新的请求。", tone: "idle" };
}

function approvalTargetLabel(approval: PendingApproval): string {
  if (approval.target_label && approval.target) {
    return `${approval.target_label} (${approval.target})`;
  }
  return approval.target_label || approval.target || "当前服务器目标";
}

function approvalReason(approval: PendingApproval): string {
  return approval.risk_reason || "服务器需要执行该命令才能继续当前请求。请检查命令、风险等级和目标设备后再选择授权方式。";
}

export function ProductShell({
  sessions,
  activeSessionId,
  connectionStatusText,
  targetLabel,
  messages,
  pendingApprovals,
  toolActivities,
  finalAnswer,
  executionStatus,
  composer,
  onSend,
  onContinue,
  onRetry,
  onStop,
  canStop = false,
  controlsBusy = false,
  continueDisabled = false,
  retryDisabled = false,
  onNewConversation,
  onSelectConversation,
  onApproveApproval,
  onRejectApproval,
  onOpenSettings,
  onCopyFinalAnswer,
  onCopyFullReport
}: ProductShellProps) {
  const [isConversationDrawerOpen, setIsConversationDrawerOpen] = useState(false);
  const hasFinalAnswer = Boolean(finalAnswer?.trim());
  const hasConversation = messages.length > 0 || pendingApprovals.length > 0 || toolActivities.length > 0 || hasFinalAnswer;
  const sendDisabled = composer.disabled || composer.busy || composer.value.trim().length === 0;
  const continueControlDisabled = controlsBusy || continueDisabled;
  const retryControlDisabled = controlsBusy || retryDisabled;
  const stopControlDisabled = controlsBusy || !canStop;
  const statusTone = connectionStatusText.toLowerCase().includes("error") ? "需要关注" : "在线";
  const currentExecutionStatus = resolveExecutionStatus(executionStatus, connectionStatusText, pendingApprovals, toolActivities);

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!sendDisabled) {
      onSend();
    }
  }

  return (
    <div className="product-shell product-shell--dark" style={styles.shell}>
      {isConversationDrawerOpen ? (
        <button
          className="product-shell__scrim"
          type="button"
          aria-label="关闭会话侧栏"
          onClick={() => setIsConversationDrawerOpen(false)}
        />
      ) : null}
      <aside
        className={`product-shell__sidebar${isConversationDrawerOpen ? "" : " product-shell__sidebar--closed"}`}
        style={styles.sidebar}
        aria-label="Conversation drawer"
      >
        <div className="product-shell__sidebar-header" style={styles.sidebarHeader}>
          <div className="product-shell__brand-mark" style={styles.brandMark} aria-hidden="true">
            DS
          </div>
          <div>
            <div className="product-shell__brand-name" style={styles.brandName}>
              DeepSeek 远程 Linux
            </div>
            <div className="product-shell__brand-meta" style={styles.muted}>
              {sessions.length} 个会话
            </div>
          </div>
        </div>

        <button className="product-shell__new-chat" style={styles.primaryButton} type="button" onClick={onNewConversation}>
          新建会话
        </button>

        <nav className="product-shell__conversation-list" style={styles.conversationList} aria-label="会话列表">
          {sessions.length === 0 ? (
            <div className="product-shell__sidebar-empty" style={styles.muted}>
              暂无会话
            </div>
          ) : (
            sessions.map((session) => {
              const isActive = session.id === activeSessionId;
              return (
                <button
                  className={`product-shell__conversation${isActive ? " product-shell__conversation--active" : ""}`}
                  style={{ ...styles.conversation, ...(isActive ? styles.activeConversation : {}) }}
                  type="button"
                  key={session.id}
                  onClick={() => onSelectConversation(session.id)}
                  aria-current={isActive ? "page" : undefined}
                >
                  <span className="product-shell__conversation-title" style={{ display: "block", fontWeight: 700, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                    {session.title || "未命名会话"}
                  </span>
                  <span className="product-shell__conversation-time" style={styles.muted}>
                    {formatSessionTime(session.updated_at_ms)}
                  </span>
                </button>
              );
            })
          )}
        </nav>
      </aside>

      <section className="product-shell__workspace" style={styles.workspace}>
        <header className="product-shell__header" style={styles.header}>
          <button
            className="product-shell__menu-button"
            style={styles.secondaryButton}
            type="button"
            aria-label="打开会话侧栏"
            onClick={() => setIsConversationDrawerOpen((current) => !current)}
          >
            会话
          </button>
          <div className="product-shell__header-title" style={styles.headerTitle}>
            <div className="product-shell__eyebrow" style={styles.eyebrow}>
              <span className="product-shell__status-dot" aria-hidden="true" />
              {statusTone} · {connectionStatusText}
            </div>
            <div
              className={`product-shell__execution-status product-shell__execution-status--${currentExecutionStatus.tone}`}
              role="status"
              aria-label="执行状态"
              aria-live="polite"
            >
              <strong>{currentExecutionStatus.label}</strong>
              {currentExecutionStatus.detail ? <span>{currentExecutionStatus.detail}</span> : null}
            </div>
            <h1 style={styles.title}>DeepSeek 远程 Linux</h1>
            <p className="product-shell__header-subtitle" style={{ ...styles.muted, margin: "3px 0 0" }}>
              服务器执行工具，浏览器只负责交互
            </p>
          </div>
          <div className="product-shell__controls" style={styles.controlBar} aria-label="Agent controls">
            <button
              className="product-shell__control-button"
              style={styles.secondaryButton}
              type="button"
              disabled={continueControlDisabled}
              onClick={onContinue}
            >
              继续
            </button>
            <button
              className="product-shell__control-button"
              style={styles.secondaryButton}
              type="button"
              disabled={retryControlDisabled}
              onClick={onRetry}
            >
              重试
            </button>
            <button
              className="product-shell__control-button product-shell__control-button--danger"
              style={styles.dangerButton}
              type="button"
              disabled={stopControlDisabled}
              onClick={onStop}
            >
              停止
            </button>
          </div>
          {onCopyFullReport ? (
            <button className="product-shell__copy-report" style={styles.secondaryButton} type="button" onClick={onCopyFullReport}>
              复制完整报告
            </button>
          ) : null}
          <button
            className="product-shell__settings-button"
            style={styles.settingsButton}
            type="button"
            onClick={onOpenSettings}
            aria-label="目标设备设置"
          >
            <span style={styles.muted}>目标设备</span>
            <strong style={{ overflowWrap: "anywhere" }}>{targetLabel}</strong>
          </button>
        </header>

        <main className="product-shell__chat-scroll" style={styles.scroll} aria-label="远程 Linux 对话工作区">
          {!hasConversation ? (
            <section className="product-shell__welcome" style={styles.welcome} aria-label="Welcome">
              <p className="product-shell__welcome-kicker" style={styles.eyebrow}>
                Linux-first thin client
              </p>
              <h2 style={styles.welcomeTitle}>连接远程 Linux 设备，让服务器侧 Agent 执行检查。</h2>
              <p style={{ ...styles.muted, fontSize: "15px", lineHeight: 1.5 }}>
                SSH、模型调用和工具策略都在服务器端处理。浏览器不会直接 SSH、不会读取本地文件、不会运行 shell，也不会保存 DeepSeek API key。
              </p>
              <div className="product-shell__welcome-target">
                <span style={styles.muted}>当前目标</span>
                <strong>{targetLabel}</strong>
              </div>
              <div className="product-shell__suggestions" style={styles.suggestions} aria-label="Suggested prompts">
                {suggestedPrompts.map((prompt) => (
                  <button
                    className="product-shell__suggestion"
                    style={styles.suggestion}
                    type="button"
                    key={prompt}
                    onClick={() => composer.onChange(prompt)}
                  >
                    {prompt}
                  </button>
                ))}
              </div>
            </section>
          ) : (
            <div className="product-shell__workbench" style={styles.workbench}>
              <section className="product-shell__chat-column" style={styles.chatColumn} aria-label="Chat">
                <div className="product-shell__thread" style={styles.thread} role="list" aria-label="对话消息" aria-live="polite">
                  {messages.map((message) => {
                    const isUser = message.role === "user";
                    return (
                      <article
                        className={`product-shell__message${
                          isUser ? " product-shell__message--user" : " product-shell__message--assistant"
                        }`}
                        style={{ ...styles.message, ...(isUser ? styles.userMessage : styles.assistantMessage) }}
                        key={message.id}
                        role="listitem"
                      >
                        <div className="product-shell__message-label" style={styles.label}>
                          {isUser ? "你" : "DeepSeek"}
                        </div>
                        <p style={{ margin: 0, whiteSpace: "pre-wrap" }}>{message.text}</p>
                      </article>
                    );
                  })}

                </div>

                {hasFinalAnswer ? (
                  <section
                    className="product-shell__final-answer product-shell__final-answer--sticky"
                    style={styles.finalAnswer}
                    aria-label="Final answer"
                  >
                    <div>
                      <span style={styles.eyebrow}>最终答案</span>
                      <p style={{ margin: "8px 0 0", whiteSpace: "pre-wrap" }}>{finalAnswer}</p>
                    </div>
                    <div className="product-shell__final-actions" style={styles.row}>
                      <button type="button" style={styles.secondaryButton} onClick={onCopyFinalAnswer}>
                        复制最终答案
                      </button>
                    </div>
                  </section>
                ) : null}
              </section>

              <aside className="product-shell__tool-activity-panel" style={styles.toolActivityPanel} aria-label="工具活动">
                <div className="product-shell__panel-header" style={styles.panelHeader}>
                  <div>
                    <h2 className="product-shell__card-title" style={{ marginTop: 0 }}>
                      工具活动
                    </h2>
                    <p className="product-shell__card-copy">执行过程单独记录，输出默认折叠。</p>
                  </div>
                  <div
                    className={`product-shell__execution-status product-shell__execution-status--${currentExecutionStatus.tone}`}
                    style={styles.executionStatus}
                    aria-hidden="true"
                  >
                    <strong>{currentExecutionStatus.label}</strong>
                    {currentExecutionStatus.detail ? <span>{currentExecutionStatus.detail}</span> : null}
                  </div>
                </div>

                {pendingApprovals.map((approval) => (
                  <article className="product-shell__approval" style={styles.approval} key={approval.id} aria-label="Pending approval">
                    <div className="product-shell__approval-meta" style={{ ...styles.row, ...styles.muted }}>
                      <span>需要授权</span>
                      <span>{approval.cwd || "默认目录"}</span>
                    </div>
                    <h2 className="product-shell__card-title">服务器请求执行命令</h2>
                    <p className="product-shell__card-copy">浏览器不会直接执行命令，授权结果只会发送给服务器端策略。</p>
                    <div className="product-shell__approval-details" aria-label="授权详情">
                      <div className="product-shell__approval-field product-shell__approval-field--command">
                        <span>命令</span>
                        <code style={styles.code}>{approval.command}</code>
                      </div>
                      <div className="product-shell__approval-field">
                        <span>为什么需要执行</span>
                        <p>{approvalReason(approval)}</p>
                      </div>
                      <div className="product-shell__approval-field">
                        <span>风险等级</span>
                        <p>{approval.risk_level || "unknown"}</p>
                      </div>
                      <div className="product-shell__approval-field">
                        <span>目标设备</span>
                        <p>{approvalTargetLabel(approval)}</p>
                      </div>
                    </div>
                    <div className="product-shell__approval-actions" style={styles.row}>
                      <button type="button" style={styles.primaryButton} onClick={() => onApproveApproval(approval.id, "approve_once")}>
                        仅这次执行
                      </button>
                      <button type="button" style={styles.secondaryButton} onClick={() => onApproveApproval(approval.id, "approve_session")}>
                        本会话都允许
                      </button>
                      <button type="button" style={styles.dangerButton} onClick={() => onRejectApproval(approval.id, "reject_stop")}>
                        拒绝并停止
                      </button>
                    </div>
                  </article>
                ))}

                <ToolActivityView activities={toolActivities} />
              </aside>
            </div>
          )}
        </main>

        <section className="product-shell__diagnostics" style={styles.diagnosticPanel} aria-label="常用诊断">
          <div className="product-shell__diagnostics-header" style={styles.diagnosticHeader}>
            <strong>常用诊断</strong>
          </div>
          <div className="product-shell__diagnostics-grid" style={styles.diagnosticGrid}>
            {commonDiagnostics.map((diagnostic) => (
              <button
                className="product-shell__diagnostic-button"
                style={styles.diagnosticButton}
                type="button"
                key={diagnostic.label}
                aria-label={diagnostic.label}
                disabled={composer.disabled || composer.busy}
                onClick={() => composer.onChange(diagnostic.prompt)}
              >
                <span className="product-shell__diagnostic-label">{diagnostic.label}</span>
                <small>{diagnostic.hint}</small>
              </button>
            ))}
          </div>
        </section>

        <form className="product-shell__composer" style={styles.composer} aria-label="消息输入区" onSubmit={handleSubmit}>
          <textarea
            style={styles.textarea}
            value={composer.value}
            onChange={(event) => composer.onChange(event.currentTarget.value)}
            disabled={composer.disabled || composer.busy}
            placeholder={composer.placeholder ?? "询问远程 Linux 设备..."}
            rows={3}
          />
          <button type="submit" style={styles.primaryButton} disabled={sendDisabled}>
            {composer.busy ? "发送中" : "发送"}
          </button>
        </form>
      </section>
    </div>
  );
}
