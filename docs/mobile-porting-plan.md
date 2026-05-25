# deepseek-tui 手机端 Agent Core 移植计划

本文档合并了两份 `docs/mobile-porting-plan.md` 的内容。重复的架构判断、风险说明、MVP 路线和结论已去重；英文版本中独有的当前实现状态、验证阶段、任务树和里程碑已翻译为中文。英文专有名词如 `DeepSeek TUI`、`Agent Core`、`mobile-agent-core`、`kai-runner`、`UniFFI`、`MCP`、`SSH`、`PowerShell` 保留原文。

核心结论：不要把 `DeepSeek TUI` 原样移植到 iOS。正确路线是新建轻量的手机端 `mobile-agent-core`，让它负责对话、规划、审批、会话状态、模型调用和远程工具调度；电脑侧通过 SSH/PowerShell、轻量 `kai-runner` 或 remote MCP 执行 shell/file/git/browser/LSP 等平台相关工具。

## 执行摘要

`DeepSeek TUI` 是很有价值的架构参考，但不是直接 iOS port 的合适起点。当前生产 runtime 仍集中在 `crates/tui`，Agent turn、工具执行、本地 shell、文件系统、MCP stdio、LSP、任务状态和终端 UI 耦合较深。workspace 中较新的 `crates/protocol`、`crates/tools`、`crates/execpolicy`、`crates/state`、`crates/agent`、`crates/app-server` 已经提供了可复用边界，但还不是完整生产 Agent loop 的来源。

推荐策略：

1. 新建 `mobile-agent-core`，承载手机端 Agent loop、session state、event stream、approval gate、model client trait、persistence trait 和 remote tool dispatcher。
2. iOS 端不包含本地 shell、文件修改、PTY、本地 LSP、桌面 sandbox、本地 MCP stdio spawn 和终端 UI。
3. 手机工作流只暴露 remote tools；本地桌面工具实现保留在 runner 或桌面 runtime。
4. 支持从 manual bootstrap，到 SSH/PowerShell，到轻量 `kai-runner`，再到 remote MCP/browser automation 的能力升级。

适配度判断：

| 角色 | 适配度 | 原因 |
|---|---|---|
| `DeepSeek TUI` 作为手机端 `Agent Core` | Low/Medium | 协议、审批、session、工具目录经验可复用，但当前 live agent loop 太依赖桌面本地执行。 |
| `DeepSeek TUI` 作为电脑侧 runner 参考 | Medium-High | shell/file/git/MCP/runtime API 等实现很有价值，但需要瘦身和安全加固。 |
| `DeepSeek TUI` 作为架构参考 | High | event model、approval flow、tool registry、session persistence 直接相关。 |

建议继续一周 technical spike，但不要做完整产品。Spike 应证明 iOS demo 能运行 Agent loop、持久化 session、发起审批，并通过 bootstrap 文本和 SSH transport 执行一个远程诊断命令。

## 产品定位

目标不是“把 Rust TUI 编译到 iOS”，而是做一个手机端电脑救援 Agent。核心价值是：当用户电脑还不能运行完整 Agent、环境损坏、安装器失败、命令行工具缺失、浏览器下载失败、权限/网络/证书/包管理器异常时，手机端仍能承担对话、规划、审批、状态管理和下一步指导。

产品前提：

- 电脑一开始可能没有 Node、Bun、Go、Rust、Python，也可能没有可用包管理器或远程连接。
- 用户可能只能在电脑终端手动执行一两条命令，再把输出复制、粘贴或拍照 OCR 回手机。
- 手机端必须轻量、可靠、独立，不能依赖电脑端已经安装完整 runtime。
- 电脑端能力应从“无 Agent”逐步升级到“SSH/PowerShell 可执行”，再升级到“轻量 runner”。
- 核心工作流是文本、命令、日志和结构化结果；RDP/VNC/截图/视频流不是核心控制平面。
- LLM 推理通过云端 API 完成，不在手机本地运行大模型。

这改变了对 `DeepSeek TUI` 的判断标准。`DeepSeek TUI` 默认 Agent 与工具执行在同一台机器；手机救援产品要求 Agent 先在手机上独立成立，再把电脑视为能力逐步增长的远端 target。

## 当前架构

当前 workspace 结构：

```text
deepseek CLI dispatcher
  ├─ crates/cli
  │    └─ 启动 deepseek-tui、app-server、mcp-server、doctor、config 等命令
  │
  ├─ crates/tui
  │    ├─ core/engine.rs、core/turn.rs、core/session.rs、core/events.rs
  │    │    └─ live interactive agent loop、turn、取消、event emission
  │    ├─ client.rs、llm_client/
  │    │    └─ DeepSeek/OpenAI-compatible streaming chat completions
  │    ├─ tools/
  │    │    └─ shell、file、git、browser/web、plan、task、subagent、RLM、OCR
  │    ├─ mcp.rs、mcp_server.rs
  │    │    └─ local MCP client/server、stdio spawn、HTTP MCP
  │    ├─ lsp/
  │    │    └─ 文件修改后按需 spawn 本地 LSP server
  │    ├─ sandbox/
  │    │    └─ macOS Seatbelt、Linux Landlock、Windows helper contracts
  │    ├─ runtime_api.rs、runtime_threads.rs、task_manager.rs
  │    │    └─ local HTTP/SSE runtime API 和 durable task timeline
  │    └─ tui/
  │         └─ ratatui UI、approval modal、command palette、history、rendering
  │
  ├─ crates/protocol
  │    └─ ThreadRequest/Response、ToolPayload、ToolOutput、EventFrame、approval events
  ├─ crates/tools
  │    └─ generic ToolRegistry、ToolHandler、ToolCall、ToolSpec、ToolResult
  ├─ crates/execpolicy
  │    └─ command approval policy、trusted/denied prefixes、approval decisions
  ├─ crates/state
  │    └─ SQLite thread/session/message/checkpoint/job persistence
  ├─ crates/core
  │    └─ 较新的 runtime boundary，但还不是完整 live TUI engine
  ├─ crates/app-server
  │    └─ HTTP/JSON-RPC wrapper over crates/core Runtime
  └─ crates/secrets、crates/config、crates/hooks、crates/agent、crates/tui-core
```

重要现实：

- `docs/ARCHITECTURE.md` 仍把 `crates/tui` 作为真实 end-user runtime。
- `crates/core` 有 thread、state、job、tool、approval、event 边界，但 `handle_prompt` 仍偏 scaffold，不包含 `crates/tui/src/core` 中完整的 streaming LLM/tool loop。
- `crates/tui/src/tools/spec.rs` 说明直接 iOS port 风险很高：tool context 包含 workspace path、shell manager、sandbox backend、network policy、memory file path、local LSP manager 和 large-output routing。
- `crates/tui/src/tools/shell.rs` 使用本地 process spawn、PTY、process group、stdin、cancellation、sandbox 和平台分支代码。
- `crates/tui/src/lsp` 需要通过 stdio spawn 本地 language server。
- `crates/tui/src/mcp.rs` 的 HTTP MCP 思路对 runner 有用；stdio spawn 不适合手机端。

## iOS 可移植性分级

| 模块或文件区域 | 分级 | 原因与处理建议 |
|---|---|---|
| `crates/protocol/src/lib.rs` | Portable | 主要是 `serde` protocol types。需要加入 mobile-specific remote tool payload 和 event variant。 |
| `crates/tools/src/lib.rs` | Portable | 通用 async registry 和 tool result 抽象可复用；避免引入 local executor 假设。 |
| `crates/execpolicy/src/lib.rs` | Needs Adapter | command approval core 有价值，但风险模型要扩展到 remote action、OS-specific hazard 和 tool class。 |
| `crates/agent` | Portable | model/provider registry 轻量，适合复用。 |
| `crates/config` | Needs Adapter | 使用桌面路径和配置文件；iOS 应由 Swift/Keychain/app storage 注入。 |
| `crates/secrets` | Needs Adapter | iOS 应使用 Keychain，可由 Swift 层或 iOS-specific Rust binding 注入。 |
| `crates/state` | Needs Adapter | SQLite schema 有价值，但路径发现和桌面 session 字段需要抽象。 |
| `crates/hooks` | Needs Adapter | audit/event sink 思路有用；webhook/stdout hooks 不应是手机 core 默认项。 |
| `crates/core` | Needs Adapter | runtime boundary 有价值，但不是完整 live agent loop，并依赖 config/MCP/state/hooks。 |
| `crates/app-server` | Desktop/Runner Only | 手机应消费 remote transport，不应把本地 Axum server 作为 core integration surface。 |
| `crates/mcp` | Needs Adapter | HTTP/SSE MCP 概念相关；stdio MCP spawn 留在 runner。 |
| `crates/tui/src/core/engine.rs` | Needs Adapter | 真实 Agent loop 在这里，但依赖 TUI app mode、本地 tool context、snapshot、LSP、subagent、shell manager、workspace。 |
| `crates/tui/src/core/session.rs`、`core/turn.rs`、`core/events.rs` | Needs Adapter | turn/session/event 思路可复用，但要去掉 filesystem snapshot 和 TUI event 假设。 |
| `crates/tui/src/client.rs`、`llm_client/`、`models.rs` | Needs Adapter | 云端 model client 需要复用形状；要移除桌面 config 假设，并暴露 Swift-friendly async boundary。 |
| `crates/tui/src/tools/plan.rs`、`todo.rs`、`handle.rs` | Portable/Needs Adapter | planning/checklist/handle 思路有用；要脱离 TUI-specific state 和 large-output store path。 |
| `crates/tui/src/tools/shell.rs` | Desktop Only | 本地 process、PTY、stdin、process group、sandbox；只应在 runner 侧执行。 |
| `crates/tui/src/tools/file.rs`、`apply_patch.rs`、`git.rs` | Desktop/Runner Only | 直接操作本地 workspace/git；手机只调用 `remote.file.*` 和 runner tools。 |
| `crates/tui/src/lsp/*` | Desktop/Runner Only | 依赖本地 LSP server binary 和 stdio。 |
| `crates/tui/src/sandbox/*` | Desktop/Runner Only | macOS/Linux/Windows sandbox 是目标电脑 concern。 |
| `crates/tui/src/runtime_api.rs`、`runtime_threads.rs`、`task_manager.rs` | Needs Adapter | event/timeline 模型有用；本地 server 和本地 task execution 不进 iOS core。 |
| `crates/tui/src/tui/*` | Desktop Only | ratatui、keyboard、clipboard、terminal rendering、approval modal；iOS 用原生 Swift UI。 |
| `crates/tui/src/repl/*`、`rlm/*`、`subagent/*` | Desktop/Cloud Only for MVP | Python REPL/RLM/subagents 太重，MVP 不进手机 core。 |
| `crates/cli` | Desktop Only | CLI dispatcher 和安装形态不是 iOS app 的组成部分。 |

## 推荐目标架构

```text
iOS App (Swift/SwiftUI)
  ├─ chat、command cards、approval sheets、connection setup、OCR/paste import
  ├─ Keychain credentials、SQLite app storage、Network.framework/URLSession
  └─ Rust FFI boundary
        │
        ▼
mobile-agent-core (Rust)
  ├─ Agent loop
  ├─ session state + message history
  ├─ planner/checklist state
  ├─ model client trait
  ├─ approval gate + risk classifier
  ├─ remote tool dispatcher
  ├─ event stream model
  └─ persistence abstraction
        │
        ├──────────────► Cloud model API
        │
        ▼
Remote Tool Transport trait
  ├─ ManualBootstrapTransport
  ├─ SshTransport
  ├─ PowerShellTransport
  ├─ RunnerWebSocketTransport
  └─ RemoteMcpTransport
        │
        ▼
Target computer
  ├─ no agent: 用户手动运行命令并回传文本/OCR 输出
  ├─ SSH/PowerShell: 有边界的命令执行和日志采集
  ├─ kai-runner: tools、browser automation、files、logs、capability discovery
  └─ remote MCP servers: runner 暴露的可选高层工具
```

职责划分：

- iOS App：原生 UI、连接 onboarding、approval UX、QR/paste/OCR import、本地通知、Keychain、SQLite 位置和后台恢复体验。
- `mobile-agent-core`：确定性的 Agent state machine。不能假设 shell、child process、terminal、PTY、桌面文件系统或本地 MCP server。
- model client：DeepSeek/OpenAI-compatible cloud chat completions streaming，支持 DeepSeek V4 thinking，并在 tool call 后保留 reasoning content。
- approval gate：执行前评估每个 remote tool call，产出结构化 approval request 给 Swift UI。
- remote tool transport：把批准后的 tool call 转为 manual instructions、SSH commands、PowerShell commands、runner RPC calls 或 remote MCP calls。
- target computer：在当前可用能力级别执行工具。

## `mobile-agent-core` 边界

`mobile-agent-core` 应包含：

- Agent loop 和 turn state。
- Session state、message history、DeepSeek thinking/tool-call replay 所需 reasoning content。
- Planner/checklist state。
- Tool call parser 和 dispatcher。
- Approval gate、risk classifier 和 target-aware policy。
- Remote tool schema、capability registry 和 dynamic tool catalog。
- Event stream model：text delta、reasoning delta、tool start/delta/result、approval request、connection status、error。
- Persistence abstraction；iOS SQLite 实现可放在 core 外或 feature 后。
- Model client trait；可用 Rust `reqwest` 实现，也可替换为 Swift `URLSession` bridge。
- Bootstrap transcript model：生成命令、用户执行状态、粘贴/OCR 输出、下一步建议。

`mobile-agent-core` 不应包含：

- local shell / `std::process::Command`
- local file editing / workspace traversal
- PTY / `portable-pty`
- TUI / `ratatui` / `crossterm`
- local LSP
- local MCP stdio server spawn
- desktop sandbox implementation
- desktop clipboard、keyboard、terminal rendering 或 OS process management
- phone filesystem 的 git snapshot
- local Python/RLM REPL

## 手机控制电脑模型

手机控制电脑应采用“手机规划，电脑执行”的严格分离模型：

```text
Level 0: Manual bootstrap
  用户从手机复制一条命令到电脑执行
  用户把终端输出粘贴或 OCR 回手机

Level 1: Remote shell
  手机通过 SSH、WinRM 或 PowerShell Remoting 连接电脑
  低风险诊断可自动运行
  中高风险命令必须审批

Level 2: Lightweight runner
  手机安装或引导安装 kai-runner
  runner 暴露结构化 tools 和 capabilities
  runner 不运行本地 LLM

Level 3: Enhanced tools
  runner 暴露 browser automation、installer logs、file operations、
  remote MCP calls、package repair 和 system diagnostics
```

原则：

- 手机端永远是 Agent owner。LLM 推理、规划、session history、审批上下文和用户解释都在手机端。
- 电脑端是 tool target。它只执行明确工具调用，不能自行扩大权限或替用户批准动作。
- bootstrap 模式是一等能力，不是降级错误路径。
- runner 是能力增强层，不是大模型宿主。
- 所有模式共用 tool risk policy、audit log 和 session timeline。

## 执行模式

### 模式 1：Bootstrap 手动引导

适用于电脑无法被远程连接时。

流程：

1. 用户在手机上描述问题。
2. Agent 创建短诊断计划。
3. Agent 输出一个安全命令或手动步骤。
4. 用户在电脑上执行。
5. 用户粘贴输出或用 OCR 回传文本。
6. Agent 解析输出并提出下一条命令。
7. 重复直到 SSH、runner 或目标软件可安装。

产品要求：

- 默认一次只给一条命令。
- 命令必须标注 OS 和 shell 假设。
- 危险修复即便在手动模式也要解释并审批。
- app 把手动命令和输出作为 audit log entry 保存。

### 模式 2：SSH / PowerShell 远程执行

适用于电脑接受远程命令时。

流程：

1. 用户创建或选择电脑连接。
2. Agent 探测 OS、shell、package manager、网络状态和权限。
3. Agent 自动运行低风险命令。
4. Agent 对中高风险命令请求审批。
5. stdout/stderr 以结构化事件回传。
6. Agent 根据输出决定下一步。

说明：

- macOS/Linux 首选 SSH。
- Windows 首选 PowerShell over SSH；WinRM/PowerShell Remoting 可后续加入。
- 每个 command result 都应包含 command、cwd、exit code、stdout、stderr、duration、truncation metadata 和 idempotency key。

### 模式 3：轻量 runner 增强模式

适用于电脑已稳定到能安装 `kai-runner` 时。

runner 负责：

- 执行 tools。
- 采集 logs。
- 按 policy 读写文件。
- 执行 browser automation。
- 上报 capabilities。
- 代理 remote MCP tools。
- stream structured events。

runner 不负责：

- 运行本地 LLM。
- 自主决策。
- 绕过手机端 approval policy。

### 模式 4：复杂软件安装与浏览器辅助

适用于安装需要网页下载、登录控制台、表单、浏览器点击或 installer 页面时。

行为：

- 无 runner：手机给出手动浏览器步骤，要求用户粘贴 visible text、error text、downloaded filename 或 OCR result。
- 有 runner：手机调用 `remote.browser.*` tools。
- Browser tools 返回 DOM text、URL、selected element summary、network status、download status；截图只作为可选 evidence，不作为核心控制面。
- login、submit、upload、payment、deletion、privilege elevation、account change 必须审批。

## 典型用户旅程

以“Homebrew 安装失败”为例：

1. 用户打开 iOS app，输入“帮我检查 Mac 上 Homebrew 为什么安装失败”。
2. Agent 询问 Mac 是否可通过 SSH 连接；如果不可连接，进入 bootstrap。
3. Agent 生成第一条低风险诊断命令：`uname -a; sw_vers; command -v brew; xcode-select -p; echo "$PATH"`。
4. 用户在电脑终端执行并粘贴输出。
5. Agent 判断是 Xcode Command Line Tools 缺失、PATH 损坏、代理/DNS 问题、权限问题、证书问题或 partial install。
6. 如 SSH 可用，Agent 连接并执行有边界的诊断。
7. 如修复需要安装或 sudo，Agent 展示风险摘要并请求审批。
8. 基础工具恢复后，Agent 引导启用 SSH 或安装 `kai-runner`。
9. Runner 上报 capabilities 后，Agent 从 shell-only diagnostics 切换到结构化 package/log/browser tools。
10. session 保存完整 timeline：用户问题、命令、输出摘要、审批决定、修复结论和回滚建议。

## 远程工具接口设计

统一输入字段建议：

```json
{
  "connection_id": "macbook-1",
  "cwd": "/Users/alice/project",
  "timeout_ms": 60000,
  "idempotency_key": "turn-tool-call-uuid"
}
```

统一输出字段建议：

```json
{
  "ok": true,
  "status": "completed",
  "duration_ms": 1234,
  "truncated": false,
  "artifact_ref": null
}
```

风险等级：

- Low：只读诊断，预期不暴露凭据、不修改远端。
- Medium：可能读敏感日志、打开页面、写普通用户配置或产生外部可见行为。
- High：sudo/admin、删除数据、修改网络/安全设置、修改 shell profile、安装服务、提交表单、上传文件、支付、账号变更。
- Critical：安装持久化 daemon、改权限、上传密钥、 destructive reset 等。

主要工具：

| Tool | 输入摘要 | 输出摘要 | 风险 | 审批 |
|---|---|---|---|---|
| `remote.shell.exec` | `connection_id`、`command`、`cwd?`、`env?`、`stdin?`、`timeout_ms?`、`capture_limit_bytes?`、`idempotency_key?` | `status`、`exit_code`、`stdout`、`stderr`、`duration_ms`、truncation metadata、`runner` | Low 到 High，取决于命令 | 只读 allowlist 可自动；mutating、privileged、destructive、credential-related、network/security-changing 必须审批 |
| `remote.powershell.exec` | `connection_id`、`script`、`working_directory?`、`execution_policy?`、`timeout_ms?`、`idempotency_key?` | shell 输出字段，另可含 `objects_json` | Low 到 High | registry edits、UAC elevation、service changes、script downloads、execution policy changes 高风险 |
| `remote.file.read` | `connection_id`、`path`、`start_line?`、`max_lines?`、`encoding?` | `path`、`content`、`total_lines?`、`shown_lines?`、`truncated`、`next_start_line?`、`sha256?` | Low/Medium | 已知日志和用户批准路径可自动；secrets、browser profiles、SSH keys、keychains、private documents 必须审批 |
| `remote.file.write` | `connection_id`、`path`、`content`、`mode`、`expected_sha256?`、`create_dirs?`、`diff_preview?` | `bytes_written`、`old_sha256?`、`new_sha256`、`diff?`、`backup_ref?` | High | 必须审批，尽量先生成 diff 和 backup/rollback plan |
| `remote.diagnose.system` | `connection_id`、`checks`、`cwd?` | platform 和 check summaries | Medium | 默认可自动或建议审批；不得修改系统 |
| `remote.package.install` | `manager`、`packages`、`version?`、`dry_run?`、`cwd?` | `changed`、stdout/stderr、preview | Critical | 必须审批，优先 dry-run/preview |
| `remote.browser.open` | `url`、`browser?`、`profile?`、`purpose?` | `session_id?`、`url`、`title?` | Medium | 外部 URL 建议审批；本地文档可自动 |
| `remote.browser.extract_text` | `url?`、`session_id?`、`selector?`、`max_chars?`、`wait_ms?` | `url`、`title`、`text`、`links?`、truncation metadata | Low/Medium | 一般可自动；登录态/内网域名建议审批 |
| `remote.browser.click` | `session_id`、`selector?`、`text?`、`x?`、`y?`、`purpose` | `url`、`title`、`observed_text?`、`screenshot_ref?` | High | 必须审批 |
| `remote.bootstrap.guide` | `os_hint?`、`desired_transport`、`user_can_copy_paste` | `steps`、`commands`、`verification_challenge?` | Medium | 生成安装/诊断命令前建议审批；实际执行由用户手动完成 |
| `remote.mcp.call` | `server`、`tool`、`arguments`、`timeout_ms?`、`idempotency_key?` | `result`、`server`、`tool`、`duration_ms` | Dynamic | 读工具可自动；未知/写工具必须审批 |

`remote.shell.exec` 不能暴露为无限制 bash。手机端 policy 应把命令、cwd、target、风险解释和 LLM 理由一起展示给用户；runner 侧必须二次校验，不能信任手机端。

## 通信方式

| 方案 | 适用场景 | 优点 | 缺点 | 建议 |
|---|---|---|---|---|
| SSH | 用户已有 Mac/Linux 账号，局域网/VPN/公网可达，MVP 诊断 | 无需预装 runner；概念成熟；可 bootstrap | iOS SSH key 管理复杂；网络可达性差；Windows 支持不稳定；stream/cancel/file transfer 需要封装 | MVP 支持，作为 bootstrap 和 power-user 模式 |
| HTTPS/WebSocket runner | 长期主路径，电脑已安装 `kai-runner` | capability discovery、audit、stream log、cancel、reconnect、双向审批、细粒度 sandbox | 需要安装升级；auth/pairing/firewall 复杂 | 推荐主架构 |
| MCP over HTTP/SSE | 用户已有 MCP 生态或 runner 想暴露标准工具 | 标准化 tool/resource/prompt；手机无需 stdio spawn | MCP auth/approval/risk 仍需外层 policy；很多 MCP server 仍只能本地 stdio | 作为 runner 后面的工具总线 |
| 手动 bootstrap | 电脑初始不可达、无 SSH、无 runner | 不要求预安装；用户复制命令即可开始 | 不是自动执行；容易复制错误；安全提示必须清晰 | 必须支持 |

推荐顺序：MVP 支持 SSH + manual bootstrap；早期稳定版加入 `kai-runner` HTTPS/WebSocket；扩展生态时让 runner 暴露 MCP over HTTP/SSE。

## iOS 集成

可选方案：

| 方案 | 优点 | 缺点 | 适配点 |
|---|---|---|---|
| Rust `staticlib` + Swift FFI | 最少魔法，ABI 可控 | 手写 C ABI 成本高，复杂类型和 async stream 麻烦 | 适合极小 API，不适合大量事件和 schema |
| UniFFI | Rust 类型、async、error 导出到 Swift 更自然，减少 FFI 样板 | 需要维护 UDL/生成流程；stream/callback 仍需设计 | 推荐用于 `mobile-agent-core` |
| Flutter/React Native native module | 跨平台 UI 快 | 多一层 runtime；Keychain/后台/network 仍需桥接 | 若产品目标跨平台，可包在 UniFFI 外层 |

推荐：`mobile-agent-core` 使用 Rust library + UniFFI。Swift 负责 UI、Keychain、SQLite 路径、网络权限和生命周期；Rust core 负责 deterministic state machine、risk policy、remote tool schema 和 event production。

## 产品能力映射

| 产品能力 | 当前类似能力 | 可复用模块 | 新增模块 | iOS core | 电脑 runner | MVP |
|---|---|---|---|---|---|---|
| 用户意图理解 | 有 | `crates/tui/src/core/engine/turn_loop.rs`、prompts | mobile rescue prompt、故障 taxonomy | 是 | 否 | 是 |
| 诊断计划生成 | 有 | `tools/plan.rs`、`tools/todo.rs` | rescue-specific playbooks | 是 | 否 | 是 |
| Bootstrap 手动引导 | 只有对话能力 | session/message/planning | `BootstrapSession`、manual command guide、OCR/text ingest | 是 | 否 | 是 |
| SSH 远程执行 | 无，本地 shell | `execpolicy`、shell result schema | `SshTransport`、remote shell schema、stream reconnect | 调度/审批 | 执行 | 是 |
| PowerShell 远程执行 | 无 | `execpolicy` 思路 | `PowerShellTransport`、Windows risk classifier | 调度/审批 | 执行 | 是 |
| 远程文件读取 | 本地 file tools | `tools/file.rs` 分页思想 | `remote.file.read`、sensitive path policy | 调度/审批 | 读取 | 是 |
| 系统诊断 | 有本地 diagnostics | `tools/diagnostics.rs` | `remote.diagnose.system` per OS checks | 规划/解释 | 探测 | 是 |
| 包管理器修复 | 可通过本地 shell | shell、approval、task gate | brew/winget/apt/npm repair playbooks | 规划/审批 | 执行 | 是 |
| 浏览器自动化 | 有 web/fetch，非桌面浏览器 | `web_run.rs`、`fetch_url.rs` | `remote.browser.*`、manual browser guide | 规划/审批 | 控制浏览器 | Should |
| runner 安装升级 | 无专门能力 | CLI/install docs、runtime API | installer flow、pairing、self-update | 引导/审批 | 安装/升级 | 是 |
| capability discovery | 部分有 | `runtime_api.rs`、`mcp.rs`、`ToolRegistryBuilder` | runner capability schema | 是 | 上报 | 是 |
| 工具风险评估 | shell prefix approval | `crates/execpolicy`、`approval_cache.rs` | target-aware risk classifier | 是 | 二次校验 | 是 |
| 用户审批 | 有 TUI approval/runtime endpoint | `core/engine/approval.rs`、`runtime_api.rs` | Swift approval sheet、remote approval payload | 是 | enforcement mirror | 是 |
| 审计日志 | 有 tool audit/log/runtime events | `runtime_log.rs`、`audit.rs`、`runtime_threads.rs` | mobile audit timeline、runner signed log | 是 | 是 | 是 |
| 会话恢复 | 有 | `crates/state`、`runtime_threads.rs`、`session_manager.rs` | mobile persistence、transport reconnect replay | 是 | runner job replay | 是 |

MVP 底线：用户意图理解、诊断计划、bootstrap、SSH/PowerShell、基础文件/日志读取、系统诊断、包管理器修复、风险评估、审批、审计、会话恢复和 runner 安装引导。

## 场景覆盖

| 场景 | 可复用点 | 缺口 | 承载位置 | MVP |
|---|---|---|---|---|
| macOS Homebrew 安装失败 | shell tool、diagnostics、approval、planning | bootstrap/OCR、SSH transport、brew 诊断模板 | iOS core 规划 + runner/SSH 执行 | 必须 |
| Xcode Command Line Tools 缺失 | shell 执行和错误解释 | `xcode-select --install` 是 GUI/系统动作，需要手动步骤和审批 | iOS guide + runner 诊断 | 必须 |
| macOS 证书/代理/DNS/权限异常 | network policy、fetch/web、shell diagnostics | 系统网络配置、Keychain/证书说明、代理环境差异 | runner system diagnose + iOS 解释 | 必须 |
| Windows Python/Node/VS Code 安装失败 | planning/approval/session | PowerShell/winget/MSI logs/Event Log/UAC 适配 | runner PowerShell adapter | 必须 |
| Windows winget/PowerShell/PATH/UAC 异常 | exec policy 思路 | Windows 命令风险模型和权限升级流程 | runner + iOS approval | 必须 |
| 用户不会看命令行 | LLM conversation、message history | OCR/text ingest、错误文本结构化解析 | iOS core | 必须 |
| 电脑无法联网 | planning | offline remediation templates、手机下载/传输策略、hash 校验 | iOS core + manual guide | Should |
| 引导安装 runner | runtime API/MCP/tool registry | pairing、installer、capability discovery、安全模型 | iOS core + runner | 必须 |

总体判断：`DeepSeek TUI` 对“已有本地执行能力后的 Agent 行为”覆盖较好；对“电脑一开始不可用、只能手动救援”的最低保底能力覆盖不足，需要新增 bootstrap-first 产品层。

## 当前实现状态

这部分来自英文分支的新增内容，描述当前 scaffold 与真实平台验证之间的边界。

- `crates/mobile-agent-core` 已有 session、persistence、agent loop、model trait、transport trait、approval/risk、event、bootstrap、audit、remote schema、UniFFI UDL 等 scaffold。
- `crates/kai-runner` 已有 pairing API、capabilities endpoint、bearer auth、本地 HTTP smoke、runner-to-mobile core smoke、file read/write、diagnose scaffold、browser adapter seam、PowerShell request shape、maintenance dry-run/audit。
- iOS demo 已有 SwiftUI session list、chat/event rendering、command cards、approval sheet、connection setup、Keychain/SQLite adapter scaffold、background/resume UI model 和 UniFFI handoff scripts。
- Shell/PowerShell 执行只应在显式 approval nonce 和 policy 检查后开放；默认 runner 仍应阻止 raw shell execution。
- Maintenance execution、package install execution 和真实 browser engine 仍未启用；browser extraction 需要真实 `BrowserEngine`。
- Linux 可以测试 Rust API、fake transport、runner HTTP handler、auth、pairing 和 scaffold；不能证明 SwiftUI、Keychain、SQLite sandbox、Xcode package resolution、simulator launch、signing、local network permission 或真机行为。

## MVP 改造计划

### Phase 1：抽象 tool execution

目标：从 `crates/tui` 中明确分离“Agent 想调用工具”和“本机如何执行工具”。

工作项：

- 定义 `ToolExecutor` trait，输入 tool name/arguments/session context，输出 stream/event/result。
- 定义 `ToolRiskPolicy`，从 shell prefix 扩展到 remote action class。
- 定义 `EventSink`，让 TUI、runtime API、mobile core 可以消费同一类 tool/model/approval 事件。
- 保持 `crates/tui` 行为不变，只先引入 boundary。
- 为现有 shell/file/git tools 加 adapter，而不是直接改语义。

### Phase 2：Mobile mode 用 remote tools 替换 local tools

目标：手机 core 不链接本地 shell/file/LSP/TUI。

工作项：

- 定义 `remote.*` tool catalog。
- 将 `exec_shell`、`read_file`、`write_file`、`apply_patch`、`git`、`lsp` 映射为 runner-side tools。
- 手机端 model prompt 只暴露 `remote.*` tools。
- 本地工具实现留在 runner 或 desktop runtime。
- 对每个 remote tool 加 risk metadata 和 approval payload。

### Phase 3：实现 `mobile-agent-core`

当前：scaffold 已存在，产品 wiring 未完成。

应继续完成：

- 将 fake model/fake transport 的 bootstrap loop 扩展为真实 model client。
- 补齐 session replay、reasoning replay、pending approval resume。
- 将 SQLite 持久化落到 iOS sandbox。
- 将 `uniffi_api.udl` 生成 Swift bindings 并替换 mock bridge。
- 保持 core 无 TUI、无 shell、无 process spawn。

### Phase 4：iOS demo shell

当前：Mock SwiftUI scaffold 已存在，Xcode/模拟器/真机验证未完成。

应继续完成：

- 在 macOS 上运行 `ios/DeepSeekMobileDemo/Scripts/verify-macos`。
- 打开 `ios/DeepSeekMobileDemo/Package.swift`，跑 Swift package tests 和 simulator build。
- 验证 session list、chat、command card、connection setup、approval sheet、bootstrap paste、maintenance approval、browser approval、audit timeline。
- 把 mock-only session/audit 数据逐步替换为 `mobile-agent-core` 真实 Rust bridge。

### Phase 5：接入 `kai-runner` / remote MCP

当前：runner scaffold 完成较多，真实部署和平台 hardening 未完成。

应继续完成：

- 明确 `kai-runner` binary/service entrypoint。
- 绑定明确 local address，暴露 `/health`、`/pairing/code`、`/pairing/redeem`、`/capabilities`、`/approval/nonce`、`/tool-call`。
- 手机 demo 通过 `RunnerHttpTransport` 或未来 WebSocket transport 连接 live runner。
- 在 nonce binding、command policy、output redaction、timeout/cancel、sudo/UAC 行为和 OS sandbox 证明前，不开放任意 shell。

## 后续阶段建议

1. 先执行 `docs/mobile-validation-checklists.md` 中真实平台验证清单，再改变平台支持声明。使用 `docs/mobile-evidence-bundles/templates/` 的空骨架，或在 Linux 上 dry-run `scripts/mobile_evidence_bundle.py --list` 与 `scripts/mobile_evidence_bundle.py --platform <platform> --dry-run`。
2. 在真实 macOS 上证明 Xcode toolchain 路径：Swift package tests、Xcode 打开 package、simulator 启动，并记录缺失的 project/signing/binding 工作。
3. 从 `crates/mobile-agent-core/src/uniffi_api.udl` 生成并链接真实 UniFFI bridge，逐个替换 Swift demo 的 mock surface。
4. 增加真实 `kai-runner` binary/service entrypoint，包括 bind address、TLS 或 local-network pairing posture、token storage、logs、lifecycle commands 和 version reporting。
5. 在一次性 Mac 和 Windows VM 上验证 runner install/upgrade，失败也作为一等 audit outcome。
6. 扩展 remote shell 前先加固：allowed cwd roots、command lease metadata、nonce 与 command hash/session/tool/cwd/idempotency 绑定、output redaction、timeout/cancel、sudo/UAC 行为、macOS/Linux/Windows sandbox 期望。
7. 选择第一个真实 browser engine 放到 `BrowserEngine` 后面，`remote.browser.click` 继续保持 approval-bound。
8. Maintenance 从 dry-run plan 变成 staged executor 前，必须证明 binary signing、rollback、idempotency 和 local approval UX。
9. 提前准备 App Store 风险评审：local network、remote control 表述、credential storage、browser automation、background limits、privacy labels 和 reviewer demo credentials。

## 验证与证据阶段

真实平台验证被拆成 S/T/U/V/W 阶段，避免把 Linux scaffold 误说成 macOS/iOS/Windows 已通过。

| 阶段 | 状态 | 内容 |
|---|---|---|
| S5 | done | `docs/mobile-validation-checklists.md` 已拆出 macOS、Windows、iOS simulator、iOS device、LAN runner 验证清单，包含前置条件、命令/动作、预期结果和失败证据。 |
| S6-S12 | open | 真实 Mac/Xcode、iOS simulator/device、LAN runner、Windows、UniFFI link、真实 browser engine 仍需硬件或真实环境验证。 |
| T5 | done | 已加入 fillable evidence log templates，要求记录日期、设备、OS/runtime、commit/worktree、命令、结果、日志、截图、blockers 和 next steps。 |
| T6-T10 | open | 真实 macOS/iOS/LAN runner/Windows evidence run 尚未完成，完成后要回填本计划并转成任务。 |
| U5 | done | 已加入 result summary table 和 plan-backfill 规则。 |
| U6-U10 | open | 需要把完成的 evidence summary 回填到相关 S/T/U milestone 和实现任务。 |
| V5 | done | 已加入 `docs/mobile-evidence-bundles/` 模板、README 和 `scripts/mobile_evidence_bundle.py` dry-run/list flow。 |
| V6-V10 | open | 需要生成真实 macOS、iOS、LAN runner、Windows evidence bundle，并将 blocker 转成 implementation tasks。 |
| W1-W3 | done | Linux LAN runner evidence harness、plan draft parser、Linux validation entrypoint 已可支持本地证据工作流。 |
| W4 | open | 真实 macOS/iOS/Windows 证据仍依赖硬件和真实平台执行。 |
| W5 | done/limited | Linux mobile Web SSH simulator 可做 LAN/Web UI 形态验证，但不能关闭真实 iOS、macOS、Windows 或真实 runner 证据项。 |

## 任务树

```text
Mobile Porting Program
├─ A. Architecture split
│  ├─ [done] A1. 清点当前 runtime dependencies
│  ├─ [done] A2. 冻结 phone-core non-goals：无 shell、PTY、TUI、LSP、MCP stdio
│  ├─ [done] A3. 定义 mobile-agent-core public API
│  ├─ [done] A4. 定义 remote tool protocol 和 event protocol
│  └─ [done] A5. 添加 cargo workspace member，默认不改变 app 行为
│
├─ B. Mobile agent core
│  ├─ [done] B1. Session model
│  ├─ [done] B2. Message/session snapshots
│  ├─ [done] B3. 带 max-step guard 的 turn loop
│  ├─ [done] B4. Model client trait 和 fake model
│  ├─ [done] B5. Tool dispatcher trait 和 fake/runner transports
│  ├─ [done] B6. Approval gate 和 risk classifier
│  ├─ [done] B7. Event stream
│  ├─ [done] B8. Persistence abstraction 和 SQLite scaffold
│  └─ [partial] B9. UniFFI binding surface 已存在；Swift demo 尚未链接生成 bindings
│
├─ C. Bootstrap mode
│  ├─ [done] C1. Bootstrap tool schema
│  ├─ [done] C2. Command card generation
│  ├─ [done] C3. User output ingestion
│  ├─ [partial] C4. Paste normalization scaffold；真实 OCR 是 iOS 后续工作
│  ├─ [done] C5. Manual audit log
│  ├─ [done] C6. Homebrew/Xcode CLT diagnostic playbook prompt
│  ├─ [done] C7. macOS runner install/bootstrap fallback guidance
│  └─ [done] C8. Windows runner rescue guidance
│
├─ D. Remote execution
│  ├─ [partial] D1. SSH connection config 已存在；生产 credential storage 是 iOS/Keychain 工作
│  ├─ [partial] D2. `remote.shell.exec` schema 和 fake/runner path 已存在；live runner execution 仍 gated/incomplete
│  ├─ [partial] D3. Output event shapes 已存在；真实 network streaming 未完成
│  ├─ [partial] D4. Timeout fields 和 runner helpers 已存在；cancel 语义需真实 transport 验证
│  ├─ [partial] D5. Maintenance planning 已有 idempotency keys；更广泛 mutating tools 仍需绑定
│  ├─ [partial] D6. PowerShell schema/runner support 已存在；真实 Windows host smoke 未完成
│  └─ [done] D7. Windows diagnostics/bootstrap playbook scaffold
│
├─ E. Runner
│  ├─ [partial] E1. `kai-runner` crate 和 pairing API 已存在；standalone install/service path 未完成
│  ├─ [done] E2. capabilities endpoint 和 report schema
│  ├─ [partial] E3. shell/powershell tool 只在 explicit approval nonce policy 后可用
│  ├─ [done] E4. file read/write 有 workspace confinement 和 backups
│  ├─ [done] E5. diagnose system scaffold
│  ├─ [partial] E6. package install 仅 dry-run/preview；execution disabled
│  ├─ [partial] E7. browser session/approval scaffold only；无真实 browser engine
│  ├─ [partial] E8. remote MCP proxy shape 已存在；hardening 和真实 servers 未完成
│  └─ [partial] E9. self-update/uninstall plans/audit 已存在；execution disabled
│
├─ F. iOS app
│  ├─ [done] F1. SwiftUI session list scaffold
│  ├─ [done] F2. Chat 和 event rendering scaffold
│  ├─ [done] F3. Command cards
│  ├─ [done] F4. Approval sheet 展示 shell/browser nonce
│  ├─ [done] F5. Connection setup 支持 bootstrap、SSH、runner、remote MCP
│  ├─ [partial] F6. Keychain adapter scaffold；真实设备验证未完成
│  ├─ [partial] F7. SQLite persistence adapter scaffold；iOS sandbox 验证未完成
│  ├─ [partial] F8. Background/resume UI model 已存在；真实 lifecycle 验证未完成
│  └─ [open] F9. Internal testing build/TestFlight build
│
├─ G. Verification
│  ├─ [done] G1. Fake model deterministic tests
│  ├─ [done] G2. Fake transport approval tests
│  ├─ [partial] G3. Manual bootstrap Homebrew test scaffold；真实 Mac execution 未完成
│  ├─ [partial] G4. SSH Mac Homebrew test scaffold；真实 Mac execution 未完成
│  ├─ [partial] G5. Windows PowerShell smoke scaffold；真实 Windows host 未完成
│  ├─ [done] G6. Runner capability/auth/pairing tests
│  ├─ [done] G7. Audit log 和 session recovery tests
│  ├─ [done] G8. Runner maintenance dry-run/audit tests
│  ├─ [done] G9. Browser session/approval scaffold tests
│  └─ [open] G10. macOS Xcode/Swift/iOS simulator verification
│
└─ Platform validation
   ├─ [done] P1-P3. Runner live pairing、mobile-core-to-live-runner smoke、maintenance approval E2E
   ├─ [partial] P4. Maintenance execution 仍是 dry-run；signed artifacts 和 staged executor 未完成
   ├─ [done] Q1/Q3/Q5/Q7. Browser seam、shell nonce policy、PowerShell scaffold、iOS UniFFI handoff scripts
   ├─ [partial] Q2/Q4/Q6. 真实 browser engine、production OS sandbox、真实 Windows host smoke 未完成
   ├─ [done] S1-S5/T1-T5/U1-U5/V1-V5/W1-W3. 文档、证据模板、parser 和 Linux tooling 已完成
   └─ [open] S6-S12/T6-T10/U6-U10/V6-V10/W4. 真实 macOS/iOS/LAN runner/Windows 证据和 blocker 回填未完成
```

## 手机可运行里程碑

| 里程碑 | 手机上运行什么 | 人工可验证什么 | 范围 |
|---|---|---|---|
| M0: Rust core linked | Rust `uniffi_api` scaffold 已存在，尚未真正在手机上证明 | Linux 可测 Rust API；Xcode binding link 未完成 | Core API、fake model、generated Swift bindings |
| M1: Bootstrap command cards | Mock Swift UI 和 Rust bootstrap steps 已存在 | Linux 可测命令生成；真实 Mac paste flow 未完成 | Bootstrap mode、fake/real model |
| M2: Real cloud model | Rust model client scaffolds/tests 已存在 | Linux 可测 transport shape；phone streaming 未完成 | Model client、event stream、iOS network integration |
| M3: Session resume | Rust/Swift persistence scaffolds 已存在 | Linux 可测 Rust persistence；iOS kill/reopen 未完成 | SQLite persistence、iOS lifecycle |
| M4: SSH diagnostics | Rust SSH mapping/fake transport 已存在 | 真实 Mac SSH execution 未完成 | SSH transport、approval gate、credential storage |
| M5: Approved repair | Rust/iOS approval scaffolds 已存在 | Mock nonce UX 可 review；真实高风险执行仍 blocked/open | Risk classifier、approval UI、nonce binding |
| M6: Runner mode | Runner HTTP/auth/pairing/capability scaffold 已存在 | Linux 可测 API；phone-to-live-runner 未完成 | Runner transport、capabilities、service lifecycle |
| M7: Browser assist | Browser schema/session/click approval scaffold 已存在 | Fake engine 可测 adapter/click approval；无真实 browser | Browser tool schema、runner `BrowserEngine`、audit policy |
| M8: Runner maintenance | Dry-run plan/audit scaffold 已存在 | Linux 可测 self-update/uninstall plan 和 nonce audit；execution disabled | Signed artifacts、rollback、staged executor |
| M9: Runner pairing live smoke | Local HTTP runner pairing 和 bearer auth 已有测试 | Linux 可测 local socket pairing、capabilities、auth rejection、diagnose；LAN phone-to-runner 未完成 | Pairing manager、bearer routes、capabilities、diagnose |
| M10: Maintenance approval E2E | Mobile approval metadata 可塑形为 runner maintenance plan request | Linux 可测 nonce、denial、replay rejection、redacted audit；真实 update/uninstall disabled | Approval request、nonce manager、maintenance dry-run、audit redaction |
| M11: Browser adapter | Runner 通过 `BrowserEngine` 和 session registry 接受 browser open/extract/click shape | Fake engine 可测；未驱动真实 browser | `BrowserEngine`、session registry、approval-bound click |
| M12: Shell and PowerShell policy | Shell/PowerShell routes 只在 explicit approval nonce scaffold 后存在 | Linux 可测 blocked default 和 request shape；真实 Windows/production sandbox 未完成 | `ShellEnabledKaiRunner<E>`、nonce policy、PowerShell schema |
| M13: iOS UniFFI handoff | macOS scripts 和 Swift package layout 记录 generated binding/artifact path | script syntax 和 plan checks 可 review；真实 generation/link/simulator/device 未完成 | `generate-uniffi-macos`、`verify-macos`、Swift bindings、static library/XCFramework |
| M14: UniFFI and pairing bridge scaffold | Rust UDL 和 iOS pairing-upgrade bridge 已存在 | Linux 可测 UDL/API alignment 和 redacted pairing profile shaping | `uniffi_api.udl`、`build.rs`、`CoreBridgeJSONAdapter` |
| M15: Gated local executor smoke | 真实 local shell executor 只在 test opt-in、approval nonce 和 policy 后可用 | Linux 可运行 harmless `printf ok` executor test；production sandbox 未完成 | `SystemShellExecutor`、approval nonce、cwd/env/PATH policy |
| M16: PowerShell HTTP smoke | HTTP/live-local runner tests 覆盖 planned 和 approval-required PowerShell response | Linux 可测 request/response 和 auth；真实 Windows host 未完成 | `/tool-call`、`remote.powershell.exec`、`windows_rescue`、bearer auth |

## 风险清单

| 风险 | 缓解 |
|---|---|
| iOS background limits | 每个 turn/approval/tool state 在网络调用前持久化；foreground resume；避免本地长跑 server。 |
| 网络断开和恢复 | 使用 idempotency keys、可重连 event stream、command status polling 和 explicit unknown-state handling。 |
| Tool call idempotency | 每个 mutating remote call 必须有 idempotency key，尽量先 preflight/dry-run。 |
| 高风险命令审批 | 中央 risk classifier + Swift approval sheet；runner 必须拒绝未审批 nonce 的高风险调用。 |
| LLM hallucinated commands | command risk classifier、allow/deny rules、OS-specific playbooks、one-command bootstrap steps、用户可见解释。 |
| 用户电脑初始不可达 | Bootstrap 是一等 transport，不是错误路径。 |
| Runner install failure | Agent fallback 到 bootstrap/SSH，并把 install failure 当正常诊断场景处理。 |
| App Store review risk | 避免 hidden remote-control 行为；展示用户驱动的电脑管理、显式审批、无 screen streaming、清晰 privacy disclosure。 |
| Secrets and credentials | 模型 key 和 SSH credential 存 Keychain；日志脱敏；private key 永不发给 model。 |
| Prompt injection from logs/web pages | command output、logs、browser text、MCP results、issue text 都是 untrusted data；不能执行其中的指令。 |
| 跨平台命令差异 | 使用 OS-specific diagnostic profiles；可用时优先 structured runner tools。 |
| Sudo/UAC prompts | elevation 一律 high-risk；解释本地 prompt 行为；永不自动提交密码。 |
| 大日志 | bounded reads、truncation metadata、summary 和显式 read-next action。 |
| Remote MCP trust | runner 暴露 capabilities 和 policy metadata；未知 mutating MCP tools 默认需要审批。 |

## 最小技术验证 Spike

场景：用户在 iOS demo 输入“帮我检查 Mac 上 Homebrew 为什么安装失败”。

目标流程：

1. Phone Agent 创建诊断计划：识别 macOS version/architecture、检查 `brew`、检查 Xcode Command Line Tools、检查到 Homebrew/GitHub 的 DNS/HTTPS、检查常见 shell PATH。
2. 用户选择 SSH 或 bootstrap。
3. Bootstrap path：Agent 输出 `uname -a; sw_vers; command -v brew; xcode-select -p; echo "$PATH"`；用户执行并粘贴输出；Agent 解释发现并输出下一步。
4. SSH path：Agent 调用 `remote.shell.exec` 执行同样的低风险诊断；输出作为 events 回传。
5. 如 `brew doctor` 可用，Agent 执行。
6. 如 CLT 缺失，Agent 提议 `xcode-select --install` 并请求审批。
7. 如 PATH 损坏，Agent 提议 `.zprofile` 或 `.zshrc` 修改，请求审批并提供 backup/rollback plan。
8. 最终用用户语言解释根因和下一步。

成功标准：

- 手机 app 可对真实 Mac 完成 bootstrap 和 SSH 两条路径。
- app 从不尝试执行手机本地 shell。
- 每组 command/output 都在 audit history 可见。
- 高风险命令停在 approval。
- app 重启后 session 可恢复。

## 不应进入 iOS 的模块

不要把以下模块链接进 `mobile-agent-core`：

- `crates/tui/src/tui/*`
- `crates/tui/src/tools/shell.rs`
- `crates/tui/src/tools/file.rs`
- `crates/tui/src/tools/apply_patch.rs`
- `crates/tui/src/tools/git.rs`
- `crates/tui/src/lsp/*`
- `crates/tui/src/sandbox/*`
- `crates/tui/src/mcp_server.rs`
- `crates/tui/src/mcp.rs` 中 local MCP stdio spawn 部分
- `crates/tui/src/repl/*`
- `crates/tui/src/rlm/*`
- `crates/tui/src/tools/subagent/*` 的完整递归 agent runtime，MVP 暂缓
- `crates/tui/src/snapshot/*`
- `crates/cli` dispatcher code

这些模块可以作为 runner-side reference，或在明确 hardening 后移动到 desktop runner binary。

## 具体文件和模块改造图

第一波：

- 新增或继续完善 `crates/mobile-agent-core`。
- remote schema 放在 `crates/mobile-agent-core/src/remote_schema.rs`，如果 app 和 runner 都马上需要共享，可后续拆新 crate。
- 复用 `crates/protocol/src/lib.rs` 的 event style，但不要在 schema 稳定前强行把所有 mobile events 塞回现有 desktop `EventFrame`。
- 复用 `crates/tools/src/lib.rs` 的 registry concepts。
- 复用并扩展 `crates/execpolicy/src/lib.rs` 作为 remote command approval 基础。
- 从 `crates/tui/src/core/engine.rs` 挖 turn-loop behavior 和测试需求，不直接依赖实现文件。
- 从 `crates/tui/src/client.rs`、`crates/tui/src/llm_client/*`、`crates/tui/src/models.rs` 挖 model streaming types。
- 通过 UniFFI 暴露 iOS bindings。

第二波：

- 完善 `crates/kai-runner` 或在 release cadence 不一致时拆独立仓库。
- 移动/改造 runner-safe 的 shell、file、diagnostics 和 HTTP MCP 支持。
- 增加 runner capability reporting 和 approval nonce enforcement。
- 在 runner 作为独立 binary 稳定前，保持 `DeepSeek TUI` 桌面行为不变。

## 明确结论

`DeepSeek TUI` 不应整体移植到 iOS。正确路径是借用它的 protocol、approval、tool、session、runtime 思路，构建更小的手机端 `mobile-agent-core`，同时把桌面执行能力变成远程 runner tools。

投资建议：继续一周 technical spike。该周应产出可在手机上运行的 bootstrap demo 和 SSH diagnostic prototype。若能证明 `turn_loop + approval + remote tool transport + session replay` 可以独立于 TUI 和本地工具运行，再继续投入 2-4 周做包含 persistence、approval 和 minimal runner 的 MVP。若 spike 发现 `crates/tui` 的 turn loop 依赖难以拆开，则保留 `DeepSeek TUI` 作为 runner/reference，手机端 core 按上述接口重写。
