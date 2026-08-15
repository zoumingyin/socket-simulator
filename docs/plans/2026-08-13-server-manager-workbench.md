# 服务管理工作台重设计

| 字段 | 内容 |
|------|------|
| 日期 | 2026-08-13 |
| 状态 | 已实现（前端） |
| 范围 | 服务管理页 + 移除独立 Mock 路由 |

## 决策

- 采用主从工作台：左窄列表 + 右分段工作区
- 移除 `/mock` 菜单与页面；Mock 仅在服务管理「HTTP·Mock」区
- 移除旧 HTTP `httpRoutes` Form.List UI（后端字段保留兼容）
- 后端 `/api/mock/*` 本阶段保留、无 UI

## 结构

```
src/pages/ServerManager/
  ServerManagerPage.tsx
  protocolStyles.ts
  components/ServerList.tsx
  components/ServerWorkbench.tsx
  components/sections/{Overview,Basics,HttpMock,Probe}Section.tsx
```

## 数据注意

`POST /api/server/update` 为整对象替换。`useServerStore.updateServer` 已改为与当前列表项合并后再提交，避免局部保存清空 Mock 规则。
