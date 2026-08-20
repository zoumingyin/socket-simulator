# 更新日志

所有重要变更记录于此文件。格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/)。

---

## [3.0.0] — 2026-08-20

### 新增
- **Mock 工作台增强**：
  - 导入 Swagger/OpenAPI 文档（JSON，OpenAPI 3.x / Swagger 2.0）批量生成规则：按文档 tags 分组勾选导入；响应体按文档响应示例（`example`/`examples`/schema，含 `$ref` 解析与循环引用防护）自动生成。
  - 状态码下拉框：按 1xx/2xx/3xx/4xx/5xx 分组、可自由输入任意状态码。
  - 规则管理：按 Swagger tags 分组展示（可折叠）、批量删除、一键删除全部、响应参数声明式生成响应体（`parseBodyToParams`/`paramsToBody` 双向转换）。
- **存储升级**：SQLite 主读（`config.db`）+ JSON 逃生门（`SSM_REPOSITORY=json`）；日志 `logs.db`、鉴权 `auth.db`、审计 `audit.db` 独立分库；`migrate_from_json` 幂等迁移；数据位于 `app_data_dir`（`SSM_DATA_DIR` 可覆盖）。
- **新能力**：内置压测 CLI（`cargo run --bin benchmark`）；协议适配器骨架（TCP/UDP/MQTT/SSE 预留）；管理 API 鉴权（admin token + 角色分级，viewer 只读强制 403）+ 审计；OpenAPI 3.1 契约发布（`src-tauri/openapi.json`）。
- **品牌与体验**：新 Logo（青绿底白闪电 W1x-a）与应用全套图标；亮色主题品牌色/背景统一为青绿系；前端侧边栏标明版本信息（vite 注入 `package.json`）；启动时直接显示主窗口（移除「最小化到托盘」配置）；侧边栏取消滚动条。

### Changed
- 日志导出改为前端按当前过滤条件导出内存日志为 JSON 下载（移除对不存在的 `POST /logs/export` 后端的依赖；过滤语义与 `log_manager::get_entries` 对齐：serverId 精确 / level 下限 / keyword 命中 message+serverId）。
- 前端日志导出收敛为单一出口 `useLogStore.exportLogs`，移除 `LogViewerPage` 中重复的 blob 逻辑与 500ms 假延迟。
- 工程化：多 bin 声明 `default-run`、lib crate 改名消除 Windows PDB 冲突、全量清理编译警告（主 bin 34→0）、mock 延迟统一由 `engine::dispatch` sleep 生效（移除 DelayMs 死代码）。

### Internal — P0–P3 重构（2026-08-13 ~ 08-15，已在 `b6ec1bf` / `117ed9d` 提交）
- **P0 纠错与安全网**：`ClientDisconnect` 改 camelCase；Socket.IO × Mock 共端口守卫（明确报错，禁静默失效）；CORS / 死字段清理；README 对齐。
- **P1 传输与 API 基建**：抽取 `ip_access` / `bind_with_port_release` / `TransportHooks`；UnifiedServer 组合化（复用 http router + MockEngine，去重）；handlers 按域拆分 + `MockManager` 门面；请求 DTO camelCase 审计。
- **P2 Mock 模型统一**：决策 B（双配置、单引擎），`MockEngine` 统一入口（主端口 / 自定义端口 / 共端口共用），`import_config` 全量重启受影响服务 + `MockManager::restore`，清理 `templates` 死配置。
- **P3 前端整洁**：typed `api/` 模块；`bootstrapCore` 全局 hydrate；`MockWorkbench` 抽取 + 删除试跑 Tab；Mock 实时性注释修正；类型瘦身（删 `ServerStats` / `PressureTest*` / `McpTool*` / `ITransport` 等孤儿类型，补齐 `mockServices` / `ClientInfo.group`）；拆壳（MockComponents / LogViewerPage / App.tsx）；导入规范化 + unused-lint。

### Fixed
- HttpServer 传输层补 `CorsLayer::permissive()`：浏览器跨域访问 HTTP 受管服务（如 127.0.0.1:3333）此前报 CORS 错误。
- 日志导出死调用（见 Changed）。

---

## [2.0.0] — 2026-08-04

### ⚠️ Breaking Changes — 后端架构重写

后端从 Node.js sidecar **完全重写为 Rust**，直接编译进 Tauri 二进制：

- **进程模型**：不再有独立 Node 后端进程，后端随 Tauri 应用自动启动（`tauri::async_runtime::spawn`）
- **管理通道**：Socket.IO → 纯 WebSocket (`/admin/ws`，端口 3080，基于 axum::extract::ws)
- **受管服务**：WS 服务端用 tokio-tungstenite（每服务独立监听），Socket.IO 用 socketioxide 0.18
- **状态共享**：`Arc<Backend>` + 各 Manager 内部 `tokio::sync::Mutex` + `broadcast` 事件总线
- **数据目录**：迁移到 Tauri `app_data_dir()`，首次运行自动从旧目录迁移 config.json
- **配置写入**：通过 mpsc 单写者串行化，避免并发覆盖
- **端口冲突释放**：用 `std::process::Command` 实现（等价 Node killPort）

其他破坏性变更：
- 前端开发端口 5173 → 4173
- 废弃 `start-dev.bat` / `build-all.sh`（含旧 Node sidecar 步骤，已失效）

### New Features

- 消息中心支持本地保存 / 自动填充 / 删除已存消息
- 受管 Socket.IO 服务传输层恢复（socketioxide 0.18）+ CORS 支持
- 模板配置支持
- 开发者工具（DevTools）支持
- 系统托盘支持

### Bug Fixes

- 消息中心发送失败（serverId 等字段反序列化丢失）
- `/api/events/add` 反序列化缺少 id 报错
- 受管 Socket.IO 服务外部客户端无法连接
- Windows 任务栏图标未显示
- Windows 下 beforeDevCommand 因 NODE_PATH 过长而失败

### Internal

- 后端死代码清理（20 处 warning 清零）
- 移除未使用的依赖包
- 仓库清理：构建产物 / 散落文件加入 .gitignore
- 测试：新增 `/admin/ws` 端到端 WS 冒烟测试（23 passed / 0 failed）

---

## [1.1.0] — 2026-08-03

内部版本，代码内容与 v2.0.0 相同。后端 Rust 化已完成并提交，版本号后续调整为 2.0.0 以反映重大架构变更。

---

## [1.0.0] — 2026-06-05

初始发布版本。基于 Node.js sidecar 后端 + Socket.IO 管理通道。
