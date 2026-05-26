import type { CSSProperties } from "react";

export type OpencodeConversation = {
  id: string;
  title: string;
  createdAt: number;
  updatedAt: number;
};

export type OpencodeSidebarProps = {
  conversations: OpencodeConversation[];
  currentConversationId: string;
  onSelectConversation: (id: string) => void;
  onNewConversation: () => void;
  onDeleteConversation: (id: string) => void;
  isOpen: boolean;
};

const styles: Record<string, CSSProperties> = {
  sidebar: {
    width: "min(320px, 86vw)",
    minWidth: "260px",
    height: "100%",
    display: "flex",
    flexDirection: "column",
    gap: "14px",
    padding: "16px",
    background: "#0b1020",
    color: "#e2e8f0",
    borderRight: "1px solid rgba(148, 163, 184, 0.18)",
    transition: "transform 180ms ease"
  },
  closed: {
    display: "none"
  },
  header: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    gap: "10px"
  },
  title: {
    margin: 0,
    fontSize: "17px",
    lineHeight: 1.2
  },
  count: {
    color: "#94a3b8",
    fontSize: "12px"
  },
  newButton: {
    border: 0,
    borderRadius: "8px",
    padding: "11px 12px",
    background: "#2563eb",
    color: "#ffffff",
    fontWeight: 700,
    cursor: "pointer"
  },
  list: {
    flex: 1,
    display: "flex",
    flexDirection: "column",
    gap: "8px",
    overflowY: "auto"
  },
  item: {
    display: "grid",
    gridTemplateColumns: "1fr auto",
    gap: "8px",
    alignItems: "center",
    borderStyle: "solid",
    borderWidth: "1px",
    borderColor: "rgba(148, 163, 184, 0.16)",
    borderRadius: "8px",
    padding: "8px",
    background: "#111827"
  },
  activeItem: {
    borderColor: "rgba(45, 212, 191, 0.56)",
    background: "rgba(20, 184, 166, 0.12)"
  },
  selectButton: {
    minWidth: 0,
    border: 0,
    padding: "2px",
    background: "transparent",
    color: "inherit",
    cursor: "pointer",
    textAlign: "left"
  },
  itemTitle: {
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
    fontWeight: 700,
    fontSize: "14px"
  },
  itemDate: {
    marginTop: "4px",
    color: "#94a3b8",
    fontSize: "12px"
  },
  deleteButton: {
    border: "1px solid rgba(248, 113, 113, 0.32)",
    borderRadius: "8px",
    padding: "7px 9px",
    background: "rgba(127, 29, 29, 0.24)",
    color: "#fecaca",
    cursor: "pointer"
  },
  disabledButton: {
    opacity: 0.4,
    cursor: "not-allowed"
  },
  empty: {
    color: "#94a3b8",
    fontSize: "14px",
    textAlign: "center",
    padding: "34px 10px"
  }
};

function formatTimestamp(timestamp: number) {
  const date = new Date(timestamp);
  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit"
  }).format(date);
}

export function OpencodeSidebar({
  conversations,
  currentConversationId,
  onSelectConversation,
  onNewConversation,
  onDeleteConversation,
  isOpen
}: OpencodeSidebarProps) {
  const canDelete = conversations.length > 1;

  return (
    <aside
      aria-label="会话列表"
      aria-hidden={!isOpen}
      style={{
        ...styles.sidebar,
        ...(isOpen ? null : styles.closed)
      }}
    >
      <div style={styles.header}>
        <div>
          <h2 style={styles.title}>DeepSeek</h2>
          <div style={styles.count}>共 {conversations.length} 个对话</div>
        </div>
        <button type="button" onClick={onNewConversation} style={styles.newButton}>
          新建
        </button>
      </div>

      <div style={styles.list}>
        {conversations.length === 0 ? (
          <div style={styles.empty}>暂无对话</div>
        ) : (
          conversations.map((conversation) => (
            <div
              key={conversation.id}
              style={{
                ...styles.item,
                ...(conversation.id === currentConversationId ? styles.activeItem : null)
              }}
            >
              <button
                type="button"
                onClick={() => onSelectConversation(conversation.id)}
                aria-current={conversation.id === currentConversationId ? "page" : undefined}
                style={styles.selectButton}
              >
                <div style={styles.itemTitle}>{conversation.title}</div>
                <div style={styles.itemDate}>{formatTimestamp(conversation.updatedAt)}</div>
              </button>
              <button
                type="button"
                onClick={() => onDeleteConversation(conversation.id)}
                disabled={!canDelete}
                aria-label={`删除 ${conversation.title}`}
                title={canDelete ? "删除对话" : "至少需要保留一个对话"}
                style={{
                  ...styles.deleteButton,
                  ...(canDelete ? null : styles.disabledButton)
                }}
              >
                删除
              </button>
            </div>
          ))
        )}
      </div>
    </aside>
  );
}
