# deepseek-tui 手机端 Agent Core 移植可行性分析与改造设计

本文档基于当前代码库的真实结构，评估将 `deepseek-tui` 拆成可在 iOS 上运行的轻量 Agent Core 与桌面/服务器/runner 执行层的可行性。目标不是复用终端 UI，而是复用 Agent loop、会话状态、审批、工具调度、LLM 云端调用和远程工具协议。

结论：当前 `deepseek-tui` 不适合直接作为 iOS core。workspace 已经有 `crates/protocol`、`crates/tools`、`crates/execpolicy`、`crates/state`、`crates/core` 等拆分雏形，但真实的生产 Agent loop 仍主要在 `crates/tui/src/core/engine*`，并强依赖本地 shell、PTY、文件系统、LSP、MCP stdio、TUI 事件和桌面快照。推荐路径是先抽象工具执行边界，再新建 `mobile-agent-core`，复用协议/模型/审批思想，避免把 `crates/tui` 整体搬进 iOS。

## 1. 当前架构梳理

当前 workspace 结构：

- `crates/cli`：`deepseek` dispatcher，负责命令入口和分发到 TUI/runtime/MCP 等模式。
- `crates/tui`：当前生产运行时主体。包含 TUI、Agent engine、DeepSeek client、工具实现、MCP、LSP、runtime API、会话快照、任务、子代理、RLM 等。
- `crates/protocol`：线程、工具 payload/output、事件、审批请求等可序列化协议类型。
- `crates/tools`：较新的共享工具调用抽象，包括 `ToolRegistry`、`ToolHandler`、`ToolCall`、`ToolResult`、并行执行约束。
- `crates/execpolicy`：命令审批策略、allow/deny 前缀规则、approval requirement。
- `crates/state`：SQLite thread/message/checkpoint/job 持久化。
- `crates/core`：较新的 runtime boundary，但当前更像 app-server/runtime scaffold；`handle_prompt` 目前只做模型解析和持久化 payload，不是 `crates/tui` 中完整流式 Agent loop。
- `crates/mcp`：较新的 MCP manager 抽象，主要是 in-memory/管理接口；完整 async stdio/HTTP MCP 实现在 `crates/tui/src/mcp.rs`。
- `crates/app-server`：HTTP/stdio JSON-RPC wrapper，持有 `deepseek_core::Runtime`。
- `crates/secrets`：OS keyring/file fallback。
- `crates/tui-core`：TUI 状态机 scaffold。

当前文本架构图：

```text
deepseek CLI dispatcher (crates/cli)
    |
    +--> interactive runtime (crates/tui)
    |       |
    |       +--> TUI UI: ratatui/crossterm, approval modal, input handling
    |       |
    |       +--> Engine: crates/tui/src/core/engine.rs
    |       |       +--> turn loop: core/engine/turn_loop.rs
    |       |       +--> approval wait: core/engine/approval.rs
    |       |       +--> tool dispatch: core/engine/tool_execution.rs
    |       |       +--> session: core/session.rs
    |       |       +--> events: core/events.rs
    |       |
    |       +--> LLM client: client.rs + llm_client/mod.rs
    |       |
    |       +--> Tool registry: tools/registry.rs
    |       |       +--> shell/PTY: tools/shell.rs
    |       |       +--> file/edit/search/git: tools/file.rs, search.rs, git.rs
    |       |       +--> web/fetch/finance: tools/web_*.rs, fetch_url.rs
    |       |       +--> tasks/subagents/RLM/review/automation/github
    |       |
    |       +--> Local execution dependencies
    |               +--> std::process / portable-pty
    |               +--> local filesystem
    |               +--> local MCP stdio process spawn
    |               +--> local LSP server stdio
    |               +--> macOS/Linux/Windows sandbox backends
    |
    +--> runtime API (crates/tui/src/runtime_api.rs)
    |       +--> HTTP/SSE local server, approvals endpoint, thread/task APIs
    |
    +--> MCP server/client modes
            +--> stdio server and local/HTTP MCP clients

new split crates (partial, not production source of truth yet)
    |
    +--> crates/protocol: serializable frames and event schema
    +--> crates/tools: generic tool dispatch abstraction
    +--> crates/execpolicy: approval policy engine
    +--> crates/state: SQLite persistence
    +--> crates/core: runtime boundary scaffold
    +--> crates/app-server: HTTP/stdio wrapper over crates/core
```

关键文件：

- Agent loop/session/conversation/planning：`crates/tui/src/core/engine.rs`、`core/engine/turn_loop.rs`、`core/session.rs`、`tools/plan.rs`、`tools/todo.rs`。
- 协议/事件/tool schema：`crates/protocol/src/lib.rs`、`crates/tui/src/core/events.rs`、`crates/tui/src/models.rs`、`crates/tui/src/tools/spec.rs`。
- exec policy/approval：`crates/execpolicy/src/lib.rs`、`crates/tui/src/core/engine/approval.rs`、`crates/tui/src/tools/approval_cache.rs`、`crates/tui/src/runtime_api.rs` 的 `/v1/approvals/{approval_id}`。
- MCP：`crates/tui/src/mcp.rs`、`crates/mcp/src/lib.rs`、`crates/tui/src/mcp_server.rs`。
- app-server/HTTP/SSE：`crates/tui/src/runtime_api.rs`、`crates/app-server/src/lib.rs`。
- state/persistence：`crates/state/src/lib.rs`、`crates/tui/src/runtime_threads.rs`、`crates/tui/src/session_manager.rs`、`crates/tui/src/snapshot/*`。
- tools：`crates/tui/src/tools/registry.rs`、`shell.rs`、`file.rs`、`apply_patch.rs`、`git.rs`、`web_run.rs`、`fetch_url.rs`、`diagnostics.rs`、`subagent/*`、`rlm.rs`。
- TUI/CLI：`crates/tui/src/main.rs`、`crates/tui/src/tui/*`、`crates/cli/src/*`。

## 2. iOS 可移植性分级

| 模块 | 分级 | 原因 | 处理建议 |
|---|---|---|---|
| `crates/protocol` | Portable | 主要是 `serde` 类型，包含 thread/event/tool/approval frame；仅 `PathBuf` 需要跨端语义收敛 | 复用并增加 mobile/remote tool 事件 |
| `crates/execpolicy` | Portable | 纯 Rust 策略判断，适合在手机端决定是否需要审批 | 改名/泛化为 tool risk policy，不只判断 shell prefix |
| `crates/agent` model registry | Portable | 模型/provider 解析逻辑轻量 | 复用，避免桌面配置路径假设 |
| `crates/tools` 抽象 | Needs Adapter | `ToolRegistry`/`ToolHandler` 可用，但 `ToolPayload::LocalShell` 偏本地执行 | 增加 `RemoteTool` payload 或把 transport 注入 handler |
| `crates/state` | Needs Adapter | SQLite 模型适合 iOS，但当前使用 `dirs`、本地路径、同步 `rusqlite` | 抽出 `Persistence` trait；iOS 实现用 app sandbox SQLite |
| `crates/secrets` | Needs Adapter | iOS 应使用 Keychain；当前 macOS/Linux/Windows keyring cfg 不覆盖 iOS | 增加 iOS Keychain backend 或由 Swift 层注入 token |
| `crates/tui/src/client.rs` / `llm_client` | Needs Adapter | `reqwest`/SSE 逻辑可移植性取决于 iOS TLS/runtime；API 形状有价值 | 抽成 `ModelClient` trait；iOS 可用 Rust reqwest/rustls 或 Swift URLSession adapter |
| `crates/tui/src/core/session.rs` | Needs Adapter | 会话字段有价值，但含 workspace、project context、approval mode、working set 等桌面语义 | 拆出 mobile `SessionState`，workspace 改为 remote target context |
| `crates/tui/src/core/engine/turn_loop.rs` | Needs Adapter | 是核心 Agent loop，但耦合 LSP、capacity、compaction、TUI events、local tool registry | 移植算法，不搬文件；先抽 `ToolExecutor`、`EventSink`、`ModelClient` |
| `crates/tui/src/core/events.rs` | Needs Adapter | 事件概念好，但有 `PauseEvents/ResumeEvents`、TUI-only、subagent UI payload | 在 `protocol` 中定义 mobile-safe event stream |
| `crates/tui/src/tools/plan.rs` / `todo.rs` | Portable | 内存状态和 schema 轻量，可用于手机端规划/清单 | 移到 core 或重写为 core planner state |
| `crates/tui/src/runtime_api.rs` | Desktop Only for iOS app; useful for runner | 是本地 HTTP/SSE server，绑定端口，调用本地 runtime 和本地文件/git | iOS 不内嵌 server；可作为电脑 runner API 参考 |
| `crates/app-server` | Desktop/Server Only | axum server/stdio JSON-RPC wrapper，适合作 headless desktop process | 可演化为 runner，不进 iOS |
| `crates/tui/src/tools/shell.rs` | Desktop Only | `std::process`、PTY、process group、sandbox、stdin、background job | 手机端只能通过 remote transport 调用 |
| `crates/tui/src/tools/file.rs` / `apply_patch.rs` / `search.rs` | Desktop Only for target files | 直接访问本地工作区和外部命令 OCR/PDF fallback | 逻辑可在 runner 侧保留；手机端只持 schema/stubs |
| `crates/tui/src/tools/git.rs` / `github.rs` | Desktop Only for git; Needs Adapter for GitHub API | git 工具 shell out 到 `git`/`gh`，依赖本机 repo | runner 侧执行；GitHub API 可后续云端/手机直连但不是 MVP |
| `crates/tui/src/lsp/*` | Desktop Only | LSP server stdio spawn，不适合 iOS | runner 侧可提供 `remote.diagnose.*` 或 remote MCP |
| `crates/tui/src/mcp.rs` stdio | Desktop Only | stdio server spawn 和本地 config 文件 | 手机端只支持 MCP over HTTP/SSE 或通过 runner 代理 |
| `crates/tui/src/mcp.rs` HTTP transport 思路 | Needs Adapter | HTTP MCP transport 方向适合手机，但当前代码在 TUI crate 且含本地 config/network approval | 抽到 `remote-mcp-client`，去掉本地 spawn |
| `crates/tui/src/tui/*`、`ratatui/crossterm` | Desktop Only | 终端 UI、raw mode、alt screen、keyboard | 不进 mobile core |
| `crates/tui/src/repl/*`、`rlm/*` | Desktop Only / Later | Python REPL、本地大上下文工具、子模型调用复杂 | MVP 不进手机；可由云端/runner 代理 |
| `crates/tui/src/sandbox/*` | Desktop Only | macOS Seatbelt/Linux Landlock/Windows helper 语义 | runner 侧执行；手机端只表达 policy intent |
| `crates/tui/src/snapshot/*` | Desktop Only | side-git workspace 快照依赖桌面文件系统和 git | runner 侧可保留；手机端只记录 checkpoint metadata |
| `crates/tui/src/tools/web_run.rs` / browser automation | Desktop Only for click/open | 手机端不应控制桌面截图流；无本地浏览器自动化 | 通过 runner/browser service 暴露结构化 browser tools |

## 3. 推荐目标架构

目标架构：

```text
iOS App
    |
    v
mobile-agent-core
    |
    +--> model client
    |       +--> DeepSeek/OpenAI-compatible cloud API
    |
    +--> approval gate
    |       +--> Swift UI approval sheet / notification / biometric optional
    |
    +--> remote tool transport
            |
            +--> SSH transport
            +--> bootstrap manual guide transport
            +--> HTTPS/WebSocket kai-runner transport
            +--> MCP over HTTP/SSE transport
                    |
                    v
            target computer
                +--> bootstrap script/session
                +--> kai-runner lightweight daemon
                +--> remote MCP servers
                +--> shell/file/git/browser/LSP execution
```

职责划分：

- iOS App：Swift/SwiftUI UI、输入框、会话列表、审批弹窗、远程目标配置、Keychain、后台/前台生命周期、网络权限说明。
- `mobile-agent-core`：纯 Agent runtime。负责会话、消息、规划、工具调用决策、审批 gate、事件流、重试、持久化抽象、远程工具 schema。它不知道 UIKit/SwiftUI，也不直接碰本地 shell。
- model client：通过云端 API 进行推理，支持流式 response、thinking block、tool calls、reasoning replay。手机只做网络调用，不做本地大模型推理。
- approval gate：统一处理高风险 tool call，包括命令执行、写文件、安装包、浏览器点击、MCP mutation。审批结果进入 core，core 再调 transport。
- remote tool transport：把 model tool call 转成远程请求。初期支持 SSH 和手动 bootstrap；稳定后优先 runner/WebSocket 或 MCP over HTTP/SSE。
- SSH/bootstrap/kai-runner/remote MCP：电脑侧执行层。负责真实 shell、文件、git、browser、MCP stdio、LSP、sandbox、日志裁剪和 capability discovery。
- target computer：用户 Mac/Linux/Windows。所有危险和平台相关操作发生在这里。

## 4. `mobile-agent-core` 边界

手机端 core 应包含：

- Agent loop：从 user message 到 model stream、tool call、tool result、final answer 的循环。
- session state：thread id、turn id、message history、model、remote target、capabilities、pending approvals。
- message history：兼容 DeepSeek thinking/tool-call replay，保留 reasoning content。
- planner：`update_plan`/checklist 类型的轻量 planning state。
- tool call dispatcher：只分发到 remote executor，不执行本地系统命令。
- approval gate：根据风险等级、policy、session approvals 决定是否阻塞等待用户。
- remote tool schema：手机端向模型暴露 `remote.*` 工具，而不是 `exec_shell/read_file/write_file`。
- event stream model：response delta、thinking delta、tool start/result、approval required、transport status、resume/reconnect。
- persistence abstraction：`PersistenceStore` trait，iOS 实现落 SQLite；测试可用 in-memory。
- model client abstraction：`ModelClient` trait，允许 Rust reqwest 或 Swift URLSession 实现。
- transport abstraction：`RemoteToolTransport` trait，允许 SSH、runner、MCP HTTP、manual bootstrap。

手机端 core 不应包含：

- local shell / `std::process::Command`
- local file editing / workspace filesystem traversal
- PTY / `portable-pty`
- TUI / `ratatui` / `crossterm` / raw terminal
- local LSP server
- local MCP stdio spawn
- desktop sandbox implementation
- local git snapshot side-repo
- local browser automation or screenshots
- local Python/RLM REPL
- `deepseek serve --http` 这类本机 server

建议新增/调整文件：

- 新增 `crates/mobile-agent-core/Cargo.toml`。
- 新增 `crates/mobile-agent-core/src/lib.rs`、`engine.rs`、`session.rs`、`events.rs`、`approval.rs`、`model.rs`、`persistence.rs`、`remote_tools.rs`、`transport.rs`。
- 修改 `crates/protocol/src/lib.rs`：加入 mobile-safe remote event/tool payload，或新建 `crates/remote-protocol` 避免污染现有桌面协议。
- 修改 `crates/tools/src/lib.rs`：将 `ToolPayload::LocalShell` 的本地语义从 core registry 中剥离，或新增 `ToolPayload::Remote { name, arguments }`。
- 暂不移动 `crates/tui/src/core/engine.rs`；先从中抽接口和测试用例。

## 5. 远程工具接口设计

统一字段建议：

```json
{
  "target_id": "macbook-pro",
  "cwd": "/Users/me/project",
  "timeout_ms": 60000,
  "idempotency_key": "turn-tool-call-uuid"
}
```

统一输出建议：

```json
{
  "ok": true,
  "status": "completed",
  "started_at": "2026-05-22T10:00:00Z",
  "duration_ms": 1234,
  "truncated": false,
  "artifact_ref": null
}
```

| Tool | 输入 schema 摘要 | 输出 schema 摘要 | 风险 | 审批 |
|---|---|---|---|---|
| `remote.shell.exec` | `{target_id, command, cwd?, timeout_ms?, env?, stdin?, capture_limit_bytes?, idempotency_key?}` | `{ok,status,exit_code,stdout,stderr,duration_ms,truncated,artifact_ref?}` | High | 默认需要；只读 allowlist 如 `pwd`, `brew doctor`, `git status` 可降为 Medium/Auto |
| `remote.powershell.exec` | `{target_id, command, cwd?, timeout_ms?, execution_policy?, capture_limit_bytes?, idempotency_key?}` | 同 shell，另含 `{powershell_version?}` | High | 默认需要 |
| `remote.file.read` | `{target_id, path, start_line?, max_lines?, encoding?, follow_symlinks?}` | `{ok,path,content,total_lines?,shown_range?,truncated,next_start_line?,sha256?}` | Low/Medium | workspace 内只读通常不需要；敏感路径需要审批 |
| `remote.file.write` | `{target_id, path, content, mode:"overwrite|append|create_new", expected_sha256?, create_dirs?, diff_preview?}` | `{ok,path,bytes_written,old_sha256?,new_sha256,diff?,backup_ref?}` | High | 必须审批 |
| `remote.diagnose.system` | `{target_id, checks:["os","shell","xcode","brew","network","disk","git"], cwd?}` | `{ok,platform,checks:[{name,ok,summary,detail,raw_ref?}]}` | Medium | 自动或建议审批；不应修改系统 |
| `remote.package.install` | `{target_id, manager:"brew|apt|winget|npm|pip|cargo", packages:[...], version?, dry_run?, cwd?}` | `{ok,manager,packages,exit_code,stdout,stderr,changed}` | Critical | 必须审批；推荐先 dry run |
| `remote.browser.open` | `{target_id, url, browser?, profile?, purpose?}` | `{ok,session_id?,url,title?}` | Medium | 外部 URL 建议审批；本地文档可自动 |
| `remote.browser.extract_text` | `{target_id, url?, session_id?, selector?, max_chars?, wait_ms?}` | `{ok,url,title,text,truncated,links?}` | Low/Medium | 通常自动；登录态/内网域名建议审批 |
| `remote.browser.click` | `{target_id, session_id, selector?, text?, x?, y?, require_visible?, purpose}` | `{ok,url,title,observed_text?,screenshot_ref?}` | High | 必须审批；避免截图流，截图仅作为 artifact 可选 |
| `remote.bootstrap.guide` | `{target_id?, os_hint?, desired_transport:"ssh|runner|mcp", user_can_copy_paste:boolean}` | `{ok,steps:[...],commands:[{label,command,risk}], verification_challenge?}` | Medium | 生成安装/诊断命令前建议审批；执行由用户手动完成 |
| `remote.mcp.call` | `{target_id, server, tool, arguments, timeout_ms?, idempotency_key?}` | `{ok,result,server,tool,duration_ms}` | Dynamic | 读工具可自动；未知/写工具必须审批 |

风险等级定义：

- Low：只读、范围受限、不会泄露敏感信息或修改远端。
- Medium：只读但可能访问敏感路径/网络/登录态，或产生外部可见行为。
- High：执行命令、写文件、点击浏览器、修改系统或项目。
- Critical：安装软件、删除/重置、上传密钥、改权限、远端持久化 daemon。

`remote.shell.exec` 不应暴露为“无限制 bash”。手机端 policy 应把命令、cwd、target、风险解释、LLM 理由一起展示给用户。runner 侧也必须二次校验，不信任手机端。

## 6. 通信方式建议

| 方案 | 适用场景 | 优点 | 缺点 | 建议 |
|---|---|---|---|---|
| SSH | 用户已有 Mac/Linux 账号、局域网/VPN/公网可达、MVP 诊断 | 无需预装 runner；概念成熟；可 bootstrap | iOS SSH key 管理复杂；网络可达性差；Windows 支持不稳定；流式/取消/文件传输需要封装 | MVP 支持，作为 bootstrap 和 power-user 模式 |
| HTTPS/WebSocket runner | 长期主路径；电脑已安装 `kai-runner` 或轻量 daemon | 可做能力发现、审计、流式日志、取消、重连、双向审批、细粒度 sandbox | 需要安装和升级；认证/配对/防火墙复杂 | 推荐作为主架构 |
| MCP over HTTP/SSE | 用户已有 MCP 生态或 runner 想暴露标准工具 | 标准化 tool/resource/prompt；避免 stdio spawn 在手机端 | MCP auth/approval/能力风险仍需外层策略；很多 MCP server 仍只能本地 stdio | 作为 runner 后面的工具总线，手机端只连 HTTP/SSE MCP |
| 手动 bootstrap 模式 | 电脑初始不可达、无 SSH、无 runner | 不要求预安装；用户复制命令即可开始 | 不是自动执行；容易复制错误；安全提示必须清晰 | 必须支持，用于首次安装/故障恢复 |

推荐顺序：

1. MVP：SSH + manual bootstrap。
2. 早期稳定版：kai-runner HTTPS/WebSocket，runner 内部可调用本地 shell/file/git/MCP stdio。
3. 扩展生态：runner 暴露 MCP over HTTP/SSE，iOS core 将 `remote.mcp.call` 当普通 remote tool。

## 7. iOS 集成方式

可选方案：

| 方案 | 优点 | 缺点 | 适配点 |
|---|---|---|---|
| Rust `staticlib` + Swift FFI | 最少魔法；可精细控制 ABI；适合稳定小接口 | 手写 C ABI 成本高；复杂类型/async stream 麻烦 | 适合极小 core API，不适合大量事件和 schema |
| UniFFI | Rust 类型/async/错误导出到 Swift 更自然；减少 FFI 样板 | 需要维护 UDL/生成流程；某些 stream/callback 仍需设计 | 推荐用于 `mobile-agent-core` |
| Flutter/React Native native module | 跨平台 UI 速度快 | 会多一层 runtime；iOS 原生 Keychain/后台/network 仍需桥接 | 如果产品目标是跨平台，可在 UniFFI 外再包一层 |

推荐方案：

- `mobile-agent-core` 用 Rust library + UniFFI 导出 Swift API。
- Swift 负责 UI、Keychain、文件选择器、Local Network 权限说明、Push/BackgroundTask、URLSession 可选 adapter。
- Rust core 负责 deterministic agent state machine、tool policy、消息和事件 schema。
- SQLite：优先由 Rust `rusqlite` 使用 app container 路径；如果 iOS 交叉编译或加密需求复杂，则定义 `PersistenceStore` trait，由 Swift/GRDB 或 SQLCipher 实现。
- Keychain：不要把 API key 写入 Rust 文件 fallback。Swift 从 Keychain 取 token，在创建 `ModelClientConfig`/`TransportConfig` 时注入短生命周期 secret。
- 网络权限：iOS App 需要明确声明网络访问；局域网 runner 发现可能触发 Local Network permission。后台长任务应设计为可暂停/恢复，而不是假设后台无限运行。

FFI API 草案：

```text
MobileAgentCore.create(config) -> CoreHandle
CoreHandle.start_thread(params) -> ThreadId
CoreHandle.send_user_message(thread_id, text) -> AsyncEventStream
CoreHandle.submit_approval(approval_id, decision)
CoreHandle.configure_target(target_config)
CoreHandle.resume_pending_turn(thread_id)
CoreHandle.export_thread(thread_id) -> JSON
```

## 8. MVP 改造计划

2-4 周 MVP，目标是验证手机端 Agent core 能控制远端 Mac 做 Homebrew 诊断。

### Phase 1：抽象 tool executor

范围：

- 在 `crates/tui/src/core/engine/tool_execution.rs` 和 `crates/tui/src/tools/registry.rs` 周围抽出 `ToolExecutor` / `ToolCatalog` / `ToolPolicy` trait。
- 将本地工具执行从 Agent loop 中隔离，避免 loop 直接知道 shell/file/MCP stdio。
- 在 `crates/protocol` 或新 crate 中定义 mobile-safe `ToolCallRequest`、`ToolCallResult`、`ToolRisk`。

需要改的文件：

- `crates/tui/src/core/engine/tool_execution.rs`
- `crates/tui/src/core/engine/turn_loop.rs`
- `crates/tui/src/tools/spec.rs`
- `crates/tui/src/tools/registry.rs`
- `crates/protocol/src/lib.rs` 或新 `crates/remote-protocol`

验收：

- 桌面现有 TUI 行为不变。
- 测试中可用 fake executor 接管一个 tool call。

### Phase 2：禁用本地 shell/file/LSP/TUI，替换为 remote tool

范围：

- 增加 remote tool catalog，只向模型暴露 `remote.*`。
- 手机模式下不注册 `with_shell_tools()`、`with_file_tools()`、`with_patch_tools()`、`with_git_tools()`、`with_mcp_tools()`、LSP hooks、snapshot。
- 把本地 `exec_shell/read_file/write_file` prompt guidance 改为 remote 语义。

需要改的文件：

- `crates/tui/src/tools/registry.rs`
- `crates/tui/src/prompts/agent.txt`
- `crates/tui/src/prompts/base*.md/txt`
- `crates/tui/src/core/engine/lsp_hooks.rs`
- `crates/tui/src/core/engine.rs`

验收：

- mobile profile 下 tool catalog 不包含本地执行工具。
- 所有 tool call 经过 `RemoteToolTransport` fake。

### Phase 3：实现 `mobile-agent-core` crate

范围：

- 新 crate：`crates/mobile-agent-core`。
- 搬入/重写最小 engine：session、message history、model stream、tool dispatcher、approval gate、event stream。
- 依赖尽量限于 `serde`、`serde_json`、`tokio` 可选、`uuid`、`chrono`、`deepseek-protocol`、`deepseek-execpolicy`。
- 不依赖 `ratatui`、`crossterm`、`portable-pty`、`std::process`、`axum` server、LSP、sandbox。

需要改的文件：

- workspace `Cargo.toml`
- 新增 `crates/mobile-agent-core/*`
- 可能新增 `crates/remote-protocol/*`

验收：

- macOS host 上可跑 core unit tests。
- fake model + fake transport 能完整执行 user -> tool approval -> remote result -> final response。

### Phase 4：做 iOS demo shell

范围：

- SwiftUI demo：会话输入、事件流、审批 sheet、远程目标配置。
- UniFFI 绑定 core。
- Keychain 存 API key/SSH key 或 runner token。
- 本地 SQLite 或 in-memory persistence。

验收：

- iOS simulator 或真机可输入 prompt，看到 model stream 和 pending approval。
- fake transport 可演示全流程。

### Phase 5：接入 `kai-runner` / remote MCP

范围：

- runner 提供 HTTPS/WebSocket API：pairing、capabilities、tool call、cancel、log stream。
- runner 内部复用桌面工具实现：shell/file/git/MCP stdio/LSP。
- iOS core 增加 runner transport；后续支持 MCP over HTTP/SSE。

验收：

- 真机通过 runner 对 Mac 执行 `remote.diagnose.system` 和 `remote.shell.exec("brew doctor")`。
- 高风险命令必须弹审批。

## 9. 风险清单

- iOS 后台限制：App 进入后台后网络和 CPU 时间有限。长 turn 必须可暂停、恢复、重连；runner 侧应持久化 tool job。
- 网络断开与恢复：tool call 需要 idempotency key；event stream 需要 sequence/replay；SSH 断线要能重新 attach 或报告 job unknown。
- tool 调用幂等性：写文件、安装包、点击浏览器都不是天然幂等。每个 request 带 `idempotency_key`，runner 记录已执行结果。
- 高风险命令审批：手机端和 runner 侧都要校验。不能只信任 LLM 的风险解释。
- LLM 幻觉命令：policy 需要识别 `rm -rf`、`sudo`、`curl | sh`、权限修改、密钥读取、上传等模式，并要求明确用户确认。
- 用户电脑初始不可达：提供 manual bootstrap guide；不要把“无法连接”当作 Agent 失败。
- runner 安装失败：bootstrap 需要诊断路径：shell 类型、PATH、权限、Homebrew/Xcode、网络、杀软/防火墙。
- App Store 审核风险：App 不能表现为绕过 iOS sandbox 在手机本机执行代码；文案和实现应明确远端用户自有电脑执行。避免下载执行任意移动端代码。
- 密钥与凭据存储：API key、SSH private key、runner pairing token 必须在 Keychain；日志和 tool output 要做 secret redaction。
- 远程 runner 暴露面：runner 必须默认 bind localhost 或配对认证；局域网访问必须 TLS/token；防重放。
- MCP 信任边界：远端 MCP server 是任意代码。手机端展示 server/tool 名称、参数、风险；runner 做 allowlist。
- 隐私和日志：Homebrew/git/system diagnostics 可能含用户名、路径、内网 host、token。手机端展示前应截断和标注敏感。
- 成本和上下文膨胀：远程命令输出需要 artifact/ref + summary，避免把完整日志塞进手机会话。
- 平台差异：Mac/Linux/Windows shell、PowerShell、路径和权限不同；remote schema 要显式 `platform`。

## 10. 最小技术验证 Spike

目标场景：

用户在 iOS demo 输入：“帮我检查 Mac 上 Homebrew 为什么安装失败”。

最小流程：

```text
1. iOS App -> mobile-agent-core
   用户输入 prompt，core 创建 turn。

2. core -> model client
   模型生成诊断计划：
   - 确认连接方式
   - 检查 brew doctor
   - 检查 xcode-select
   - 检查网络/DNS
   - 检查 PATH 和权限

3. core -> iOS App
   事件：需要选择 target transport。
   用户选择：
   - SSH：填 host/user/key
   - bootstrap：复制一段只读诊断命令到 Mac

4. core -> approval gate
   `remote.shell.exec` 请求：
   command = "brew doctor && xcode-select -p && brew config"
   cwd = "$HOME"
   risk = Medium
   reason = "只读诊断 Homebrew/Xcode 配置"
   用户批准。

5. core -> remote transport
   SSH 或 runner 执行：
   - `brew doctor`
   - `xcode-select -p`
   - `xcodebuild -version` 或 `clang --version`
   - `brew config`
   - `brew update --verbose` 仅在用户批准后执行，因为可能联网且耗时
   - `curl -I https://formulae.brew.sh` 或 DNS 检查

6. target computer -> core
   返回 stdout/stderr、exit code、duration、truncated/artifact_ref。

7. core -> model client
   把结构化结果作为 tool result 回传，模型解释问题。

8. core -> iOS App
   展示结论和下一步。
   如果模型建议修复命令，如：
   - `xcode-select --install`
   - `brew update-reset`
   - `sudo chown -R ...`
   - `rm -rf "$(brew --cache)"`
   必须再次生成 High/Critical approval。
```

Spike 需要实现的最小技术面：

- `mobile-agent-core` fake/real model client 二选一；若时间紧，先 fake model 固定生成 tool call。
- `RemoteToolTransport` 的 SSH 实现或 fake runner 实现。
- `remote.shell.exec` schema、risk policy、approval UI。
- event stream：`response.delta`、`tool.started`、`approval.required`、`tool.output.delta/result`、`turn.completed`。
- iOS SwiftUI demo：输入、事件列表、审批按钮、SSH/bootstrap 选择。

Spike 不做：

- 本地手机 shell。
- 本地文件编辑。
- RDP/VNC/截图流。
- 本地 MCP stdio。
- LSP。
- 完整 runner 安装器。

## 不应进入 iOS 的现有模块清单

以下模块应留在 desktop/runner/server 侧，或只作为远端服务能力暴露：

- `crates/tui/src/tools/shell.rs`
- `crates/tui/src/tools/file.rs`
- `crates/tui/src/tools/apply_patch.rs`
- `crates/tui/src/tools/git.rs`
- `crates/tui/src/tools/diagnostics.rs`
- `crates/tui/src/tools/test_runner.rs`
- `crates/tui/src/lsp/*`
- `crates/tui/src/mcp.rs` 的 stdio spawn 部分
- `crates/tui/src/mcp_server.rs`
- `crates/tui/src/runtime_api.rs` 的 local server
- `crates/tui/src/tui/*`
- `crates/tui/src/repl/*`
- `crates/tui/src/rlm/*`
- `crates/tui/src/sandbox/*`
- `crates/tui/src/snapshot/*`
- `crates/tui/src/tools/subagent/*` 的完整递归 agent runtime，MVP 暂缓

## 推荐替代方案

如果目标是快速拥有手机端 Agent Core，不建议从 `crates/tui` 做“减法式移植”。更稳的方案是：

1. 以 `crates/protocol`、`crates/execpolicy`、`crates/tools` 的抽象为基础，新建 `mobile-agent-core`。
2. 从 `crates/tui/src/core/engine/turn_loop.rs` 复制行为级测试和协议需求，而不是复制实现文件。
3. 把 `crates/tui` 继续作为 desktop runner/reference runtime。
4. 新建 `kai-runner` 或扩展现有 runtime API，提供手机可调用的远程工具执行层。

这样可以避免把 iOS 明确不支持的 shell/PTY/TUI/stdio/LSP/sandbox 依赖拖进 mobile binary，同时保留 deepseek-tui 已经验证过的 Agent 行为和工具安全经验。

## Original Product Requirements

原始产品不是“把 Rust TUI 编译到 iOS”，而是做一个手机端电脑救援 Agent。核心价值是：当用户电脑没有准备好、环境损坏、安装器失败、命令行工具缺失、浏览器下载失败、权限/网络/证书/包管理器异常时，手机端仍然可以稳定承担规划、对话、审批、状态管理和下一步指导。

产品前提：

- 电脑一开始可能无法运行复杂 Agent，也可能没有 Node/Bun/Go/Rust/Python。
- 电脑可能无法安装目标软件，甚至无法联网。
- 用户可能只能打开终端，手动执行一两条命令。
- 手机端必须轻量、干净、可靠，不依赖电脑端已安装完整运行时。
- 电脑端能力必须能从“无 Agent”逐步升级到“SSH/PowerShell 可执行”，再升级到“轻量 runner”。
- 整个过程以文本、命令、日志和结构化结果为核心，不把 RDP/VNC/截图/画面流作为核心方案。

这个定位改变了对 deepseek-tui 的判断标准。deepseek-tui 的桌面工具能力本身很强，但它默认假设 Agent 与工具执行在同一台机器上；我们的产品要求 Agent 先在手机上独立成立，再把电脑视为一个能力逐步增长的远端 target。

## Phone-to-Computer Control Model

手机控制电脑应采用“手机规划，电脑执行”的严格分离模型：

```text
Phone
  iOS App
    - user conversation
    - approvals
    - target setup
    - OCR/text paste intake
    - keychain
  mobile-agent-core
    - agent loop
    - session history
    - diagnostic planning
    - tool risk policy
    - remote tool dispatcher
    - event stream
    - persistence

Computer
  no-agent state
    - user manually runs commands
    - user returns text/OCR output
  SSH/PowerShell state
    - remote command execution
    - stdout/stderr stream
    - basic file/log read
  runner state
    - kai-runner
    - capabilities discovery
    - shell/file/git/browser/MCP/LSP adapters
    - audit log and idempotency
```

控制逻辑：

- 手机端永远是 Agent owner。LLM 推理、规划、会话状态、审批上下文和用户解释都在手机端。
- 电脑端是 tool target。它只执行明确的工具调用，不能自行扩大权限或替用户批准动作。
- bootstrap 模式不是降级体验，而是最低保底能力。它要能在完全没有 runner 的电脑上推动诊断。
- runner 是能力增强层，不是大模型宿主。`kai-runner` 不运行本地大模型，只执行工具、采集日志、做浏览器/文件/系统适配。
- 所有模式共用同一套 tool risk policy、审计记录和 session timeline，避免用户从手动模式升级到 runner 后丢上下文。

deepseek-tui 的 `crates/tui/src/core/engine/turn_loop.rs`、`crates/tui/src/core/session.rs`、`crates/tui/src/core/events.rs`、`crates/execpolicy/src/lib.rs` 能提供这个模型的参考；`crates/tui/src/tools/shell.rs`、`file.rs`、`mcp.rs`、`lsp/*` 则必须放到电脑端 runner 或被 remote tool 替代。

## Execution Modes

### 模式 1：Bootstrap 手动引导模式

当电脑无法被远程连接时，手机 Agent 需要生成一步一步的诊断命令，用户在电脑终端手动执行，再把输出复制/粘贴或拍照 OCR 成文本回传手机。

流程：

```text
user describes issue on phone
  -> mobile-agent-core creates diagnostic plan
  -> model proposes one low-risk command
  -> approval/risk explanation shown on phone
  -> user manually runs command on computer
  -> user pastes/OCRs output into phone
  -> agent parses output
  -> next command or explanation
  -> repeat until SSH/runner/target software becomes installable
```

需要新增：

- `bootstrap_session` state：记录每一步命令、用户回传文本、判断结果和下一步。
- `manual_output_ingest`：接收复制文本/OCR 文本，标注来源和置信度。
- `remote.bootstrap.guide`：生成命令时必须给出风险、用途、预期输出和失败时的回传要求。
- 命令模板库：macOS Homebrew/Xcode/network/cert，Windows PowerShell/winget/PATH/UAC/Event Log。

deepseek-tui 当前有 planning、approval、conversation 机制，但没有“用户手动执行并回传输出”的一等模式，需要新增。

### 模式 2：SSH / PowerShell 远程执行模式

当电脑开启 SSH 或可远程执行命令时，手机 Agent 连接电脑，按计划执行低风险命令，中高风险命令必须审批，输出实时回传。

流程：

```text
connect target
  -> capability probe
  -> diagnostic plan
  -> risk classify command
  -> auto-run low-risk read-only checks
  -> request approval for medium/high risk
  -> stream stdout/stderr
  -> summarize and choose next step
```

需要新增：

- `RemoteToolTransport::Ssh` 与 `RemoteToolTransport::PowerShell`。
- 命令风险分类从 `ExecPolicyEngine` 的 prefix allow/deny 扩展为 target-aware policy。
- stdout/stderr streaming event：可复用 `EventFrame::ExecCommandOutputDelta` 的思想，但放入 mobile-safe protocol。
- idempotency 和 reconnect：远程执行必须支持 retry-safe key。

deepseek-tui 的 shell manager、approval cache、exec policy 有强参考价值，但当前实现是本地 `std::process`/PTY，不能直接进手机端。

### 模式 3：轻量 runner 增强模式

当电脑环境足够稳定后，手机 Agent 引导安装 `kai-runner`。runner 不运行本地大模型，只负责执行工具、采集日志、浏览器自动化、文件操作、MCP 代理和 capability discovery。

流程：

```text
bootstrap/SSH fixes enough prerequisites
  -> phone proposes runner install
  -> user approves
  -> runner pairs with phone
  -> runner reports capabilities
  -> mobile-agent-core selects available tools
  -> tool calls go through HTTPS/WebSocket/MCP
```

需要新增：

- `kai-runner` pairing、auth、capability report、tool call、cancel、log stream。
- runner 侧 adapter：shell/file/git/browser/MCP stdio/LSP/system logs。
- mobile core capability planner：根据 runner capabilities 动态裁剪 tool catalog。

deepseek-tui 适合作为 runner 参考，因为它已有 `crates/tui/src/tools/registry.rs`、`runtime_api.rs`、`mcp.rs`、`tools/file.rs`、`tools/shell.rs`、`tools/git.rs`。但它太重，不应原样作为首版 runner；应收敛成轻量执行进程。

### 模式 4：复杂软件安装与浏览器辅助

当安装软件需要打开网页、下载文件、登录控制台、填写表单时，手机 Agent 应优先走文本和结构化事件。

流程：

```text
install requires browser
  -> no runner: guide user manually open URL / copy error text / paste download link
  -> runner available: remote.browser.open/extract_text/click
  -> browser events return URL/title/text/form state
  -> login/submit/upload/payment always require approval
```

需要新增：

- `remote.browser.*` runner tools，以 URL、title、DOM text、selector、事件为核心，不依赖屏幕流。
- 强审批动作分类：login、submit、upload、payment、download executable、grant permission。
- 手动模式浏览器 guide：给用户简短可执行步骤，并要求回传页面文本或错误码。

deepseek-tui 的 `web_run`/`fetch_url` 提供网络文本抽取参考，但桌面浏览器控制需要 runner 新实现，不能依赖 TUI。

## User Journey

典型用户旅程：

1. 用户打开手机 App，描述“Mac 上 Homebrew 装不上”或拍照/OCR 一段终端错误。
2. 手机 Agent 判断当前电脑能力未知，进入 bootstrap 手动引导。
3. Agent 生成第一条低风险诊断命令，例如 `uname -a; sw_vers; command -v brew; xcode-select -p`，并解释用途。
4. 用户在电脑终端执行，复制输出或拍照回传。
5. Agent 解析输出，确认是 Xcode Command Line Tools 缺失、DNS 问题、代理问题、权限问题或证书问题。
6. Agent 给出下一步。低风险诊断继续手动或 SSH 执行；高风险修复命令先展示风险并要求审批。
7. 电脑恢复到可安装基础工具后，Agent 引导启用 SSH 或安装 `kai-runner`。
8. 安装 runner 后，Agent 自动发现 capabilities，切换到远程工具执行。
9. 后续安装软件、读日志、收集诊断、浏览器辅助都通过 runner 结构化执行。
10. 会话保留完整时间线：用户问题、每条命令、输出摘要、审批决定、修复结论和回滚建议。

这个 journey 要求手机端 core 有很强的“无工具可用时仍能推进”的能力。deepseek-tui 当前更偏“本地工具丰富时的 Agent”，因此需要补 bootstrap-first 产品层。

## Scenario Coverage

| 场景 | deepseek-tui 当前可复用点 | 缺口 | 推荐承载位置 | MVP |
|---|---|---|---|---|
| macOS Homebrew 安装失败 | shell tool、diagnostics、approval、planning | bootstrap 手动命令/OCR、远程 SSH transport、brew 专用诊断模板 | iOS core 规划 + runner/SSH 执行 | 必须 |
| macOS Xcode Command Line Tools 缺失 | shell 执行和错误解释可参考 | `xcode-select --install` 是 GUI/系统动作，需要手动步骤和审批 | iOS core guide + runner 诊断 | 必须 |
| macOS 证书/代理/DNS/权限异常 | network policy、fetch/web、shell diagnostics 思路 | 系统网络配置读取、Keychain/证书说明、代理环境差异 | runner system diagnose + iOS 解释 | 必须 |
| Windows Python/Node/VS Code 安装失败 | planning/approval/session 可复用 | PowerShell/winget/MSI logs/Event Log/UAC 适配缺失 | runner PowerShell adapter | 必须 |
| Windows winget/PowerShell/PATH/UAC 异常 | exec policy 思路可复用 | Windows 命令风险模型和权限升级流程缺失 | runner + iOS approval | 必须 |
| Windows Event Log 或安装日志 | file/log read 思路可复用 | Event Log API/PowerShell query tool 缺失 | runner | Should |
| 用户不会看命令行，需要翻译错误文本 | LLM conversation、message history 可复用 | OCR/text ingest、错误文本结构化解析 | iOS core | 必须 |
| 电脑无法联网，手机给离线修复步骤 | planning 可复用 | offline remediation templates、手机下载/传输策略、校验 hash | iOS core + manual guide | Should |
| 引导安装更强 runner | runtime API/MCP/tool registry 可参考 | pairing、installer、capability discovery、安全模型 | iOS core + runner | 必须 |

总体覆盖判断：deepseek-tui 对“已有本地执行能力后的 Agent 行为”覆盖较好；对“电脑一开始不可用、只能手动救援”的产品最低保底能力覆盖不足，需要新增 bootstrap 产品层。

## Product Capability Mapping

| 产品能力 | deepseek-tui 当前是否已有类似能力 | 可复用模块 | 需要新增模块 | 适合 iOS core | 应在电脑 runner | MVP 必须 |
|---|---|---|---|---|---|---|
| 用户意图理解 | 有，基于 LLM 对话和 system prompt | `crates/tui/src/core/engine/turn_loop.rs`、`prompts/*` | mobile rescue prompt、故障分类 taxonomy | 是 | 否 | 是 |
| 诊断计划生成 | 有，`update_plan`/checklist | `tools/plan.rs`、`tools/todo.rs` | rescue-specific diagnostic playbooks | 是 | 否 | 是 |
| Bootstrap 手动引导 | 部分有对话能力，无一等模式 | session/message/planning | `BootstrapSession`、manual command guide、OCR/text ingest | 是 | 否 | 是 |
| SSH 远程执行 | 无，当前是本地 shell | `execpolicy`、shell result schema 思路 | `SshTransport`、remote shell schema、streaming reconnect | 调度/审批在 iOS | 执行在远端 shell | 是 |
| PowerShell 远程执行 | 无 | `execpolicy` 思路 | `PowerShellTransport`、Windows risk classifier | 调度/审批在 iOS | 执行在 Windows | 是 |
| 远程文件读取 | 本地 file tools 有 | `tools/file.rs` schema/分页思想 | `remote.file.read` transport、sensitive path policy | 调度/审批在 iOS | 实际读取在 runner | 是 |
| 安装器日志采集 | 部分可用 shell/file/git | `tools/file.rs`、`diagnostics.rs` | macOS/Windows installer log adapters | 策略在 iOS | 采集在 runner | Should |
| 系统诊断 | 有本地 diagnostics | `tools/diagnostics.rs`、runtime API health 思路 | `remote.diagnose.system` per OS checks | 规划/解释在 iOS | 探测在 runner/SSH | 是 |
| 包管理器修复 | 可通过本地 shell 完成 | shell、approval、task gate 思路 | brew/winget/apt/npm repair playbooks | 规划/审批在 iOS | 执行在 SSH/runner | 是 |
| 浏览器自动化 | 有 web/fetch 文本工具，非桌面浏览器控制 | `web_run.rs`、`fetch_url.rs` 思路 | `remote.browser.*` runner tools、manual browser guide | 规划/审批在 iOS | 浏览器控制在 runner | Should |
| runner 安装与升级 | 无专门能力 | CLI/install docs、runtime API 参考 | `remote.bootstrap.guide` installer flow、pairing、self-update | 引导/审批在 iOS | 安装/升级在电脑 | 是 |
| capability discovery | 部分有 MCP/tool listing/runtime info | `runtime_api.rs`、`mcp.rs`、`ToolRegistryBuilder` | runner capability schema、dynamic tool catalog | 是 | runner 上报 | 是 |
| 工具风险评估 | 有 shell prefix approval | `crates/execpolicy`、`approval_cache.rs` | target-aware risk classifier、OS-specific dangerous command rules | 是 | runner 二次校验 | 是 |
| 用户审批 | 有 TUI approval 和 runtime approval endpoint | `core/engine/approval.rs`、`runtime_api.rs` | Swift approval sheet、biometric optional、remote approval payload | 是 | runner enforcement mirror | 是 |
| 审计日志 | 有 tool audit/log/runtime events | `runtime_log.rs`、`audit.rs`、`runtime_threads.rs` | mobile audit timeline、runner signed log entries | 是 | 是 | 是 |
| 回滚建议 | 有 snapshot/revert_turn，但偏本地 git/workspace | `snapshot/*`、`revert_turn.rs` 思路 | remote rollback plan、backup refs、Windows restore guidance | 生成建议在 iOS | 备份/恢复在 runner | Should |
| 会话恢复 | 有 session/runtime thread/checkpoint | `crates/state`、`runtime_threads.rs`、`session_manager.rs` | mobile persistence abstraction、transport reconnect replay | 是 | runner job replay | 是 |

MVP 的底线不是覆盖所有桌面工具，而是覆盖：用户意图理解、诊断计划、bootstrap、SSH/PowerShell、基础文件/日志读取、系统诊断、包管理器修复、风险评估、审批、审计、会话恢复、runner 安装引导。

## Fit Assessment for Our Product

### 是否适合作为“手机端 Agent Core”

适配度：Low。

原因：

- 正面：deepseek-tui 已经验证了 DeepSeek V4 thinking/tool-call Agent loop、审批、计划、事件、会话、MCP、工具目录和长输出处理，这些是手机 Agent core 的关键参考。
- 负面：当前生产 core 在 `crates/tui` 内，和 TUI、本地 workspace、本地 shell、PTY、本地 MCP stdio、本地 LSP、桌面 sandbox、快照系统耦合很深。
- 产品缺口：它默认“Agent 所在机器就是工具执行机器”，而我们的产品要求“手机 Agent 独立存在，电脑只是逐步增强的远程 target”。

判断：不建议把 `crates/tui` 减法移植成 iOS core。建议新建 `mobile-agent-core`，复用协议、审批策略、tool schema 思路和 turn-loop 行为测试。

### 本地 shell/file/git/LSP 是否容易替换成 remote tools

适配度：Medium。

替换是可行的，但不是简单改 transport：

- `ToolRegistryBuilder` 已经集中注册工具，提供替换入口。
- `ToolContext` 当前包含 workspace、shell_manager、sandbox_backend、LSP、runtime services 等本地对象，需要拆成 remote-neutral context。
- `turn_loop` 中 tool execution、LSP diagnostics、capacity、working set、snapshots 互相穿插，需要先抽 `ToolExecutor` 和 `ToolPostProcessor`。
- shell/file/git/LSP 实现本身应保留在 runner 侧，而不是改造成 iOS 可编译。

最大工作是把“工具调用协议”和“工具执行实现”彻底分离。

### skills/MCP/approval/session 是否能服务远程电脑救援

适配度：Medium。

- skills：适合变成 rescue playbooks，例如 `homebrew-repair`、`windows-winget-repair`、`offline-install`。但 mobile core 只应加载轻量文本技能，不应执行技能里的本地命令。
- MCP：适合作为 runner 内部扩展机制，尤其是 MCP over HTTP/SSE。手机端不应 spawn MCP stdio server。
- approval：高度可复用。需要从 shell prefix approval 扩展为 remote tool risk approval。
- session：方向正确。需要增加 bootstrap step、target capability、manual output、approval decision、runner job id 等字段。

### 是否支持 bootstrap -> SSH -> runner 的能力升级

当前适配度：Low；架构参考价值：Medium。

deepseek-tui 当前没有把 target capability 当作会话一等状态，也没有 bootstrap 手动引导状态机。它有 runtime API、MCP manager、tool registry 和 session timeline，可以作为能力升级设计参考，但需要新增：

- `TargetProfile`
- `TargetCapabilitySet`
- `ExecutionMode`
- `BootstrapSession`
- `TransportUpgradePlan`
- dynamic remote tool catalog

### 是否能彻底分离“电脑端执行”和“手机端规划”

当前不能彻底分离。最大改造点是 Agent loop 的依赖倒置：

```text
current:
Engine -> ToolRegistry -> local ToolContext -> local shell/file/git/LSP/MCP

target:
MobileAgentCore -> ToolCatalog -> ApprovalGate -> RemoteToolTransport
Runner -> LocalToolRegistry -> shell/file/git/LSP/MCP
```

必须先抽：

- `ModelClient`
- `EventSink`
- `PersistenceStore`
- `ToolCatalog`
- `ToolRiskPolicy`
- `ApprovalGate`
- `RemoteToolTransport`
- `CapabilityProvider`

然后让 `crates/tui` 和 `mobile-agent-core` 各自组合这些接口。

### 明确结论

- deepseek-tui 作为手机端 Agent Core 的适配度：Low。它的 Agent 行为有价值，但当前生产 runtime 与桌面本地执行耦合过深，不适合作为手机 core 直接改造。
- deepseek-tui 作为电脑端 runner 的适配度：High。它已有 shell/file/git/MCP/runtime API/approval 的大量实现，适合被瘦身成 runner 或作为 runner 的实现参考。
- deepseek-tui 作为架构参考的价值：High。
- 是否建议继续投入一周做 technical spike：建议。

一周 spike 的目标应非常收敛：

1. 不移植 TUI。
2. 新建最小 `mobile-agent-core` prototype 或 crate skeleton。
3. 实现 fake model/fake transport 的 bootstrap loop。
4. 实现一个真实 SSH `remote.shell.exec` 或本地 mock runner。
5. 跑通“Homebrew 安装失败”诊断：手机规划、审批、执行/手动回传、解释、下一步。

如果这个 spike 能证明 `turn_loop + approval + remote tool transport + session replay` 可以独立于 TUI 和本地工具运行，就值得继续投入拆 core。若 spike 发现 `crates/tui` 的 turn loop 依赖难以拆开，则应保留 deepseek-tui 作为 runner/reference，手机端 core 从零按上述接口重写。
