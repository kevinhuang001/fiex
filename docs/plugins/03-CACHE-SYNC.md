# 03 · 内核缓存、选择性同步与一致性

[索引](README.md) · [上一篇：VFS](02-VFS.md) · [下一篇：IPC](04-IPC-PROTOCOL.md)

## 1. 所有权和不可变约束

**CacheManager 与 SyncEngine 是 Fiex Core 的公共基础设施，插件 MUST NOT 各自实现 Fiex 的缓存/同步规则、任务队列、版本基线或冲突 UI。** 插件只提供 `list/stat/read/write` 与可选 `getChanges/watch/conditionalWrite`，由内核决定何时读取、存储和上传。

允许插件内部存在短期协议缓冲、服务器协商状态或客户端 session；它们不等同于 Fiex 用户可见的离线缓存。

强制区分以下三种数据：

1. **目录元数据快照**：名字、父子关系、大小和版本，即使文件正文没有下载，也可以显示整棵可达树。
2. **应用私有内容缓存**：下载且经过校验的字节，允许在安全条件下按配额淘汰；缓存状态应显示“已缓存/正在下载/不可用”。
3. **用户同步目录**：由用户选择路径、可编辑的真实文件，**永不作为淘汰缓存自动删除**。本地未同步修改、冲突副本和失败写入必须保护。

不得用长度为 0 的真实文件占位代表云端未下载文件。

## 2. 建议存储

```text
FiexData/
  vfs/metadata.sqlite             # mounts,nodes,directory_snapshots
  cache/objects/aa/<digest>       # 内容寻址对象（带长度、校验）
  cache/staging/                   # 部分下载、写入临时对象
  sync/state.sqlite               # rules, baselines, change_cursors, conflicts
  tasks/tasks.sqlite              # 由任务中心持久化
```

SQLite 建议启用 WAL，记录事务批次。表的逻辑模型（字段及索引可内部演进）：

| 表                    | 主要列                                                                             |
| --------------------- | ---------------------------------------------------------------------------------- |
| `mounts`              | mount_id, plugin_id, provider_config_ref, status                                   |
| `nodes`               | mount_id, node_id, parent_id, name, kind, remote_version, snapshot_time            |
| `directory_snapshots` | mount_id, directory_id, snapshot_token, last_refresh, next_cursor                  |
| `content_objects`     | digest, length, integrity, last_access, pin_count                                  |
| `content_refs`        | mount_id, node_id, version, object_digest, state                                   |
| `sync_rules`          | rule_id, mount_id, node_id/path_fallback, action, recursive, mode, local_target    |
| `sync_baselines`      | mapping_id, local_relpath, remote_ref, local_version, remote_version, last_success |
| `sync_conflicts`      | id, mapping_id, baseline, local_snapshot, remote_snapshot, resolution              |
| `change_cursors`      | mount_id, cursor, committed_at                                                     |

缓存对象应以不可变键存储；未完成的 staging 文件不进入可用缓存索引。私有数据不应跨权限域错误复用缓存条目。凭据不写入这些表。

## 3. 读取与缓存状态机

`not_cached → queued → fetching → verifying → ready`；失败可转 `failed` 并重试。`stale` 表示有旧版本，可按明确的离线策略允许离线阅读；不能把旧版本标记为最新。缓存命中必须在 `mountId + nodeId + version` 对应，后端无法提供强版本号时采用保守 TTL + 校验策略，并显式降低一致性等级。

- `metadata TTL` 默认短，按挂载配置，可异步 stale-while-revalidate；刷新失败显示警告但不清空已有数据。
- `content cache` 采用可配置容量与 LRU/优先级淘汰，缓存引用被打开、下载中、钉选离线或关联未解决修改时不能淘汰。
- 文件下载采用临时文件 + 长度/哈希（如可用）验证 + 原子安装。多处并发打开同一版本只下载一次，共享任务引用。
- 离线打开未缓存文件必须提示不可用，不能返回空内容。

## 4. 三种用户行为

| 行为                                   | 需要本地文件         | 自动上传本地变更   | 删除策略                   |
| -------------------------------------- | -------------------- | ------------------ | -------------------------- |
| `browse` 远程浏览                      | 不需要（可按需预览） | 否                 | 缓存自由淘汰               |
| `pinOffline` 离线可用                  | 下载到私有缓存       | 否                 | 取消钉选不删除用户内容     |
| `mirrorOneWay` / `syncTwoWay` 目录同步 | 用户指定的同步目录   | 单向：否；双向：是 | 按规则、保护、确认传播删除 |

UI 必须清晰区分“离线可用”和“同步到本地文件夹”。**仅有 read 能力的挂载也可以钉选离线，不必具有写入能力。**

## 5. 选择性同步规则

规则可选择单文件、目录、递归目录、排除项；优先级为**更具体路径/节点优先**，同级冲突由显式 `exclude` 优先。选择父目录递归意味着未来新增后代默认包含，除非匹配排除规则。

```json
{
  "mountId": "dav-001",
  "mode": "two-way",
  "localTarget": "/home/user/Fiex Sync/Projects",
  "rules": [
    { "nodeId": "projects", "action": "include", "recursive": true },
    { "nodeId": "temp", "action": "exclude", "recursive": true },
    { "nodeId": "critical.pdf", "action": "include", "recursive": false }
  ]
}
```

- 优先使用稳定 nodeId；路径型 ID 的挂载必须维护 rename/move 对规则的追踪，无法确认时暂停规则并提示修复，不可误删新路径文件。
- 初次同步先做预览（预计文件/字节、目标权限、路径冲突）；只有确认后创建本地目标。
- **取消同步 ≠ 删除本地文件**。默认保留已有本地副本，需要清理时二次明确确认；有未上传修改时禁止自动清理。
- 同步规则与离线钉选均由 Core 存储。挂载插件停用时暂停规则，不能清空规则或同步目录。

## 6. SyncEngine 差异算法

针对每个映射项执行**三方比较**：最后一次成功同步基线 B、当前本地 L、当前远端 R。版本可采用 ETag/对象版本/可靠哈希/时间大小组合，后者仅为弱判断，须在写入前尽可能重新核验。

| L 相比 B | R 相比 B     | 行为                                    |
| -------- | ------------ | --------------------------------------- |
| 未变     | 未变         | 无操作                                  |
| 变       | 未变         | 双向：条件上传；单向：保留/报告本地改动 |
| 未变     | 变           | 安全下载至 staging 再提交               |
| 变       | 变           | 冲突，不得静默覆盖                      |
| 删除     | 未变         | 双向允许经策略传播删除，先入保护期/确认 |
| 未变     | 删除         | 默认保留本地/提示远端删除，待策略处理   |
| 删除     | 变（或反向） | 删除-修改冲突，停下来处理               |

下载/上传完成且条件校验通过，才更新同步基线。部分同步成功也必须逐项提交结果，重启时重放未提交任务不能重复应用破坏性操作。

## 7. 插件可选的同步加速能力

- `fs.getChanges(cursor)`：增量变化与可持久化 cursor。Core 仅在**完整处理对应批次**后提交新游标；cursor 失效退回完整扫描。
- `fs.watch`：即时提示变化，**不是**可靠变化日志，也不直接修改同步基线。
- `fs.conditionalWrite(expectedVersion)`、`fs.conditionalDelete`：用于避免 TOCTOU 覆盖；若插件不能保证提交前版本检查，**不得标记强安全的自动双向同步**。
- `fs.atomicCommit`、`fs.rangeRead`、`fs.checksum`：优化性能/恢复，不应改变基本语义。

插件只报告能力，Core 决定能否开放双向选项；只有幂等提交、可验证的远端版本前置条件与合理删除保护才能默认开启自动双向模式。不具备时可提供手动上传/冲突确认，不能伪造保证。

## 8. 任务、错误与异常恢复

同步任务统一注册 ActivityCenter：扫描→比较→传输→校验→提交→完成；`taskId` 控制取消、进度和恢复信息。暂停、断网、插件崩溃时保留进度及 staging；重试采用退避、限并发及限速，不允许网络失败触发删除。

错误分类：`auth_required`、`offline`、`rate_limited`、`version_conflict`、`capability_unsupported`、`partial_commit`、`storage_full`。Core 的冲突 UI 使用统一差异展示，记录用户选择（保留本地、远端、两者、跳过），插件不得偷偷选一种。

## 9. 非传统文件树

Google Calendar 可将一个事件虚拟化为 `.ics` 节点。缓存导出的事件内容可适用，但“同步”必须有明确的双向对象映射、可写语义、稳定版本与冲突策略；不能仅因为可列出目录就向 UI 提供双向同步选项。Git 提交快照是不可变只读视图，缓存可以启用，但不需要正常的双向同步。

详细节点模型见 [VFS](02-VFS.md)，可取消任务生命周期见 [任务](06-TASKS.md)。
