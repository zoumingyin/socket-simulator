# P2-4 预留协议适配器骨架（TCP / UDP / MQTT / SSE）

- 日期：2026-08-20
- 仓库：`E:\work\nengna\git\socket-service-manager`
- 关联：v3.0.0 路线图 `docs/plans/2026-08-19-v3-roadmap.md` §6 P2-4；P0-5 协议适配器抽象（`ProtocolAdapter` + `AdapterRegistry`）

## 1. 交付内容

在既有 `AdapterRegistry` 扩展点之上，新增 4 个**预留协议适配器骨架**：`TcpAdapter` / `UdpAdapter` / `MqttAdapter` / `SseAdapter`。它们已接入注册表（可被 `AdapterRegistry::create` 构造），但 `Transport::start` 等方法返回 `BackendError::NotImplemented`，表明「已规划、未实现」，防止被误用。

同时把 `ProtocolType` 与 `AdapterKind` 一并扩展到 4 个保留协议，使 `protocol()` 能返回真实类型，类型系统在编译期即覆盖这些协议。

## 2. 扩展点机制（P0-5 奠定）

```
ServiceManager
   └─ AdapterRegistry::create(kind, cfg, sys, hooks) -> Arc<dyn ProtocolAdapter>
          └─ 按 AdapterKind 查 factories HashMap，调用工厂闭包
                 └─ 工厂返回 Arc<dyn ProtocolAdapter>（WsServer / SocketIoServer / HttpServer /
                    UnifiedServer / TcpAdapter / UdpAdapter / MqttAdapter / SseAdapter）
```

新增协议只需两步，ServiceManager 零改动即可拉起：
1. 实现 `ProtocolAdapter`（继承 `Transport` 的生命周期/消息方法 + `protocol()`/`server_id()`/`is_unified()`）
2. 在 `AdapterRegistry::register_builtin()` 注册一个 `AdapterFactoryFn`

## 3. 骨架适配器清单

| 文件 | 适配器 | `protocol()` | `start()` |
|------|--------|--------------|-----------|
| `src-tauri/src/backend/transport/tcp.rs` | `TcpAdapter` | `ProtocolType::Tcp` | `Err(NotImplemented)` |
| `src-tauri/src/backend/transport/udp.rs` | `UdpAdapter` | `ProtocolType::Udp` | `Err(NotImplemented)` |
| `src-tauri/src/backend/transport/mqtt.rs` | `MqttAdapter` | `ProtocolType::Mqtt` | `Err(NotImplemented)` |
| `src-tauri/src/backend/transport/sse.rs` | `SseAdapter` | `ProtocolType::Sse` | `Err(NotImplemented)` |

每个骨架结构一致（`cfg` / `sys` / `hooks` / `running: Arc<AtomicBool>`），`stop()` 仅置 `running=false`，其余 `send`/`broadcast`/`disconnect_client` 同样返回 `NotImplemented`。

## 4. 真实实现该怎么做（每协议要点）

通用骨架（参照 `WsServer`）：
- 绑定监听（TCP/UDP 用 `TcpListener`/`UdpSocket`；MQTT 连外部 broker；SSE 复用 HTTP 路由）
- accept loop 为每个连接分配 `client_id`（`nanoid!`），维护客户端注册表
- 按协议自有分帧规则把字节流/消息解析为 `WsFrame{event,data}`
- 在连接/消息/断开时调用 `self.hooks` 的 `on_connect` / `on_message` / `on_disconnect`
- 应用 `self.sys` 的 IP 黑白名单（`crate::backend::net::ip_access::allow_ip`）与最大连接数限制
- 真实实现若需在 accept loop 中取 `Arc<Self>`，仿 `WsServer` 用 `Arc::new_cyclic` 注入 `Weak<Self>`

协议差异：
- **TCP**：面向连接字节流，需自定消息边界（长度前缀 / 分隔符 / 行）
- **UDP**：无连接数据报，按 `client_addr` 维护客户端；无显式 connect/disconnect，可用超时回收
- **MQTT**：连接 broker，按 `ServerConfig` 的 topic 订阅/发布；入站映射为 `WsFrame`
- **SSE**：`text/event-stream` 长连接，服务端单向推送 `event: x\ndata: y\n\n`；通常配合 HTTP 路由

## 5. 关联改动

- `src-tauri/src/backend/types.rs`：`ProtocolType` 新增 `Tcp/Udp/Mqtt/Sse`（serde `"tcp"/"udp"/"mqtt"/"sse"`，保留 specta/utoipa derive）
- `src-tauri/src/backend/error.rs`：新增 `BackendError::NotImplemented(String)`（`error_code`→`NOT_IMPLEMENTED`、`status_code`→501），并提供 `BackendError::not_implemented(msg)` 构造便捷方法
- `src-tauri/src/backend/transport/adapter.rs`：`AdapterKind` 新增 4 变体 + `as_str` + `from_protocol` 穷尽 match 补全；`register_builtin` 注册 4 工厂；测试更新（`builtin_kinds_are_registered` 期望 8 个 + 新增 `reserved_kinds_are_registered` / `reserved_adapter_start_returns_not_implemented`）
- `src-tauri/src/backend/transport/mod.rs`：声明 `tcp`/`udp`/`mqtt`/`sse` 模块
- `src/types/generated.ts`：由 `cargo run --bin export_types` 重新生成，`ProtocolType` 联合类型扩展 4 个字符串字面量（其余不变）

## 6. 验证

- `cargo test --bin socket-service-manager` → **92 passed / 0 failed**（基线 90 + 新增 2）
- `npx tsc --noEmit` → 0 错误
- `cargo build` → 编译无错

## 7. 测试证据

| 测试 | 说明 |
|------|------|
| `builtin_kinds_are_registered` | 注册表含全部 8 个 `AdapterKind` |
| `reserved_kinds_are_registered` | 4 个保留协议均可 `create`，`protocol()`/`server_id()` 正确 |
| `reserved_adapter_start_returns_not_implemented` | `Tcp` 适配器 `start()` 返回 `BackendError::NotImplemented` |

## 8. 推迟项

- `openapi.json` / `src/types/openapi.ts` 的重新生成推迟到 **P2-5（OpenAPI 文档发布）**，避免与 P2-5 的发布范围重叠（utipa 生成的 JSON 在 P2-5 统一产出）。
