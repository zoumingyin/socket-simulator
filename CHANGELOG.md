# 更新日志

所有重要变更记录于此文件。格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/)。

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
