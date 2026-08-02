# 产品需求文档（PRD）：socket-service-manager 后端 Rust 重写

- **文档版本**：v1.0（简单 PRD）
- **编写角色**：产品经理（许清楚 / Xu）
- **编写日期**：2025
- **语言**：简体中文
- **技术栈倾向**：Rust + Tauri 2（集成进 `src-tauri`）；异步用 `tokio`，WS 服务端用 `tokio-tungstenite`，REST 用 `axum`，序列化用 `serde` / `serde_json`，持久化待定（见需求池与待确认问题）
- **项目代号**：`rust_backend_rewrite`

---

## 1. 背景与原始需求复述

`socket-service-manager` 是一个 Tauri 2 桌面应用：前端 React 19 + TypeScript + Vite + Ant Design 5，桌面壳用 Rust/Tauri。

当前后端是 **Node.js + TypeScript**（`backend/` 目录），作为独立进程运行，由 Tauri 以 sidecar 方式拉起，对外提供：
- REST API（端口 3080，约 27 条路由）
- 面向管理前端的实时通道（Socket.IO，挂载于 `/admin/socket.io` 路径）
- 面向「被管理 socket 服务」的传输层（后端作为 socket 服务端接受外部客户端连接，支持 `websocket` / `socket.io` 两种协议）

**已确认的三个关键决策（硬约束）：**
1. **集成方式**：Rust 后端作为 Tauri 命令/插件直接在 Rust 侧运行，集成进 `src-tauri`，打包无需额外 sidecar，彻底解决打包后不启动的问题。
2. **实时协议**：前端实时通道从 Socket.IO 改为**纯 WebSocket**（Rust 侧 `tokio-tungstenite`，前端 `AdminSocketManager` 改用 ws 客户端）。前端配套改动列为配套需求。
3. **重写范围**：**完整对等重写**现有全部后端功能（服务生命周期、事件定时推送、日志、配置持久化、REST API、实时推送）。

**痛点（重写动机）**：打包后后端启动链路断裂（缺入口脚本 / externalBin / sidecar 逻辑），导致应用发布后后端起不来。同时希望统一技术栈到 Rust/Tauri，降低维护复杂度。

---

## 2. 产品定义

### 2.1 产品目标（Product Goals）

1. **彻底消除打包后后端不启动的问题**：后端作为原生 Rust 代码编译进 Tauri 二进制，随应用启动，不再依赖任何外部 Node sidecar、外部二进制或入口脚本。
2. **统一技术栈、降低长期维护成本**：用一套 Rust/Tauri 代码承载桌面壳与后端逻辑，移除 Node 运行时依赖，减小包体、加快启动、简化发布链路。
3. **功能对等、行为一致、前端改动最小**：对外契约（REST 端口 3080、路由路径、WS 消息 `{event,data}` 格式、复合 clientId `serverId___socketId`、实时推送事件名）与现网保持一致，使前端仅替换 socket 库即可平滑切换，管理端用户无感知。

### 2.2 成功标准（可度量）

- [ ] 打包（nsis）安装后，应用启动即自动拉起后端，管理界面在 3 秒内可读取 `runtime_update` / `client_update` / `log_batch`。
- [ ] 在 dev 与 release 两种构建下，`GET /api/servers` 返回与现网相同结构；全部约 27 条 REST 路由通过等价的契约测试。
- [ ] 实时通道延迟：后端状态变更后，前端 `AdminSocketManager` 在 1s 内收到对应推送事件。
- [ ] 移除后从 `package.json` / `tauri.conf.json` 中不再出现任何 backend sidecar、externalBin、`npm run` 后端启动相关配置。
- [ ] 原有 `config.json` 配置可无缝迁移（导出/导入 JSON 仍可用，且旧版 `config.json` 能被新后端读取）。

### 2.3 用户故事（User Stories）

1. **作为运维管理员**，我希望在桌面应用里新增/编辑/删除一类 socket 服务（配置 ip、端口、协议、心跳、WSS），并一键启停/重启/批量启停，以便集中管理多组长连接服务。
2. **作为运维管理员**，我希望在客户端管理页实时看到每个服务下的在线客户端（IP、连接时间、协议），并能对指定客户端发消息、广播消息、或强制断开某客户端，以便排查与运营。
3. **作为消息运营者**，我希望为指定服务配置「定时轮询事件」（如每 N 秒广播一条 JSON/文本消息），并能用消息模板快速下发，以便自动化推送业务消息。
4. **作为值班人员**，我希望在日志页按服务/级别/关键字过滤查看实时日志流、可清空、可导出，以便快速定位异常。
5. **作为普通用户**，我希望打开应用即自动恢复上次的窗口大小、系统设置（心跳、自动启动、IP 黑白名单、日志保留天数等），并能一键导出/导入全部配置做备份迁移。

---

## 3. 技术规范

### 3.1 需求池（P0 / P1 / P2）

> 优先级定义：P0=必须（阻塞发布）、P1=应该（发布前完成）、P2=可选（后续迭代）。

#### P0 — 必须（对等重写核心）

**P0-1 服务全生命周期管理（对等 `ServiceManager` + `api/index.ts` 服务路由）**
- 服务 CRUD：新增 / 更新 / 删除（删除时若 `status==='running'` 应拒绝，返回错误码 `SERVER_RUNNING`）。
- 启停控制：`start` / `stop` / `restart` / `start-all` / `stop-all` / `restart-all`。
- 运行时监控：维护 `ServerRuntime`（status、startedAt、stoppedAt、error、clientCount、totalConnections、reconnectCount、sentMessages、receivedMessages、sentBytes、receivedBytes），配置变更或消息收发时触发 `runtime_updated`。
- **端口冲突自动释放重试**：启动遇到 `EADDRINUSE` 时，先尝试释放占用端口的进程（Unix：`lsof -ti :port` + `kill -9`；Windows：`netstat -ano` 解析 + `taskkill /PID /F`），释放后等待约 800ms 重试一次；仍失败则置 `status='error'` 并记录错误。
- 协议创建：依据 `ServerConfig.protocol` 创建对应传输层（本期仅 `websocket` 生效；`socket.io` 协议入口本期可保留字段但实现为「不支持/降级」或一并移除，见待确认问题 e）。

**P0-2 客户端管理（对等 `ClientManager` + 客户端路由）**
- 客户端注册/移除：连接时生成客户端信息（`clientId = serverId___socketId` 复合 id，记录 ip、连接时间、协议、状态），断开时移除。
- 列表查询：`GET /api/clients`（支持 `serverId` 过滤）。
- 断开指定客户端：`POST /api/client/disconnect`（按 `clientId` 复合 id 解析 serverId 与真实 socketId）。
- 发消息：广播 `POST /api/client/send`、`POST /api/send-message`（消息中心「指定客户端」），支持 `targetType=broadcast|client`、event 名、text/json 内容，并写日志 + 累加 `sentMessages`。
- 搜索：`clientManager.search(keyword)` 按 id/socketId/ip/groupName 模糊匹配（建议保留为内部能力）。

**P0-3 事件定时推送（对等 `EventManager` + 事件路由）**
- 事件 CRUD：新增 / 更新 / 删除 / 切换启用禁用（`GET /api/events`、`POST /api/events/add|update|remove|toggle`）。
- 轮询广播：对 `status==='enabled'` 且 `pollingEnabled` 的事件，按 `pollingInterval`（秒）定时通过传输层 `broadcast(eventName, parseMessage(defaultMessage))`；服务启动/停止时同步启停对应定时器。
- 手动发送：`emitEvent(serverId, targetClientId, eventName, data)`（指定客户端或广播），并回调 `incrementSentMessages`。
- 默认事件：`connect` / `disconnect` / `message` 初始注入（概念保留；纯 WS 下由传输层在连接/断开/收到消息时自然产生，无需 Socket.IO 专属注册逻辑）。

**P0-4 日志系统（对等 `LogManager` + 日志路由）**
- 实时收集：接收各 Manager 的 `log` 事件，统一写内存环形缓冲（上限建议 2000 条，沿用现网）。
- 文件持久化：按日期分文件（如 `YYYY-MM-DD.log`），追加 JSON 行；日志目录应落在用户可写目录（Tauri `app_data_dir()` 下 `logs/`）。
- 查询过滤：`GET /api/logs` 支持 `serverId` / `level`（含级别下限过滤）/ `keyword`。
- 清空：`POST /api/logs/clear`（清内存；是否清文件需确认，建议仅清内存并提供独立「清理旧日志」能力，见待确认问题 b/d）。
- 导出/导入：`LogManager.exportToFile` / `importFromFile`（保留契约）。

**P0-5 配置持久化与导入导出（对等 `ConfigManager`）**
- 单一 `config.json` 持久化：`servers` / `events` / `templates` / `systemSettings` / `windowConfig` / `version` / `exportedAt`。
- 读取时做防御性修正（数值 `clamp`：心跳 5s~300s、pongTimeout 10s~600s、日志保留 1~365 天、单服务最大连接 1~10000），缺失字段回填默认。
- `GET /api/export`、`POST /api/import`：导入后需重新 `loadEvents` 以刷新轮询。
- 配置目录落在用户可写目录（Tauri `app_data_dir()` 下 `config/`）。

**P0-6 系统设置（对等 `SystemSettings` / `WindowConfig` + 设置路由）**
- `GET /api/settings`、`POST /api/settings` 保存 `systemSettings`（heartbeat / wss / ipAccess 黑白名单 / autoStart / startMinimized / logRetentionDays / maxConnectionsPerServer）与 `windowConfig`（width/height/x/y/maximized）。
- 默认值与现网一致（见 `ConfigManager.getDefaultSystemSettings`）。

**P0-7 REST API（对等 `api/index.ts`，端口 3080 + CORS）**
- 完整实现约 27 条路由（服务 12 + 事件 5 + 客户端 4 + 日志 2 + 设置 2 + 导入导出 2），路径与现网保持一致（含 `/api` 前缀兼容与无前缀重定向逻辑）。
- `Access-Control-Allow-Origin` 沿用现网（dev 默认 `http://localhost:4173`，可用 `ALLOWED_ORIGIN` 覆盖）。
- 端口沿用 3080（`API_PORT` 可覆盖），以最小化前端改动。
- 统一响应结构 `{ success, data?, error?, message? }` 与错误码（`SERVER_NOT_FOUND`、`SERVER_RUNNING`、`EVENT_NOT_FOUND`、`TRANSPORT_NOT_FOUND`、`ROUTE_NOT_FOUND`、`INTERNAL_ERROR`）。

**P0-8 实时推送通道（对等 `api/index.ts` Socket.IO 管理通道 → 纯 WS）**
- 在 Rust 进程内提供管理端 WS 端点（建议路径 `/admin/ws`，详见待确认问题 a；也可沿用 `/admin/socket.io` 但作为纯 WS）。
- 连接建立后推送初始数据：`runtime_update`（全量 runtimes）、`client_update`（全量 clients）、`log_batch`（最近约 100 条日志）。
- 状态变化实时推送：`runtime_update`（ServiceManager `runtime_updated`）、`client_update`（client_connect/disconnect）、`log_update`（单条日志）。
- 心跳：后端按固定间隔推送 `heartbeat`，前端回 `heartbeat_ack` 续命；后端超时（建议 30s）清理僵尸连接。

**P0-9 Tauri 集成方案**
- Rust 后端作为 Tauri 2 插件/命令在 `src-tauri` 内运行：应用启动（`setup` 或 `#[tauri::command]` 触发）即初始化并监听 3080。
- `src-tauri/Cargo.toml` 新增依赖：`tokio`（full 或 rt-multi-thread + macros + net + time + sync）、`tokio-tungstenite`、`axum`（或 `hyper`）、`serde`/`serde_json`、`futures-util`；保留现有 `tauri`、`serde`、`serde_json`、`tauri-plugin-shell`。
- 从 `tauri.conf.json` 与 `package.json` 移除所有 backend sidecar / `externalBin` / `npm run` 后端启动配置；`beforeBuildCommand` 仅负责前端构建。
- 数据目录改用 Tauri `app.path().app_data_dir()`（不再依赖 `SSM_DATA_DIR` 或 `process.cwd()`）。

**P0-10 前端实时通道适配（配套前端改动）**
- 将 `src/socket/AdminSocketManager.ts` 的 `socket.io-client` 替换为纯 ws 客户端（如 `reconnecting-websocket` 或自封装 `WebSocket`），连接 `ws://localhost:3080/admin/ws`。
- 保留现有发布/订阅语义与事件名：`runtime_update` / `log_update` / `log_batch` / `client_update` / `heartbeat` / `heartbeat_ack`，以及指数退避重连、30s 心跳超时检测。
- `src/api/client.ts` 的 REST 调用基本不变（端口/路径对齐 P0-7）。
- 删除 `import 'socket.io-client'` 相关依赖与 `VITE_ADMIN_SOCKET_PATH` 的 socket.io 语义。

#### P1 — 应该（发布前完成）

**P1-1 消息模板管理 REST 接口**
- 现状缺口：类型 `MessageTemplate` 与 `PersistedConfig.templates` 已定义，导出/导入会持久化模板，但**当前后端没有模板 CRUD 的 REST 路由**，模板无法在前端运行时增删改。本期应补齐 `GET/POST /api/templates`、`POST /api/templates/add|update|remove` 等，使 `MessageCenterPage` / `useMessageStore` 可完整管理模板。

**P1-2 WSS/TLS 能力（建议本轮实现）**
- 依据 `ServerConfig.wssEnabled/certPath/keyPath` 与 `SystemSettings.wss`，对受管服务与管理通道启用 HTTPS/WSS（读取 cert/key）。详见待确认问题 b。

**P1-3 IP 黑白名单落地**
- `SystemSettings.ipAccess`（whitelist/blacklist）在 P0 中读取并持久化，但当前 Node 版未真正在传输层做连接拦截。本期建议在 WS 握手阶段实施 IP 过滤（白名单优先/黑名单拒绝）。

**P1-4 客户端分组能力补全或明确放弃**
- 现状缺口：`ClientManager` 有 `createGroup`/`getGroups`/`addClientToGroup` 等方法，但**无对应 REST 路由、且分组仅存内存、不持久化**。本期要么补 REST + 持久化（纳入 `PersistedConfig` 或独立存储），要么明确标注为「暂不实现」并从前端隐藏相关 UI。

**P1-5 日志保留/清理策略**
- 依据 `systemSettings.logRetentionDays` 定期清理过期 `logs/YYYY-MM-DD.log` 文件（现网仅有字段，无清理动作）。

#### P2 — 可选（后续迭代）

**P2-1 压力测试功能**
- 现状：`PressureTestConfig` / `PressureTestResult` 类型已定义但**后端无任何实现与路由**。是否在本轮保留，见待确认问题 c。

**P2-2 统计面板 `ServerStats` 聚合**
- 类型已定义（`sendRate`/`receiveRate`/`uptime` 等），现网未实现实时聚合。可作为后续展示增强。

**P2-3 Socket.IO 协议兼容过渡（如需要）**
- 见待确认问题 e。若前后端强绑定同步发布，则无需；若存在版本错配风险，可保留一个 Socket.IO 兼容窗。

**P2-4 MCP 工具接口**
- 类型 `McpToolDefinition` / `McpToolResult` 已预留。可作为对外集成增强，非本次重写必需。

---

### 3.2 UI 设计稿（引用现有前端，不重新设计）

本期为后端重写，**前端页面/交互基本不变**，仅 `AdminSocketManager` 的传输库替换。以下页面与 store 依赖后端数据（复用即可）：

| 前端页面（src/pages） | 依赖的 store | 依赖的后端能力 |
|---|---|---|
| `ServerManager/ServerManagerPage.tsx` | `useServerStore` | 服务 CRUD、启停/重启/批量、运行时监控（P0-1、P0-7） |
| `ClientManager/ClientManagerPage.tsx` | `useClientStore` | 客户端列表/搜索/断开/发消息（P0-2） |
| `EventManager/EventManagerPage.tsx` | `useEventStore` | 事件 CRUD、启停轮询（P0-3） |
| `MessageCenter/MessageCenterPage.tsx` | `useMessageStore` | 消息下发、模板（P0-2、P1-1） |
| `LogViewer/LogViewerPage.tsx` | `useLogStore` | 日志过滤/清空/导出（P0-4） |
| `Settings/SettingsPage.tsx` | `useSettingsStore` | 系统设置、导入/导出（P0-5、P0-6） |
| `Dashboard/DashboardPage.tsx` | 多 store 聚合 | 实时推送聚合展示（P0-8） |

- 实时数据由 `src/socket/AdminSocketManager.ts` 订阅 `runtime_update` / `log_update` / `log_batch` / `client_update` 并写入各 store。
- REST 调用统一经 `src/api/client.ts`（`apiFetch`，base `localhost:3080`）。
- 主题无关：`useThemeStore` 不依赖后端。

> 说明：UI 层无需重新设计；架构师只需保证后端契约与端口与现网一致，前端仅替换 socket 库（P0-10）。

---

### 3.3 待确认问题（Open Questions）

**(a) 实时通道心跳/事件命名——已核实，需再确认统一方式**
- 经通读代码核实：当前后端 `api/index.ts` 使用 `heartbeat`（后端 emit）/ `heartbeat_ack`（前端回），前端 `AdminSocketManager.ts` 也使用 `heartbeat` / `heartbeat_ack`。**仓库内不存在任何 `admin_ping` / `admin_pong` 引用**（已全文检索确认）。
- 即：现网前后端命名**已经一致**，任务简述中提到的「前端记忆为 admin_ping/admin_pong」与当前代码不符，应为旧版本记忆偏差。
- 待确认：纯 WS 重写后是否**沿用 `heartbeat` / `heartbeat_ack`** 作为 WS 文本/JSON 消息类型名（推荐），还是借机重命名为更语义化的 `ping`/`pong`。建议沿用现有命名以降低风险。

**(b) WSS/TLS 是否在本轮实现**
- 类型与字段齐备（`wssEnabled/certPath/keyPath`、`systemSettings.wss`），Node 版传输层已实现读取 cert/key。建议本轮实现（P1-2），但需确认是否需要管理通道（3080）也支持 WSS，还是仅受管服务支持。

**(c) 压力测试（PressureTestConfig/Result）是否保留**
- 类型已定义但后端**完全未实现**（无路由、无逻辑）。需用户确认：本轮一并实现、仅保留类型占位、或彻底移除类型。倾向：本轮不实现，保留类型但不暴露 UI（P2-1）。

**(d) lowdb 数据如何迁移到 Rust 存储——给出选型倾向**
- 现状澄清：当前 `ConfigManager` **已不使用 lowdb**，而是**纯 fs 读写单个 `config.json`**（注释仍提及 lowdb 属历史遗留）。因此迁移对象就是既有的 `config.json`（= `PersistedConfig` JSON）。
- 选型建议（按倾向排序）：
  1. **纯 JSON 文件（serde_json 读写，推荐起步方案）**：与现网数据格式 100% 一致，零额外依赖，迁移最平滑；缺点是并发写需串行化（单写者 + channel）。
  2. **sled（嵌入式 KV）**：适合 Rust、无外部进程，可作为配置/运行态存储；需做一次 `config.json → sled` 的导入迁移。
  3. **SQLite（sqlx/rusqlite）**：结构化查询强（尤其日志），但引入本地数据库文件与迁移脚本，复杂度更高。
- 倾向：**本期先用纯 JSON 文件方案**保住对等性与迁移平滑度，后续若并发/查询压力大再切 sled。

**(e) Rust 侧是否保留 Socket.IO 协议兼容**
- 已决策本期为**纯 WebSocket**。需确认：是否还需要一个过渡期兼容旧前端（Socket.IO）？
- 倾向：**不需要**。前端与后端同步发布（同一次打包），`AdminSocketManager` 一并改为 ws；受管「socket 服务」的客户端协议按 `ServerConfig.protocol` 字段，本期受管端也只实现纯 WS（`socket.io` 协议入口可保留字段但标记不支持/降级，或在配置 UI 中隐藏该选项）。

**(f) 受管服务与「最大连接数」`maxConnectionsPerServer` 的强制**
- 字段已存在，Node 版未真正做连接数上限强制。本期是否在 WS 握手/连接建立时强制拒绝超额连接？建议 P1 实施。

**(g) 日志「清空」语义**
- 现网 `POST /api/logs/clear` 仅清内存环形缓冲，不清磁盘文件。需确认 Rust 版是否同样只清内存，还是同时提供「清理磁盘日志」独立能力（关联 P1-5）。

---

## 4. 范围边界与风险

- **不在本期范围**：新增业务功能、UI 重新设计、移动端适配（lib.rs 已留 `show/hide_main_window` 移动端钩子但本期不扩展）。
- **主要风险**：
  1. 纯 WS 重写后实时推送语义需与现网逐一对齐（事件名、初始快照、心跳），否则前端订阅失配——通过 P0-8 + P0-10 契约测试覆盖。
  2. 端口冲突释放逻辑在 Windows（`netstat`+`taskkill`）依赖系统命令，Rust 侧需以 `std::process::Command` 等价实现并保持 800ms 重试节奏。
  3. 数据目录从 `SSM_DATA_DIR`/`cwd` 切到 Tauri `app_data_dir()` 时，旧机 `config.json`/`logs` 的迁移路径需明确（见 P0-9、3.3-d）。

---

## 5. 验收清单（摘要）

- [ ] P0-1 ~ P0-10 全部实现并通过契约测试。
- [ ] 打包（nsis）后无需 sidecar 即启动，前端 3s 内收到实时推送。
- [ ] 前端 `AdminSocketManager` 改为纯 ws 客户端，事件名/重连/心跳行为不变。
- [ ] 旧 `config.json` 可被新后端读取并继续运行；导出/导入 JSON 正常。
- [ ] P1-1（模板 CRUD）、P1-2（WSS）、P1-3（IP 黑白名单）、P1-4（分组）、P1-5（日志清理）在发布前完成或明确标注放弃。
