# P1-6 F-2 栈兼容评估：Socket.IO 共端口可行性

> 日期：2026-08-19 ｜ 决策门输出（P1-6，交付物：本评估文档）
> 关联：v3 路线图 P1-6；F 阶段遗留项 F-2（"SIO 共端口是否借插件化解决"）

## 1. 背景与问题定义

**F-2（推迟项）**：受管 Socket.IO 服务当前只能**独立端口**监听，无法与 REST（3080）/ 管理 WS / Mock 统一路由共端口（`UnifiedServer` 对 `SocketIo` 协议显式跳过，见 `transport/unified.rs`）。

**用户诉求**：SIO 服务能否经 P0-5 的**协议适配器插件化**（`AdapterRegistry`）实现共端口，从而统一端口分配与路由体验？

## 2. 现状（2026-08-19 实测）

| 协议 | 传输实现 | 端口 | 统一路由（3080） |
|------|----------|------|------------------|
| WebSocket | `WsServer`（tokio_tungstenite 0.24） | 服务自端口 | ✅ 支持（升级走 axum fallback） |
| HTTP | `HttpServer`（axum 0.7） | 服务自端口 | ✅ 支持 |
| Socket.IO | `SocketIoServer`（socketioxide 0.18 + engineioxide） | **仅独立端口** | ❌ `unified.rs` 守卫拒绝 |
| Mock | `MockManager` | 主端口(3080)/customPort | ✅ 支持（fallback 分发） |

**根因**（项目实测注释）：socketioxide 0.18 的引擎（engineioxide）基于 **hyper 1.0** 的独立服务路径；而本项目 REST 栈是 **axum 0.7**（hyper 0.14）。Socket.IO 的 engine.io 握手需要 HTTP upgrade 接入同一 hyper 服务 —— hyper 1.0 与 axum 0.7 的 `Service`/连接层不兼容，无法直接共享 `axum::serve` 的 listener 与路由 fallback。

## 3. 插件化（P0-5 AdapterRegistry）能否解决？

**结论：不能。** 理由：

1. **共端口是「传输层连接集成」问题，不是「注册/选择」问题**。`AdapterRegistry` 解决的是"按配置创建哪个适配器实现"（工厂模式），它不改变适配器底层的监听方式。WS/HTTP/Mock 能共端口是因为它们都构建在 axum 0.7 的**同一个 `Router` + `axum::serve`** 上；SIO 被排除是因为其引擎跑在**独立的 hyper 1.0 服务**上，二者无法挂到同一个 listener。
2. 注册表 `register` 覆盖机制**只能替换 SIO 适配器的实现**，不能把独立 listener 的流量路由进 3080 的 axum Router —— 后者需要框架层的 handler 集成。

**佐证**：P0-5 之后 SIO 服务经注册表创建，行为与 v2 一致（P1-1 T2 端到端通过），但共端口仍不可用 —— 插件化未改变该约束。

## 4. 可行技术路径（含升级影响评估）

### 路径 A：升级栈实现真共端口（推荐候选，v3.1+）
- **升级 axum 0.7 → 0.8**（hyper 1.0）+ **socketioxide 升级到支持 axum 0.8 的版本**（0.19+ / 0.20 系列）。
- socketioxide 新版提供 `axum` feature 的 **`axum_handler`**，可将 SIO 挂到 axum 0.8 Router 的 `/socket.io/` 路径，与 REST/WS/Mock 共享 3080。
- **影响面（大）**：axum 0.8 破坏性变更（`Route` API、`Path`/`Query` 提取器、`middleware` 签名等）波及全部 40+ handler 与 router；REST 层、统一路由、管理 WS 均需回归。升级量约 2-3 人日，风险集中在 handler 提取器与中间件。
- 前置：需沙箱/CI 网络拉取新 crate（当前环境离线，需先补依赖，见 MEMORY cargo 离线经验）。

### 路径 B：SIO 独立端口 + 场景编排组合（v3.0.0 现实解）
- 维持现状（SIO 独立端口，`UnifiedServer` 守卫），用 **P1-3 场景编排**把 SIO 服务与 WS/HTTP/Mock 服务编成一组，一键启停 —— 缓解"多服务端口分散"的运维痛点，零栈风险。
- 前端 `SceneManager` 已支持；文档明确 SIO 端口独立分配（`server.port` 唯一性校验已在 add/update 校验）。

### 路径 C：反向代理形态（不推荐）
- 3080 前端反向代理到 SIO 独立端口（如管理面加一层透明转发）。与桌面应用（Tauri 内嵌后端，回环 3080）形态不符，引入额外跳数与配置面，不推荐。

## 5. 结论与建议

| 项 | 结论 |
|----|------|
| F-2 插件化能否解决共端口 | ❌ 不能（共端口是框架层集成问题） |
| v3.0.0 建议 | **路径 B**：SIO 独立端口 + 场景编排组合；`UnifiedServer` 保留 SIO 守卫并在文档/UI 明示 |
| v3.1+ 候选 | 路径 A：axum 0.8 + socketioxide(axum handler) 统一 3080；升级需先解决离线依赖 + 40 handler 回归 |
| 验收标准（P1-6） | ✅ 已给出可行性结论（本评估）；SIO 独立端口行为经 P1-1 T2 验证与 v2 一致 |

## 6. 落地清单（如后续选择路径 A）

- [ ] Cargo.toml：axum 0.8 + socketioxide 最新（离线补依赖流程）
- [ ] router.rs 全部 handler 提取器/中间件回归适配
- [ ] unified.rs 移除 SocketIo 守卫，接入 `axum_handler("/socket.io/")`
- [ ] P1-1 T2 + 共端口矩阵测试（SIO 与 Mock/WS 共存）
- [ ] OpenAPI spec 重导出（42 paths 不变，SIO 属传输面非 REST 面）

---

**关联文档**：`docs/plans/2026-08-19-v3-roadmap.md`（P1-6）；`src-tauri/src/backend/transport/unified.rs`（守卫注释）；`src-tauri/src/backend/transport/adapter.rs`（P0-5 注册表）。
