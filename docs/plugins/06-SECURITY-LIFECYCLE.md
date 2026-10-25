# 06 · 权限、安全与生命周期

## 1. 信任边界

Core 的受信组件包括独立 `PermissionPolicy`、ResourceBroker、PluginRuntime、ContentHost、MountRegistry 和 Keyring 代理。第三方 Native Plugin、插件 iframe、网络 Provider 响应、插件返回的导航目标和远端文件名始终是**不可信输入**。

插件以隔离进程运行。文件、网络、进程执行、存储、凭据和主窗口 UI 均必须经过 Core 受控接口。插件不得访问任意宿主进程、文件路径、Tauri API、任意父窗口 DOM 或其他插件的运行时状态。

## 2. PermissionPolicy 独立模块

```text
Core candidate standard event → PermissionPolicy.project_event(...) → Plugin IPC
Plugin resource request       → PermissionPolicy.authorize_call(...) → ResourceBroker
Broker candidate resource     → PermissionPolicy.audit_response(...) → Plugin IPC
```

`PermissionPolicy` 应是独立可信 Rust crate/模块，输入 `pluginInstanceId`、Manifest 权限申请、用户 Grant、EventScope/OperationScope/TaskScope、具体资源身份与消息字段 Schema。它只返回允许字段/资源及错误，不执行压缩、解码、搜索或具体插件业务。

**标准 Context** 的 Metadata 与本次 read 引用，必须在交付给插件前审核。**自定义菜单/插件 UI** 的资源申请要在操作前审核且返回前再次审核。任何缓存命中都不能绕过这一规则。Core 业务 Host 不自行做散落的文件权限分支。

## 3. 文件授权规则

- `permissions.metadata.enabled` 是文件与文件夹共用的唯一 Metadata 权限，不能因此读文件字节或遍历任意目录。
- `permissions.files.read` 只读取已授权文件字节；`files.create` 仅创建不存在的文件，`files.write` 修改已有文件。
- `permissions.folders.read` 控制目录列表；目录创建、删除、改名及文件相应操作分别审批。
- Manifest 的 `enabled` 是申请，不是自动批准；同一插件跨不同 InteractionSession 的操作范围不自动继承。
- 用户选择文件仅确定候选对象；在自定义 UI 中选择输出路径不等于已经批准删除、覆盖或读取其他文件。最终资源操作仍须与用户明确动作绑定。
- 用户选择整个文件夹时，可授予该具体文件夹及约定子树范围，而不是自动授予当前磁盘根目录。
- 文件发生重命名、替换、符号链接跳转、VFS revision 更改或 Session 失效后，Broker 在执行前重新校验真实对象身份与所需权限。

## 4. Surface Scope

| 事件或操作 | 可访问范围 |
|---|---|
| Info / Preview | 当前打开资源；获准 Metadata 可直接在标准 Context 中交付；已批准文件读则获得该文件的限定只读流引用 |
| Decoration | 当前目录**已显示**成员的获准 Metadata；不可据此扫描其他目录或读文件内容 |
| ContextMenu.resolve | 本次目标/选择 ref；需要属性则申请 `host.fs.stat/statBatch`；不读文件字节 |
| ContextMenu.execute | 真实点击了该插件返回的有效 action 后，允许在单次已确认的资源范围内请求对应读/写/创建等操作 |
| Connection.open/create | 当前连接身份、导航范围；不能凭 Connection 注册获取 VFS、本地文件或无关账户 |
| PluginView | 本次用户交互和所属插件资源的会话；iframe 本身不给宿主 FS 权限 |
| Task | 只继承受确认的 TaskScope，取消、过期、撤权后立即失效 |

只读流的 `resourceId` 是带期限和进程所有权的访问引用，不应可被另一插件重放；大文件流由 Core FileBroker 有界分块、Seek/Range 和会话回收实现，内存不随文件总大小线性增长。

## 5. 缓存与批量请求

`host.fs.statBatch` 对每个 ref 单独进行权限检查、真实对象验证和响应字段投影；部分拒绝返回对应单项错误，不泄露拒绝资源的路径、大小等属性。限制批次长度、响应大小、并发度和耗时。

Explorer/VFS Metadata 缓存与文件内容缓存保存的是数据，**不是权限**。按 `mountId,nodeId,revision/snapshot,type` 识别并确认新鲜度；在删除、改名、写入、解绑账号、插件撤权和会话更改后失效。不能用缓存里的旧 Metadata 替代危险写入/覆盖的最终资源与版本校验。

## 6. 进程、命令与网络隔离

- Native 插件不直接读取本地文件目录、密钥或任意联网；HTTP/TCP 请求由 Core 检查已批准的 host/domain/IP、DNS、重定向、TLS 异常和本地/保留地址策略。
SDK 的同步或 Tokio Trait **不是**沙箱：平台进程隔离仍须禁止插件直接使用原生 OS Socket，网络只能经 Core Broker。
- HTTP 提供高层请求及限额流，TCP 保留低层字节流；使用同一网络授权体系。
插件可以用 rustls 等 TLS 实现叠加 SDK 的获准 TCP 流，支持 STARTTLS；Core 的网络域名/IP 授权与插件 TLS 证书检查依旧必须执行。高层 HTTP 不得默认禁用服务器证书验证。
- 外部命令执行为更高风险的单独授权；`git/gpg/es` 一类程序可有插件沙箱以外的副作用，Core 不得仅因为文件 Scope 存在就允许任意命令。
- 插件流程里的 Stream/Handle/Task 均绑定真实进程实例、Session 和所准资源；取消或禁用后拒绝新请求并回收旧句柄。
Tokio Pending Future 的等待任务在撤权/关闭/超时后必须被唤醒并失败；`spawn_blocking` 或后台任务不得延长过期授权。

## 7. Connection PluginView 的隔离

Connection 点击只接收两类 NavigationTarget：合法 VFS 或属于该插件包的 PluginView。PluginView 是独占**右侧主内容区**的 iframe，不是 Popup/Panel/嵌入 Explorer 局部子区域。加载时严格校验包内路径，禁止 `..`、不可信软链接、远端脚本入口。使用 opaque origin、sandbox 和受约束 CSP；UI 与 Core 通过 MessagePort 白名单交互。

Core 保留窗口与 TabHost 控制权。插件不能覆盖左侧栏/原生标题栏、跨标签访问、越权导航到其他插件的 iframe。更新、重启、禁用、卸载、权限撤销和用户切换视图时必须释放 iframe、Port、后台请求及授权引用。

## 8. 安装与发布

插件包身份绑定 `manifest.id`、平台二进制、校验和、签名与许可声明。安装/升级执行 Schema 校验、资产路径检查和新权限审批；需要签发真实 PluginInstance 身份，IPC 入站不得相信插件自报 ID。日志和错误报告不得记录明文 Secret、完整用户文件内容或无关绝对路径。

## 9. 安全验收

覆盖用户只授权 A 却请求 B、选中目录范围逃逸、软链接及符号链接切换、MenuSession 重放、失效 read 引用、批量混合权限、离线 VFS 跨账户、悬空 iframe、恶意 MessagePort 调用、网络重定向、未经批准的命令执行、冲突覆盖和断电恢复。使用真实 Windows/macOS/Linux 负面测试，并记录可重现的失败结果。
测试原生 Socket 网络绕行、读锁阻塞写、跨 Session Handle 重用、异步取消唤醒和 TLS 证书校验失败；Windows/macOS/Linux 的沙箱能力应分别验证。