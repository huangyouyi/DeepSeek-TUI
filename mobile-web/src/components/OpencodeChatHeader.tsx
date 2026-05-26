import type { CSSProperties } from "react";

export type OpencodeChatHeaderProps = {
  onStop: () => void;
  messageCount: number;
  onToggleSidebar: () => void;
  isSidebarOpen: boolean;
  onOpenRemoteSettings: () => void;
  connectionStatusText: string;
  isSending?: boolean;
};

const styles: Record<string, CSSProperties> = {
  header: {
    minHeight: "72px",
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    gap: "12px",
    padding: "13px 16px",
    borderBottom: "1px solid rgba(148, 163, 184, 0.18)",
    background: "#0b1020",
    color: "#e2e8f0"
  },
  left: {
    minWidth: 0,
    display: "flex",
    alignItems: "center",
    gap: "12px"
  },
  titleWrap: {
    minWidth: 0
  },
  title: {
    margin: 0,
    fontSize: "18px",
    lineHeight: 1.2
  },
  meta: {
    display: "flex",
    flexWrap: "wrap",
    gap: "8px",
    marginTop: "4px",
    color: "#94a3b8",
    fontSize: "12px"
  },
  actions: {
    display: "flex",
    alignItems: "center",
    gap: "8px"
  },
  button: {
    border: "1px solid rgba(148, 163, 184, 0.22)",
    borderRadius: "8px",
    padding: "9px 11px",
    background: "#111827",
    color: "#f8fafc",
    cursor: "pointer",
    font: "inherit",
    fontWeight: 700,
    whiteSpace: "nowrap"
  },
  stopButton: {
    borderColor: "rgba(248, 113, 113, 0.42)",
    background: "rgba(127, 29, 29, 0.34)",
    color: "#fecaca"
  },
  disabledButton: {
    opacity: 0.45,
    cursor: "not-allowed"
  },
  status: {
    color: "#5eead4"
  }
};

export function OpencodeChatHeader({
  onStop,
  messageCount,
  onToggleSidebar,
  isSidebarOpen,
  onOpenRemoteSettings,
  connectionStatusText,
  isSending = false
}: OpencodeChatHeaderProps) {
  return (
    <header style={styles.header}>
      <div style={styles.left}>
        <button
          type="button"
          onClick={onToggleSidebar}
          aria-label={isSidebarOpen ? "关闭侧边栏" : "打开侧边栏"}
          style={styles.button}
        >
          {isSidebarOpen ? "收起" : "会话"}
        </button>
        <div style={styles.titleWrap}>
          <h1 style={styles.title}>DeepSeek 远程 Linux</h1>
          <div style={styles.meta}>
            <span>{messageCount} 条消息</span>
            <span style={styles.status}>{connectionStatusText}</span>
          </div>
        </div>
      </div>

      <div style={styles.actions}>
        <button type="button" onClick={onOpenRemoteSettings} aria-label="远程设置" style={styles.button}>
          设置
        </button>
        <button
          type="button"
          onClick={onStop}
          disabled={!isSending}
          aria-label="停止生成"
          style={{
            ...styles.button,
            ...styles.stopButton,
            ...(isSending ? null : styles.disabledButton)
          }}
        >
          停止
        </button>
      </div>
    </header>
  );
}
