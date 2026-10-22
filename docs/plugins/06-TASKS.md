# 06 · 任务管理与活动中心

[索引](README.md) · [上一篇：UI](05-UI-EXTENSIONS.md) · [下一篇：安全和打包](07-SECURITY-PACKAGING.md)

## 1. 原则

活动中心是 Fiex Core 的**唯一任务事实来源**；插件不能自行在前端插入一个看似运行的“活动”。Core 文件复制、预览下载、缓存刷新、同步、搜索、Git Push/Pull 与第三方操作共用 TaskManager。

任务可以由 Core 为内部操作创建，也可由插件调用 `host.task.create` 创建；**taskId 只由 Core 发放**。插件调用仅可更新自己创建、由 Core 授权委托的任务。

## 2. 任务模型

```json
{
  "taskId": "task-7e9a",
  "owner": { "kind": "plugin", "pluginId": "org.example.git" },
  "kind": "network",
  "title": "推送到 origin",
  "status": "running",
  "cancellable": true,
  "phase": "uploading",
  "progress": {
    "completed": 42,
    "total": 100,
    "unit": "items"
  },
  "detail": "正在上传对象",
  "startedAt": "2026-10-22T04:05:00Z"
}
```

`kind`、`phase` 是展示提示，不能据此决定业务行为。`progress.unit` 支持 `bytes/items/steps/indeterminate`；`total` 不可得时使用不确定进度，不把“文件数”冒充字节进度。子任务可以带 `parentTaskId`，Core 聚合需定义权重，不把未知长度算成 100%。

## 3. 状态机

```text
queued → running ↔ paused → succeeded
            │         └────→ failed
            ↓ cancel
       cancel_requested → cancelled
            │
            ├────→ interrupted (crash/timeout, cleanup unknown)
            └────→ failed (cancel impossible / partial commit)
```

终态：`succeeded / failed / cancelled / interrupted`。终态只能由 Core 在得到可信执行结果后落库，不允许插件同时发成功和失败；重复且幂等的完成报告可以忽略。`interrupted` 与 `cancelled` 不同：异常杀进程不保证远端操作已经回滚。

## 4. 插件任务协议

```json
{
  "jsonrpc": "2.0",
  "id": "req-task-1",
  "method": "host.task.create",
  "params": {
    "title": "同步 Documents",
    "kind": "sync",
    "cancellable": true,
    "mountId": "dav-001"
  }
}
```

成功返回 `taskId`。后续使用 `host.task.progress(taskId,phase,completed,total,unit,detail)`、`host.task.complete(taskId,result)`、`host.task.fail(taskId,reason)` 或 `host.task.cancelled(taskId)`。进度可高频发生，但 Core 聚合发送 UI（例如每 100ms 刷新上限），持久化低频快照与所有状态边界。插件任务必须可与活动中心的查看/取消按钮一一对应。

## 5. 用户取消的语义

1. UI 调用 `host.tasks.cancel(taskId)`（Tauri 到 Core 内部 API），Core 验证任务归属与状态，标记 `cancel_requested`。
2. Core 将 `plugin.task.cancel(taskId)` 发送给对应插件。插件停止上游请求、取消流、abort 未提交写入，报告已清理或无法安全停止。
3. 仅收到清理确认后标记 `cancelled`；已在远端提交的不可回滚操作要报告 `partial_commit` 和恢复建议。
4. 超时或进程崩溃时设为 `interrupted`，保存日志线索与可恢复的阶段，不谎称任务已取消。
5. 窗口关闭不等于取消任务；TaskManager 存在于 Rust 进程，多窗口展示同一任务。整个应用退出前按策略提示、暂停或停止任务。

取消所有权必须是**每任务独立 token**，取代当前 `OperationCancel` 全局开关，防止一个操作误取消另一个窗口的任务。

## 6. 持久化、恢复与幂等

建议 SQLite `tasks` 表存：任务及父任务 ID、owner、mount、状态、进度摘要、最近错误、开始/结束时间、可恢复标记、提交阶段、可恢复令牌。日志分离保存并清理过期敏感信息。

- 启动时 `running` 一律转为 `interrupted` 或由可恢复适配器明确接管，不能在 UI 显示仍在跑。
- 同步/传输重试时必须通过版本条件与临时提交票据避免二次覆盖/重复删除。插件若不能保证幂等，Core 不自动重试提交阶段。
- 文件复制、网络下载、插件 Git 操作都在 ActivityCenter 中可查看进度与错误；失败提供具体 reason 而非只显示“失败”。

## 7. 现有代码迁移

`src/hooks/useActivity.ts` 的 `localStorage` 活动列表和 `Map<id,cancel>` 改为对 Core 任务快照/事件订阅；React 只管理“活动中心是否打开/选中哪一项”。`src/hooks/useFileOperations.ts` 和 `useGitRepository.ts` 不再自己分配任务 ID；`desktop.rs` 的通用传输接口接收 `taskId`，插件用 `host.task.*` 上报。

活动中心应至少接受：同时两个传输互不误取消、Git Push 可取消、WebDAV 下载断网失败可见、插件崩溃时任务中断可见、重启历史恢复。
