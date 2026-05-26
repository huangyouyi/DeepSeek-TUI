import type { CSSProperties } from "react";

export type OpencodeWelcomePageProps = {
  onSuggestedQuestion: (question: string) => void;
};

const suggestions = [
  "检查远程 Linux 设备状态",
  "排查网络连通性",
  "检查磁盘和内存压力",
  "总结刚才的工具输出"
];

const styles: Record<string, CSSProperties> = {
  root: {
    maxWidth: "860px",
    margin: "0 auto",
    padding: "42px 18px",
    color: "#e2e8f0"
  },
  header: {
    marginBottom: "26px"
  },
  eyebrow: {
    margin: "0 0 8px",
    color: "#5eead4",
    fontSize: "12px",
    fontWeight: 800,
    textTransform: "uppercase"
  },
  title: {
    margin: 0,
    fontSize: "28px",
    lineHeight: 1.16
  },
  subtitle: {
    maxWidth: "620px",
    margin: "12px 0 0",
    color: "#94a3b8",
    lineHeight: 1.6
  },
  grid: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))",
    gap: "12px"
  },
  suggestion: {
    minHeight: "74px",
    border: "1px solid rgba(148, 163, 184, 0.18)",
    borderRadius: "8px",
    padding: "14px",
    background: "#111827",
    color: "#f8fafc",
    textAlign: "left",
    cursor: "pointer",
    font: "inherit",
    fontWeight: 700
  }
};

export function OpencodeWelcomePage({ onSuggestedQuestion }: OpencodeWelcomePageProps) {
  return (
    <section style={styles.root} aria-label="欢迎页">
      <div style={styles.header}>
        <p style={styles.eyebrow}>Remote Linux Assistant</p>
        <h1 style={styles.title}>从一个诊断问题开始</h1>
        <p style={styles.subtitle}>
          选择常见远程 Linux 排查入口，或直接在输入框描述你想检查的设备状态、网络、磁盘、内存和工具输出。
        </p>
      </div>

      <div style={styles.grid}>
        {suggestions.map((suggestion) => (
          <button
            key={suggestion}
            type="button"
            onClick={() => onSuggestedQuestion(suggestion)}
            style={styles.suggestion}
          >
            {suggestion}
          </button>
        ))}
      </div>
    </section>
  );
}
