# 09 · PR #3 迁移步骤与 CI 验收

[索引](README.md) · [上一篇：SDK](08-SDK.md) · [下一篇：完整示例](10-EXAMPLES.md)

## 1. 基线和迁移策略

以合并的 PR #3 / v0.2.7 为代码基线。当前仍存在：

- Rust `src-tauri/src/remote.rs`：WebDAV/SMB、`~/.fiex/drives` 镜像、`connections.json` 与明文 `password` 字段。
- Rust `git.rs` / `git_remote.rs` / `git_history.rs`：Git 命令及 `history://` 特殊路径。
- `desktop.rs`：`list_directory`、`transfer`、`register_drag`、`search_files` 中的 `remote::is_remote`、`git_history` 分支，`remote_*`/`git_*` 专用 Tauri command。
- 前端 `ConnectionDialog.tsx` 写死 WebDAV/SMB；`GitPanel.tsx` / `useGitRepository.ts` 绑定 Git 命令；`FileContextMenu.tsx` 写死远程同步；`Sidebar.tsx` 展示连接类型。
- `useActivity.ts` 用 `localStorage` 保存状态，`OperationCancel` 用单个全局取消标记。

迁移 **MUST** 先提供兼容层、保留用户同步目录与配置，不应一次删除现有功能。一次合并是否完成，以新协议测试和旧功能回归为准。

## 2. 里程碑和依赖

| 阶段 | 改动                                                                                                    | 必需验收                                                   |
| ---- | ------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------- |
| M0   | 约定协议、manifest/schema、MockPlugin、静态架构依赖检查                                                 | MockPlugin 独立进程握手通过                                |
| M1   | `VfsRouter` + LocalFsAdapter + NodeIndex，重构通用 list/stat/read/transfer                              | 本地四种视图、复制/移动、搜索、双面板拖拽不回归            |
| M2   | `PluginHost`（发现、启动、IPC、能力/权限）；加入第三方样例 VFS                                          | 无插件时可启动；安装样例即可显示树                         |
| M3   | `CacheManager` + `SyncEngine` + `TaskManager`                                                           | 选择性离线钉选、取消、崩溃恢复、冲突保护                   |
| M4   | 把 WebDAV/SMB 打包为公共 SDK 官方插件、迁移连接数据                                                     | 无 `remote.rs` 业务路由；已有连接及离线状态可升级          |
| M5   | Git 插件注册工具栏/右键/面板 + 只读历史 VFS                                                             | Git 完整功能回归；无 `history://` 内核专用分支             |
| M6   | 预编译 iframe、`@fiex/ui-sdk`、隔离 origin/MessageChannel/CSP、插件设置、跨平台包签名与非 Rust 协议测试 | 三平台 UI-only/Native-only/Hybrid 安装、安全和崩溃测试通过 |

可以按小 PR 分阶段，但最终合入后 Core 不能残留以官方插件 ID 区分行为的代码。临时旧路径兼容适配器须有明确删除时点和测试。

## 3. 现有数据迁移

1. 先创建一次用户数据备份与数据库 schema 版本记录，确保可回滚。启动时检查旧 `~/.fiex/connections.json`，导入 Core 挂载记录，生成稳定新 `mountId` 并维护 oldId→newId 映射。
2. 旧文件中 `password` 明文迁入 OS SecretStore；成功写入并验证后**安全地停止继续使用旧明文**，按平台可行性尽量清理旧文件。不能承诺普通文件系统的覆盖删除能彻底擦除历史副本或 SSD 写入。
3. 旧 `~/.fiex/drives/<oldId>` 的“占位文件”不能作为本地实际内容导入。必须基于旧 `remote-meta` 中 `downloaded=true` 和长度/内容验证仅导入真实下载文件；无依据时标记为未缓存，绝不能把零字节占位当成可用文件。
4. 历史收藏/标签页/最后路径中的远程挂载及 `history://` 路径由兼容 PathResolver 转换到 URI，无法转换时展示修复入口而不直接删除书签。
5. 过去的“远程同步”实际偏向离线下载，默认**只迁为离线钉选**，不得自动变成双向同步；新增双向同步必须由用户显式选择本地目录与规则。
6. 卸载/禁用插件时保留同步规则、离线缓存及用户拥有的同步目录，直到用户明确选择清理。

## 4. 三平台 CI 测试矩阵

| 主题      | 测试场景                                                                                              |
| --------- | ----------------------------------------------------------------------------------------------------- |
| 进程/协议 | 连接、超时、崩溃、乱序响应、错误 JSON、恶意帧、重启、版本不兼容                                       |
| VFS       | 空目录、万级文件分页、稳定 ID、重命名、搜索、软链接/路径穿越、功能插件虚拟目录                        |
| 缓存      | 命中/过期、内容版本变化、重复下载合并、离线可用、缓存淘汰不删用户同步目录                             |
| 同步      | include/exclude、未来子节点、双向更新、删除修改冲突、ETag 不一致、断电/断线续跑                       |
| 跨源传输  | 本地↔WebDAV、SMB↔WebDAV、目录递归、冲突、拖拽到浏览器/OS、移动后删除源                                |
| UI        | 工具栏/右键、多选、预编译 iframe、ESM/CSS、MessageChannel 握手和调用、隔离 origin/CSP、禁用后贡献消失 |
| 任务      | 并行取消互不影响、插件 Push 中断、失败原因、重启历史恢复                                              |
| 安全      | 伪造插件/挂载 ID、未授权路径、凭据脱敏、插件包路径逃逸、签名/ABI 错误                                 |
| 官方平权  | 拆除官方插件后安装第三方同能力插件，功能可用；不修改 Core 源码                                        |

PR 级 CI 至少运行单测、`cargo fmt/clippy/test`、前端 lint/build、VFS 与 IPC 契约测试。集成测试应使用临时 WebDAV/SMB 服务与可控 FakeProvider，保证版本冲突场景可重复；三平台原生 CI 增加进程终止、外部拖拽视频或 E2E 核验。

## 5. 强制架构守卫

CI 约束：`fiex-core` / `fiex-vfs` / `fiex-plugin-host` 不得依赖 `plugins/official/*` crate；不得引用官方插件 ID、`remote::is_remote` / `git_history::route` 特判。manifest、Schema 与 golden protocol fixtures 必须由官方插件与一个第三方示例插件共用。可用依赖图检查 + AST/源代码扫描作静态保障，避免靠代码评审主观判断。

## 6. 发布检查清单

- [ ] 干净环境：卸载官方插件后 Fiex 本地文件管理完整可用。
- [ ] 独立安装官方 WebDAV/SMB/Git 包，文件、同步和 Git 历史面板功能恢复。
- [ ] 功能插件可安装 read-only 虚拟目录挂载，列表与跨挂载复制可用。
- [ ] 第三方自定义插件不修改 Core 即可安装、配置、使用任务中心并取消任务。
- [ ] UI-only 插件不含 Native 程序也可安装；Hybrid 插件的 iframe 只通过 `@fiex/ui-sdk` 与宿主及自身 Native 后端通信，三平台完成权限与异常处理测试。
- [ ] 用户旧连接、收藏/路径、有效离线文件迁移完成；明文密码不再写入。
- [ ] 未下载占位文件不会被误当真实文件；取消同步不会删除用户文件。
- [ ] 打包、签名、跨平台运行路径和协议拒绝错误行为均通过测试。

此文档是实施验收标准，不代表 M0–M6 已完成。
