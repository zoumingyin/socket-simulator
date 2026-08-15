# P2-1 Mock 模型统一 — 产品决策方案

> 阶段：P2（M3）首个交付物 ｜ 验收：**设计评审签字**
> 前序：决策门 2/3/4 已关闭（A / A / B），P2 已解锁（见 `2026-08-14-refactoring-plan.md` §5）
> 配套：重构实施计划 `docs/plans/2026-08-14-refactoring-plan.md` 阶段 P2 / PR #5

---

## 1. 当前真实状态（已核查在盘代码）

Mock 在代码里**存在两套并行模型**，且数据字段高度重叠：

### 1.1 独立 Mock 服务（后台存活，但前端无入口）

| 项 | 位置 |
|----|------|
| 载体 `MockServiceConfig` | `src-tauri/src/backend/types.rs:186` |
| 字段 | `id / name / base_path / custom_port:Option<u16> / default_status_code / default_response_body / default_delay_ms / enabled / rules:Vec<MockRule> / created_at / updated_at` |
| 生命周期 + 门面 | `src-tauri/src/backend/mock/manager.rs`（`MockManager` + `MockFacadeError`） |
| 引擎 | `src-tauri/src/backend/mock/server.rs:113` `dispatch(cfg, sys, req)` |
| REST | `src-tauri/src/backend/api/handlers/mock.rs`（`/api/mock/*` 7 端点） |
| 前端 | ⚠️ **无 UI**：`src/types/index.ts:114` 定义了 `MockServiceConfig` 类型，但**全仓无任何组件引用**（已 grep 确认）。独立 `/mock` 菜单/页面已在 workbench 重设计时被移除（计划 §5 决策门 1）。 |

→ 结论：**独立 Mock 服务是「后台能力完整、前端无人驱动」的孤儿**。仅能通过 REST/脚本创建，UI 用户无法触达。

### 1.2 共端口 Mock（前端唯一可见入口）

| 项 | 位置 |
|----|------|
| 载体 `ServerConfig.mock_*` | `src-tauri/src/backend/types.rs:252-270` |
| 字段 | `mock_enabled:bool / mock_rules:Vec<MockRule> / mock_default_status_code / mock_default_response_body / mock_default_delay_ms` |
| 引擎 | `src-tauri/src/backend/transport/unified.rs` `mock_dispatch`（匹配循环在 ~378 行） |
| 前端 | ✅ **唯一 UI**：`src/pages/ServerManager/components/sections/HttpMockSection.tsx`（`mockEnabled/mockRules/mockDefault*`） |

→ 共端口 Mock 仅当 `protocol != SocketIo && mock_enabled==true` 时启用（守卫在 `service_manager.rs:190`）。

### 1.3 两套的字段重叠

| 概念 | 独立 `MockServiceConfig` | 共端口 `ServerConfig.mock_*` | 是否共享 |
|------|--------------------------|------------------------------|----------|
| 规则列表 | `rules: Vec<MockRule>` | `mock_rules: Vec<MockRule>` | 同一 `MockRule`（types.rs:151） |
| 默认状态码 | `default_status_code` | `mock_default_status_code` | 同名概念、异名字段 |
| 默认响应体 | `default_response_body` | `mock_default_response_body` | 同上 |
| 默认延迟 | `default_delay_ms` | `mock_default_delay_ms` | 同上 |
| 路径/端口 | `base_path` + `custom_port` | 无（绑定到所属 Server 端口） | 差异点 |
| 元信息 | `id/name/enabled/时间戳` | 无（归属 Server） | 差异点 |

**引擎层重复**：`mock::server::dispatch`（独立）与 `unified.rs mock_dispatch`（共端口）是两个平行的「匹配→响应」循环；`match_rule`/`rule_response`/`default_response` 已抽进 `matcher.rs`/`responder.rs`，但**编排逻辑仍是两份**（P1-4 故意未合并，保留共端口的 `rule.enabled` 过滤差异）。

---

## 2. 可选方案

### 方案 A —「MockService 唯一载体 + Server 引用」（彻底单模型）

- `MockServiceConfig` 成为**唯一** Mock 载体。
- 共端口 Mock ⇒ 一条 `MockServiceConfig`（`custom_port=None` + `server_id: Option<String>` + `base_path=""`）。
- `ServerConfig` 删除 `mock_*` 五个字段，改为 `mock_service_id: Option<String>`。
- 需要 **schema 迁移**：旧 `ServerConfig.mock_*` → 生成对应 `MockServiceConfig`；`version` bump + `migrate()`。

| 优点 | 缺点 |
|------|------|
| 数据模型彻底单一、无残留 | **破坏性 schema 迁移**（§6 风险 #1：最高） |
| 单引擎天然成立 | 把「独立 Mock」重新扶正为一等公民，与决策门 1「Mock 仅经服务管理配置、UI 移除独立入口」**方向相反** |
| 未来可统一支持共端口 + 独立 | 独立 Mock 仍无 UI（除非本阶段额外补 UI，超出 P2 范围） |

### 方案 B —「双配置、单引擎」（过渡态，推荐）

- **持久化保持不变**：`MockServiceConfig` 列表 与 `ServerConfig.mock_*` 两套并存（零 schema 迁移）。
- **引擎合并为唯一入口** `MockEngine`：
  - 抽出 `MockEngine::dispatch(endpoint, req_meta) -> Response`（纯函数，只认 `rules + 默认三件套`，不认 base_path）。
  - 独立路径：`mock::server::dispatch` 先 `strip_base` 再调引擎。
  - 共端口路径：`unified.rs mock_dispatch` 直接以全路径调引擎（base_path=""）。
  - `rule.enabled` 过滤差异在引擎内统一处理（共端口保留该语义）。
- 孤儿资产（独立 `MockServiceConfig` + `/api/mock/*`）**保留但明确标注**：本阶段不删（删 `templates` 才是 P2-6 范围），其去留作为后续 F 项产品决策。

| 优点 | 缺点 |
|------|------|
| **零 schema 迁移** → 直接消除 §6 最高风险 | 数据模型仍有两套（已知残留，明确推迟） |
| 满足 DoD「匹配/响应逻辑仅一处权威实现」 | 独立 Mock 仍是孤儿（需后续产品决策） |
| 与决策门 1/2/3/4 的「最稳妥」取向一致 | 过渡态，非终态 |
| P2-3 迁移范围骤降（仅 `templates` 清理 + version bump） | — |

---

## 3. 推荐结论

**采用方案 B（双配置、单引擎）。**

理由（逐条对应已决决策与风险偏好）：

1. **风险最小**：决策门选 A/A/B 已表明「最稳妥、最小残留」取向。方案 B 完全避开破坏性 schema 迁移（§6 头号风险），与用户拍板基调一致。
2. **直接命中 DoD**：`MockEngine` 单入口即可满足「Mock 匹配/响应逻辑仅一处权威实现」，无需动持久化。
3. **与决策门 1 自洽**：当前产品事实是「Mock 只经服务管理配置、独立入口无 UI」。方案 B 不把独立 Mock 重新扶正，只是把它收敛进同一引擎、保留 REST 能力；其 UI 去留交给后续 F 项，不污染 P2。
4. **缩小 P2 爆炸半径**：PR #5 从「mock 模型迁移大 PR」降级为「`templates` 清理 + import 重载 + 引擎合并」，更易审、更易回滚。

---

## 4. 方案 B 下的 P2 工作重排

| ID | 原任务 | 方案 B 下实际动作 |
|----|--------|------------------|
| P2-1 | 产品决策落地 | 本方案；**设计评审签字后**生效 |
| P2-2 | `MockEngine` 单入口 | 新建 `src-tauri/src/backend/mock/engine.rs`：`MockEngine::dispatch(endpoint, req_meta)`；`mock::server::dispatch` 与 `unified.rs mock_dispatch` 改为薄适配；单测矩阵覆盖「同规则三来源行为一致」 |
| P2-3 | 配置迁移 | **降级**：无 Mock schema 迁移；仅 `version` bump + 启动 `migrate()` 处理 `templates` 移除（门 4=B）；旧 config 自动升级仅针对 `templates` |
| P2-4 | `import_config` 语义 | 维持门 3=A：写盘 → reload → 停受影响服务 → 按 autoStart/原状态恢复 → `MockManager::restore`（同时覆盖独立 Mock 与主端口分发） |
| P2-5 | 设置/服务变更策略 | 文档化「改 `mock_*` 需重启该 Server」「改独立 Mock 需 restart 该 service」；关键字段变更提示或自动 restart |
| P2-6 | 清理死配置 | 按门 4=B：从 `PersistedConfig` 与 `types.rs` 移除 `templates`/`MessageTemplate`，删除对应读写；**不动** `MockServiceConfig`（其去留属后续 F 项） |

> PR #5 更名为：`refactor(mock): single MockEngine + import reload + drop templates`（不再含 Mock 模型 schema 迁移）。

---

## 5. `MockEngine` 单入口设计（草案）

```rust
// src-tauri/src/backend/mock/engine.rs
pub struct MockEndpoint {
    pub rules: Vec<MockRule>,
    pub default_status_code: u16,
    pub default_response_body: String,
    pub default_delay_ms: u32,
    /// 共端口需保留「UI 可临时禁用单条规则」语义
    pub respect_rule_enabled: bool,
}

/// 纯函数：匹配 → 响应。不感知 base_path / 端口 / IP（IP 由调用方中间件处理）。
pub async fn dispatch(endpoint: &MockEndpoint, meta: RequestMeta) -> Response;
```

- `RequestMeta` = 已解析的 `{ method, relative_path, headers, query_map, body_bytes }`。
- 独立路径：`mock::server::dispatch` 负责 `strip_base` + IP + 组装 `RequestMeta` → 调 `engine::dispatch`。
- 共端口路径：`unified.rs mock_dispatch` 组装 `RequestMeta`（base_path=""）+ `respect_rule_enabled=true` → 调 `engine::dispatch`。
- `match_rule` / `rule_response` / `default_response` 维持现状（已在 `matcher.rs` / `responder.rs`）。

**验收（P2-2 单测矩阵）**：同一组 `MockRule`，分别经 (a) 独立主端口 (b) 独立自定义端口 (c) 共端口 三路 dispatch，断言响应一致（状态码/响应体/延迟）。

---

## 6. 风险与缓解

| 风险 | 等级 | 缓解 |
|------|------|------|
| 引擎合并引入行为回归（尤其共端口 `rule.enabled` 语义） | 中 | P2-2 单测矩阵强制三路一致；先抽引擎再删复制 |
| 孤儿独立 Mock 长期无 UI，未来去留不明 | 低 | 本方案明确标注为 F 项；P2 不删，避免误伤脚本/REST 用户 |
| `templates` 迁移破坏旧 config | 低 | 启动时 `migrate()` 忽略未知/移除字段（serde `default` 已具备）；保留导出备份 |

---

## 7. 签字 / 验收清单

- [ ] 产品方向确认：采用 **方案 B（双配置、单引擎）**
- [ ] 接受「独立 `MockServiceConfig` 本阶段保留、去留交后续 F 项」
- [ ] 接受 P2-3 降级为「仅 `templates` 清理 + version bump，无 Mock schema 迁移」
- [ ] PR #5 范围确认为 `refactor(mock): single MockEngine + import reload + drop templates`
- [ ] 引擎合并后三路 dispatch 行为一致（单测矩阵通过）

> 签字后：进入 P2-2 编码（先建 `MockEngine`，再改两处适配，补单测矩阵）。
