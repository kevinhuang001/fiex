# 01 · 总体架构与职责边界

[索引](README.md) · [下一篇：VFS](02-VFS.md)

## 1. 设计原则

- Fiex Core **MUST** 不依赖任何官方插件 crate/实现模块，也不能包含按官方插件名称、ID、URL scheme 判断的业务分支；`localfs` 是内核自带基础适配器而不是享有特殊权限的官方插件。
- 插件类型在底层不是 `connection` 或 `feature` 枚举。`capabilities` 是组合集合：`filesystem`、`connectionFactory`、`contextMenu`、`toolbar`、`panel`、`commands`、`changeFeed` 等；“连接插件”“功能插件”只用在产品分类。
- **功能插件 MUST 能贡献挂载及虚拟目录**，包括 Git 历史版本、搜索结果、GitHub 仓库树、日历日程视图等。插件的只读目录不必有真实的物理目录。
- 内核独占通用 VFS、缓存、同步、传输、冲突、任务、设置与 UI 宿主。插件只访问声明式 API，不绑定 React/Tauri 或其他插件进程。
- 运行时强制每个已启用插件**独立进程**，官方与第三方使用相同的运行路径、安装位置规则、握手与资源限制策略。

## 2. 分层与数据流

```text
┌────────────────────────────────────────────────────────────┐
│ React UI: Explorer / Tabs / Sidebar / Context Menu /       │
│ Toolbar / Plugin Panels / Sync Rules / Activity Center     │
└─────────────────── Tauri command/events ──────────────────┘
┌────────────────────────────────────────────────────────────┐
│ Fiex Core (Rust)                                           │
│  PluginRegistry  PluginHost  PermissionBroker SecretStore │
│  VfsRouter + NodeIndex + LocalFsAdapter                    │
│  CacheManager  SyncEngine  TransferEngine  TaskManager     │
│  UiContributionRegistry / Settings / Notifications         │
└───────────────── versioned IPC + stream ───────────────────┘
      ↕                    ↕                    ↕
  WebDAV process       SMB process         Git process
  VFS + connections    VFS + connections   UI + read-only VFS
      ↕                    ↕                    ↕
   HTTP/WebDAV             SMB                  git CLI
```

`VfsRouter` 根据挂载 ID 查找适配器。插件跨进程返回树节点、元数据与流票据，核心统一生成 Explorer `Entry`；UI 从不直接调用 Git、SMB、WebDAV 方法。

## 3. 职责矩阵

| 事情 | Core | Plugin |
| --- | --- | --- |
| 安装/授权/卸载、启动/监督 | 唯一负责人 | 提供清单与程序 |
| 挂载管理、节点路由、选择与导航 | 唯一负责人 | 提供不透明节点 ID、列表 |
| 远程网络、外部数据模型 | 提供受控凭据服务 | 实现 |
| 元数据缓存、内容缓存、离线钉选 | 唯一负责人 | 提供版本、读取流 |
| 同步规则、调度、基线与冲突 | 唯一负责人 | 读写/变更提示/条件提交 |
| 跨挂载传输和拖拽导出 | 唯一负责人 | 可选同源优化操作 |
| 任务 ID、进度、取消状态 | 唯一负责人 | 报告进度并响应取消 |
| UI 配置、右键/工具栏/面板容器 | 唯一负责人 | 注册贡献及响应命令 |
| 低层系统沙箱 | 平台专门实现 | 不得声称自动受限 |

插件可以在内存中暂存协议会话数据；不得建立独立于 Core 的“Fiex 离线内容缓存/同步数据库”。协议栈自己管理瞬时网络缓冲不算独立缓存系统。

## 4. 关键实体与所有权

- `pluginId`：逆域名格式（如 `org.example.git`），包和权限主体；跨升级稳定。
- `processInstanceId`：一次启动实例，重启变化；不得用于持久引用。
- `mountId`：Core 生成并持久化的挂载实例（一个插件可能创建多个）；插件卸载时保留或清理需明确确认。
- `nodeId`：在一个挂载内由插件负责的**不透明、尽可能稳定的**资源标识；重命名时最好不变，若后端无法保证应声明 `identityStability=path`。
- `taskId`：Core 创建并拥有，贯穿 RPC、流通道、活动中心和取消。
- `contributionId`：`pluginId` 命名空间内的菜单、按钮或面板 ID，不可与其他插件共享执行权限。

## 5. 进程生命周期

```text
discovered → verified → installed → disabled
                                  ↓ enable
             stopped → starting → negotiating → ready
                          ↓              ↓        ↓
                        failed         rejected   idle / busy
                                                   ↓ stop/crash
                                         stopping → stopped
```

1. 扫描包、校验结构/摘要/签名策略，读取 manifest，验证 API 范围和权限请求；首次启用需授权。
2. Core 按系统架构选取二进制，以当前用户的权限启动进程；`stdin/stdout` 用于控制协议，`stderr` 只记日志。
3. Core 发 `plugin.initialize`，插件回复版本和实际能力。Core 验证声明与实报一致，再登记挂载工厂/贡献。
4. 超时/崩溃标记挂载不可用、使进行中任务进入明确中断状态；按有界退避重启。挂载 ID 与已持久的缓存/同步规则**不会**因为进程重启而更换。
5. 禁用/卸载时停止新请求、取消或等待已有任务、关闭流、撤销贡献、卸载 VFS；保留的用户数据和本地同步目录不直接删除。

避免每个目录或标签启动单独进程。建议**每个插件 ID 一个常驻或按需启动的进程**，进程内服务多个挂载实例；后续可将其拆成多个进程作为优化，不改变公开协议。

## 6. 推荐源码模块

```text
src-tauri/
  src/                 # Tauri 入口，只保留通用应用命令
  crates/
    fiex-core/          # 启动与应用服务
    fiex-vfs/           # 统一节点/挂载/本地适配器
    fiex-plugin-host/   # 发现、启动、IPC、权限检查
    fiex-protocol/      # 与语言无关的线协议模型
    fiex-cache/         # 节点缓存、内容 CAS
    fiex-sync/          # 双向状态机及差异对比
    fiex-transfer/      # 跨源读写、冲突、系统拖拽
    fiex-tasks/         # 持久任务和取消
plugins/official/{webdav,smb,git}/  # 单独二进制、公共 SDK
sdk/rust/                       # SDK 可公开发布
docs/plugins/                   # 当前规格
```

## 7. 非目标和版本化

第一版不是 FUSE/WinFsp/Dokan OS 级全局挂载系统。VFS 先存在于 Fiex 应用内，向外部应用拖拽时由 Core 按需生成可访问的临时内容。插件平台协议版本与 Fiex 应用版本独立：小版本只能兼容增量扩展；破坏性更改必须提高主版本并提供迁移窗口。

有关文件操作，见 [VFS](02-VFS.md)；缓存和同步的严格所有权见 [03](03-CACHE-SYNC.md)；插件的进程与 IPC 见 [04](04-IPC-PROTOCOL.md)。
