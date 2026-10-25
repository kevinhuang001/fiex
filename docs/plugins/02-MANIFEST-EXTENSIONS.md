# 02 · Manifest 分层规范：Surfaces / Providers（目标 schemaVersion 2）

## 1. Manifest 核心模型

插件 Manifest 将**宿主固定的界面插槽（Surfaces）**、**可供调用的非固定界面功能（Providers）**分开。Panel 是 Core 按用户交互在运行时创建的 UI 实例，**不是 Manifest 静态贡献或资源**。Core 内置模块与外部插件可以向相同 Surface Registry 注册**多个独立实现**：一个 Preview 界面并不意味着只有一个预览 Provider，一个 Info 界面也不意味着只有一个元数据 Provider。界面注册不意味着获得文件权限。顶层仅允许以下字段（其余安装报错）：

| 顶层字段 | 类型 | 约束/用途 |
|---|---|---|
| `schemaVersion` | integer | 固定 `2`，与旧 manifest 划清边界 |
| `id` | string | 逆域名唯一、跨版本稳定；示例 `org.example.provider` |
| `name` | string | 展示名称，非空 |
| `version` | string | 语义版本，发布包版本 |
| `description` | string | 可选产品说明 |
| `icon` | string | 可选宿主支持的图标或经校验资源 |
| `author` | object | `{name,url?}` |
| `publisher` | object | `{id,name,keyId,publicKey}`；与签名验证一致 |
| `files` | map<string,string> | 包内文件相对路径到摘要映射，需覆盖执行文件与资源 |
| `runtime` | object | `{protocolVersion,entrypoints}`；UI-only/主题包可为空 |
| `permissions` | object | 用户需审批的资源权限声明 |
| `surfaces` | object | UI 的固定集成位置：Connections / Toolbar / ContextMenus / Search / Preview / Info / Decorations / Settings / Themes |
| `providers` | object | 无固定 UI 插槽的能力：独立 VFS 文件树（可写云盘或只读历史） / 文件操作 Hooks |

`runtime.entrypoints` 按 `windows-x86_64`、`macos-aarch64`、`macos-x86_64`、`linux-x86_64` 等平台映射已打包二进制的相对路径；Native `protocolVersion: "3.0"`；不允许 `..`、绝对路径或包外软链接。禁止在 `surfaces/providers` 中声明 `scheme/schemes/uriPrefix/mountId/mountPath/capabilities`，也禁止 `sync/writeMode/conflictControl` 状态开关；旧顶层 `contributions` 必须被 schema 拒绝，不再双写或兼容。Provider 的具体安全能力在运行时依据协议和回包验证，不由未经验证的静态自报决策。

## 2. 权限对象（完整字段）

所有 **file/folder 操作权限以及统一 `permissions.metadata` 均为对象**，每项都有显式 `enabled: boolean`；未声明或 `enabled:false` 均为禁用。这样可以在未来为每一项独立增加 `extensions`、`limits`、`constraints` 等字段而无需再次修改字段的 JSON 类型。Manifest `enabled` 表示*申请*权限，用户实际批准与本次操作 Scope 仍独立校验。

| 路径 | 类型 | 解释 |
|---|---|---|
| `permissions.metadata.enabled` | bool | **一项覆盖文件与文件夹**：允许独立权限审核模块在限定事件中披露其 metadata，或在明确授权的 OperationScope 中批准插件主动 `host.fs.stat`；不允许列目录或读取字节 |
| `permissions.folders.read.enabled` | bool | 遍历操作范围内的文件夹内容，`host.fs.list`；列举不等于获准读取所有子文件内容 |
| `permissions.folders.create.enabled` | bool | 创建文件夹（目录对象） |
| `permissions.folders.delete.enabled` | bool | 删除文件夹；默认不递归删除，递归删除需明确范围与确认 |
| `permissions.folders.rename.enabled` | bool | 重命名/移动文件夹 |
| `permissions.files.read.enabled` | bool | 读取现有文件的内容 |
| `permissions.files.write.enabled` | bool | 修改现有文件的内容，不隐含创建 |
| `permissions.files.create.enabled` | bool | 创建新文件并写入其初始内容，不隐含修改已存在的文件 |
| `permissions.files.delete.enabled` | bool | 删除已有文件 |
| `permissions.files.rename.enabled` | bool | 重命名/移动已有文件 |
| `permissions.network.enabled` | bool | 请求 Core HTTP/TCP Broker |
| `permissions.network.domains` | string[] | 安装时声明初始可请求精确域名/IP，连接保存的主机需显式批准 |
| `permissions.commands.enabled` | bool | 请求执行宿主进程的高风险权限 |
| `permissions.commands.allow` | string[] | 可调用的命令名白名单；危险性独立提示 |
| `permissions.hooks.enabled` | bool | 允许处理文件打开前钩子，不等于无限文件读取权限 |

推荐声明示例：

```json
{
  "permissions": {
    "metadata": {"enabled": true},
    "folders": {
      "read": {"enabled": true},
      "create": {"enabled": false},
      "delete": {"enabled": false},
      "rename": {"enabled": false}
    },
    "files": {
      "read": {"enabled": true},
      "write": {"enabled": false},
      "create": {"enabled": false},
      "delete": {"enabled": false},
      "rename": {"enabled": false}
    }
  }
}
```

### 2.1 元数据权限与 Core 主动提供的事件上下文

**元数据仅有一项 `permissions.metadata.enabled`，不再区分文件/目录；但 Push 和 Pull 的访问 Scope 仍独立：**

1. **Core 主动推送（Push）**：用户右键、预览、显示简介、工具栏操作等产生了明确的资源上下文；Core 已持有该资源的元数据，若插件申请并获批 `permissions.metadata.enabled`，则 Core 将候选 IPC 输出交给**独立权限审核模块**做字段/资源范围投影，获批字段才能在首次 IPC 中附带 metadata，不需要插件先调用 `host.fs.stat`、也不要求获得 `files.read`、`folders.read`。只推送该次事件实际涉及的选中对象，不能因此持续订阅整个目录。
2. **插件主动查询（Pull）**：`ctx.files().metadata(path)` → `host.fs.stat`。除了获批对应 `metadata.enabled`，**还必须存在用户已确认的 OperationScope/TaskScope 和本次授权范围内的 path**。插件不能借一个元数据权限任意枚举文件系统，查询阶段 `contextMenu.resolve` 也不允许自行主动 stat。
 
```mermaid
flowchart TD
  User["用户触发右键、预览或其他扩展"] --> Core["Core 已持有当前资源 Entry"]
  Core --> Policy["独立 PermissionPolicy.audit_output(候选载荷,插件授权,事件 Scope)"]
  Policy --> Approved{"permissions.metadata.enabled 已批准?"}
  Approved -->|是| Push["审核模块批准的 IPC params.metadata"]
  Approved -->|否| Ref["仅提供本次事件的 opaque ref / selectionCount"]
  Push --> Provider["插件直接使用参数，无第二次 IPC"]
  Ref --> Provider
  Provider --> Pull{"插件在执行阶段主动请求 stat?"}
  Pull -->|是| Scope["检查元数据批准 + OperationScope/TaskScope + 路径"]
  Scope -->|通过| Stat["host.fs.stat 返回元数据"]
  Scope -->|不通过| Denied["PermissionDenied"]
```

Core 发送元数据遵循最小披露：`{name,kind,extension,mime?,size?,modified?,readOnly?,source?}`。完整宿主绝对路径、文件内容、目录子项、密钥、其他文件元信息**不在默认推送中**。`ref` 是不透明资源引用，不是授权令牌。不具有对应 metadata 批准的插件，在其 `contextMenu.resolve` 中只拿到上下文对象的 `ref`（以及不涉及资源属性的选中计数），不会因为声明 `contextMenus:true` 自动收到 name、size 等字段。Core 不应为了推送元数据主动遍历目录或额外读取文件内容。

`folders.read.enabled` 负责列举目录成员，`files.read.enabled` 负责读取文件字节；两者**不代替**对 `host.fs.stat` 的统一 `permissions.metadata` 审核。目录列表即使可返回必要的定位引用，也不能绕过独立审核模块额外泄露文件大小、时间等 metadata；列举不构成任意对象的主动 stat 授权。

**运行时授权 = Manifest 请求 ∩ 用户批准 ∩ 当前事件/OperationScope ∩ 独立 PermissionPolicy 的方法/字段规则。** `enabled:true` 不是永久资源访问凭证；Core 可主动推送已获批的本次事件元数据，不代表插件可以随时主动读取。VFS Provider 的 `vfs.stat` 是 Core 对已建立 VFS Mount 的内部请求，用于节点和同步版本管理，与普通插件主动 `host.fs.stat` 权限不是同一个请求方向。

### 2.2 独立权限审核组件（不在 Core 业务模块里判权限）

权限审核不能分散在 Explorer、MenuBroker、PreviewHost、Decoration Host 或 Connection UI 内。定义独立、可单测的受信 `PermissionPolicy` / `AuthorizationGateway` 模块（建议独立 workspace crate，而非插件可调用的外部 Provider），输入插件身份、已批准的权限快照、Core 候选输出、事件/操作 Scope 及消息 schema；输出为**经过审查的载荷**或 Denied。

```text
Core 生成 candidate payload → PermissionPolicy.audit_output(plugin,grants,eventScope,candidate) → IPC 发送审查后的 payload
Plugin host.fs.* request   → PermissionPolicy.authorize_call(plugin,grants,operationScope,request) → Core Broker 执行
```

`PermissionPolicy` 负责具体字段和资源范围比较，包括嵌套 `entries[]`、`selection[]`、`metadata` 和返回值；未经审核的 Core 数据**不得先序列化送到插件**。Core 业务层不重复写 `if metadata granted` 分支，策略集中在审核模块。自动目录切换事件允许不包含 Metadata 的最小通知，但仅获批 Metadata 才能产出基于文件属性的逐项 Decoration。审核模块独立于插件进程和其可控 IPC 参数，不允许插件自报已获批权限。

## 3. 两类注册：Surface / Provider

**Surface 是 Core 定义的固定 UI 集成点，不是单个全局实现。** 一个 Surface Registry 可以容纳多个 Core 内置 Provider 与多个第三方插件 Provider；二者统一描述符、优先级、匹配、生命周期与视图渲染，调用 Adapter 根据进程信任域区分。**Connection 和 Preview 一样有具体的宿主位置**：添加连接对话框、连接列表/侧栏；连接本身不提供文件访问，网盘还需独立注册 `providers.vfs[]`，不应把它错误归类为无界面 Provider。

| Manifest 字段 | 类型与必要内容 | 对应宿主 UI 位置 | Core 执行入口 |
|---|---|---|---|
| `surfaces.connections[]` | `id,title,icon?` | 仅注册“添加连接（+）”入口和“连接”侧边栏项 | 用户点击调用插件 `connection.create/open`，插件经 IPC 返回 NavigationIntent 或请求 Panel；**不自动生成 VFS** |
| `surfaces.toolbar[]` | `id,title,icon?,command` | 工具栏 | 点击后通过 command 回调；可动态 `host.ui.open` |
| `surfaces.contextMenus` | `true` | 文件/目录右键菜单 | `contextMenu.resolve/execute`，每次右键一次/插件 |
| `surfaces.search[]` | `id,title,command?` | 当前目录的搜索区 | `search.query`，仅搜索指定 root |
| `surfaces.preview[]` | `id,title,extensions?,mimeTypes?,priority?,command?` | 快速预览窗口 | 多内置与外部 Preview Provider 共享 Registry；运行时选择 View |
| `surfaces.info[]` | `id,title,extensions?,mimeTypes?,command?,priority?` | 显示简介窗口 | 与**多个** Core 内置 Info Provider 共享 InfoRegistry，统一 sections 回包 |
| `surfaces.decorations` | `true` | 每次切换目录触发订阅通知 | 每次通知插件 `decorations.resolve`；仅获批 `permissions.metadata.enabled` 才附带当前目录与成员 Metadata，否则只通知、不提供可装饰资源属性 |
| `surfaces.settings[]` | `id,label,type,defaultValue?` | 插件设置页 | Core 生成表单、插件隔离存储/Keyring |
| `providers.vfs[]` | `id,title` | 无固定 UI 插槽的文件树来源；可用于云盘或 Git 历史 | 由用户触发的插件 VFS 请求独立创建 mount；支持只读 Git View、可读写云盘；Core 分配 mountId，具体能力由协议/真实操作校验 |
| `providers.hooks[]` | `id,event,command,priority?` | 文件操作的生命周期，而非 UI 位置 | `hook.beforeOpen` 等；Core 限时裁决 |
| `surfaces.themes[]` | `path` | 外观设置的主题选择器 | 包内主题 JSON token，不能执行脚本 |

不额外把 `PreviewProvider`、`InfoProvider` 声明在 `providers`，因为**预览与信息都属于已知的 UI Surface**。其**具体实现才是多个 Provider**。相反，`providers.vfs` 是无默认 UI 插槽的临时文件树能力；Connection **本身不建立 VFS mount**。网盘插件需要同时声明 `surfaces.connections[]`（UI 入口）和 `providers.vfs[]`（文件树能力），点击具体连接时由插件决定导航到哪一个已建立/按需建立的 VFS mount，也可导航到插件 Panel；Git 历史等独立虚拟文件树也使用 `providers.vfs[]`。

### 3.1 Preview 和 Info：多个内置模块分别注册

```text
PreviewSurface → PreviewRegistry
  ├─ core.preview.text         → 代码高亮/普通文本（独立模块）
  ├─ core.preview.image        → 图片（独立模块）
  ├─ core.preview.audio        → 音频播放器（独立模块）
  ├─ core.preview.video        → 视频播放器（独立模块）
  ├─ core.preview.pdf          → PDF（独立模块）
  ├─ core.preview.fallback     → 基础默认预览（独立模块）
  └─ org.example:preview.xyz   → 插件适配器

InfoSurface → InfoRegistry
  ├─ core.info.metadata        → 基础文件/文件夹 stat 元信息（独立模块）
  ├─ core.info.audio           → lofty 音频标签/时长（独立模块）
  ├─ core.info.video           → MP4 / Matroska 媒体信息（独立模块）
  └─ org.example:info.xyz      → 插件适配器
```

这不仅是接口命名，更是**源文件和责任的真实拆分**：不能将 `core.info.metadata/audio/video` 的业务逻辑全塞进一个 `src-tauri/src/plugins/info.rs` 的 if/else 中，也不能把文本/图片/音视频/PDF 的内容渲染全留在一个 `src/components/Preview.tsx` 中。Registry 只做注册、匹配、调度与会话；每个内置 Provider 拥有自己的实现模块和测试，第三方 Provider 使用相同调度合同但继续通过 IPC/沙箱隔离。

```mermaid
flowchart LR
  UI["Preview / Info UI Surface"] --> Registry["对应 Registry"]
  Registry --> M["多个内置 Provider 模块"]
  Registry --> P["多个外部 Plugin Adapter"]
  M --> Host["统一 PreviewHost / InfoHost"]
  P --> Host
  Host --> Render["宿主窗口渲染"]
```

### 3.2 资源引用和字段约束

- `surfaces.connections[]` 只注册 UI 位置及文案/图标（`id,title,icon?`），**不规定 Core 固定表单，更不自动注册 VFS**。点击添加按钮调用 `connection.create`，点击侧边栏具体连接调用 `connection.open`；插件返回受控 `NavigationIntent` 或使用当前用户会话的 `host.ui.open`。凭据交由 Core Keyring Broker 保管。
- `providers.vfs[]` 由网盘、Git 等文件树插件独立声明；VFS 挂载由插件通过受控 IPC 申请、Core 创建，导航目标只能指向已验证且属于该插件/连接的目标。云盘无需将 `readOnly` 静态写死，按调用的读/写方法及运行时实际能力校验。
- `surfaces.contextMenus:true`：注册动态右键 Provider，不写 `when/children` 或静态 action；每次右键通过 IPC 查询当前元数据、返回扁平 `items[]` 和可选 `parentId`。
- `surfaces.search[]`：没有 `scope`；Core 将当前 root 传给插件，Everything 虽用全局索引，查询与返回都必须限制 root；Core 二次校验。
- `surfaces.preview[]`：扩展名小写无点，MIME 可选；每次预览时插件动态返回具体 View/Panel 请求而非 Manifest 声明 `panelId`；所有内置和外部 Preview 同台排序并有 fallback。
- `surfaces.info[]`：返回 `{sections:[{id,title,rows:[{label,value}]}]}`；Core **分别**注册 `core.info.metadata/audio/video`，第三方可注册多个 Info Handler；一个错误不得影响其他区块。
- `surfaces.decorations:true` **仅订阅每次目录切换**，与授权分开。Core 每次发送 `decorations.resolve` 事件，**权限审核模块**把候选目录及成员 Metadata 同唯一的 `permissions.metadata` 授权核对；获批才附带对应 metadata，未获批仅发送最小事件信息，不能据此产出逐文件 Decoration。没有单独的 Declarative 规则绕过 metadata 的例外。
- `providers.hooks[]`：受控 `{action:"pass"|"consume",reason?}`，Core 最终裁决。
- `surfaces.themes[]`：包内 JSON `{id,name,mode,tokens,darkTokens?}`，只允许校验通过的 tokens，不执行任意 CSS/JS。
- 所有用户交互式 Surface（包括 Connection、Toolbar、ContextMenu、Preview、Info、Search）均可通过运行时 `host.ui.open` 创建 Panel；Manifest **没有** `panels`、`panelId` 或单独 Panel 权限。内容/标题/尺寸/展示位置通过本次 IPC 选择，Core 验证 `interactionId` 和 UI 沙箱。自动查询/刷新阶段或后台任务不可主动弹窗。

### 3.3 Manifest 分类完整示例

```json
{
  "schemaVersion": 2,
  "id": "org.example.media-tools",
  "name": "Media Tools",
  "version": "1.0.0",
  "permissions": {
    "metadata":{"enabled":true},
    "files": {"read":{"enabled":true}}
  },
  "surfaces": {
    "contextMenus": true,
    "preview": [{"id":"viewer","title":"媒体预览","extensions":["xyz"]}],
    "info": [
      {"id":"codec","title":"编码信息","extensions":["xyz"],"command":"media.info.codec"},
      {"id":"tags","title":"标签信息","extensions":["xyz"],"command":"media.info.tags"}
    ],
    "toolbar": [{"id":"open","title":"媒体工具","command":"media.openTools"}]
  },
  "providers": {
    "hooks": [{"id":"before-open","event":"beforeOpen","command":"media.beforeOpen"}]
  }
}
```

同一个插件可提供**多个** Info/Preview 实现：数组中每个 `id` 都是可独立启用、排序、取消和测试的 Provider 注册项；多个实现不必集中写在一个处理器里。Core 内置各个实现也遵守同一原则。批准 `permissions.metadata` 仅允许接收当前事件选中对象的基础属性，解析音视频文件内容仍需要 `files.read` + 本次操作 Scope。

### 3.4 Panel 不是配置对象，而是所有交互式 Surface 通用的运行时服务

Manifest 不含 `panels`、`panelId`、静态 `entry/defaultSize` 等 Panel 注册项。插件处理来自 Toolbar、ContextMenu、Preview、Info、Search、Connection 等宿主 UI 的明确用户交互时，可在 IPC 中**选择本次具体视图及其外观**：

```json
{
  "method":"host.ui.open",
  "params":{
    "interactionId":"core-issued-interaction",
    "view":{"source":"package","entry":"ui/advanced.html"},
    "presentation":{"title":"高级设置","placement":"dialog","preferredSize":{"width":520,"height":400}}
  }
}
```

`view.source` 支持已校验包内 HTML（无需 Manifest 提前声明使用何页面）或受限的运行时 HTML 文本；不得直接加载任意宿主绝对路径或远端地址。Core 仍使用 opaque-origin iframe、CSP 与 UI Bridge 隔离。自动查询（`contextMenu.resolve/info.resolve/search.query`）、后台同步或刷新不能凭注册 Surface 自行弹窗，必须有用户点击/明确打开预览等有效 `interactionId`。**允许请求 Panel 不等于文件/网络权限。** 详见 [09-RUNTIME-UI-PANELS.md](09-RUNTIME-UI-PANELS.md)。

## 4. 完整示范：云盘插件需要 Connection + VFS 两项声明

```json
{
  "schemaVersion": 2,
  "id": "org.example.cloud",
  "name": "Example Cloud",
  "version": "1.0.0",
  "runtime": {"protocolVersion":"3.0","entrypoints":{"linux-x86_64":"bin/example-cloud"}},
  "permissions": {
    "metadata": {"enabled":true},
    "files": {"read":{"enabled":true}, "write":{"enabled":true}, "create":{"enabled":true}},
    "network": {"enabled":true, "domains":["cloud.example.com"]}
  },
  "surfaces": {
    "connections": [{"id":"cloud","title":"Example Cloud","icon":"cloud"}],
    "decorations": true,
    "toolbar": [{"id":"cloud.account","title":"账户信息","icon":"cloud","command":"cloud.openAccount"}],
    "contextMenus": true
  },
  "providers": {
    "vfs": [{"id":"remote-files","title":"Cloud Files"}]
  },
  "author": {"name":"Example Contributors"}
}
```

Connection 仅提供添加入口和侧边栏连接项；用户点击时插件通过 `connection.create/open` 决定是打开运行时登录 Panel、设置页，还是导航到已申请的 VFS mount。声明 VFS 不会自动创建连接，声明 Connection 也不会自动创建挂载。具体 NavigationIntent 及双向同步见 [05](05-VFS-CONNECTION-SYNC.md)。Decoration 虽然会收到每次切换目录的事件，**没有独立批准的 `permissions.metadata` 也拿不到成员元数据**。示例只是一份目标 Schema，不能作为已实现行为。

## 4.1 右键菜单完全动态：Manifest 只注册 Provider

本设计废除静态右键项：**没有 `when`，也没有 `children`**。所有插件只需声明：

```json
{"surfaces":{"contextMenus":true}}
```

Core 在用户每次右键时，并发调用每个已启用且声明该 Surface 的 Native 插件（每个插件恰好一次）：`contextMenu.resolve`。独立权限审核模块仅在插件获批 `permissions.metadata.enabled` 后，才放行该次选择对象的详细 `metadata`；是否通知不依赖 metadata 权限。响应即本次菜单的完整内容；没有独立的静态根菜单或 Core 内置 `isArchive` 判断。UI-only 插件若想使用这一扩展点，必须提供 Native 后端；纯静态 iframe 不是 IPC Provider。

**请求（目标结构）**：

```json
{
  "menuSessionId": "core-ephemeral-session",
  "target": {
    "ref": {"mountId":"m1","nodeId":"n42"},
    "metadata": {"name":"report.zip","kind":"file","extension":"zip",
                 "mime":"application/zip","size":8192,"modified":1800000000,
                 "readOnly":false,"source":"local"}
  },
  "selection": [{"ref":{"mountId":"m1","nodeId":"n42"},"metadata":{"name":"report.zip","kind":"file","extension":"zip","size":8192}}],
  "directory": {"ref":{"mountId":"m1","nodeId":"parent"}}
}
```

若右键目标不在当前多选集合中，Core 以本次右键目标建立操作选中快照；不得将其他无关已选文件发送给插件。请求只提供用户当前显式操作的资源引用；**仅在 `permissions.metadata.enabled` 被批准时**，独立 PermissionPolicy 才会允许把对应文件/文件夹的 `metadata` 对象发送给插件，不要求插件主动调用 `host.fs.stat`。未批准元数据时省略 `metadata`，不提供文件内容、密钥、全局目录树或默认完整主机绝对路径。不透明 `ref` 是定位符，不是后续读写授权。

**响应：一份扁平的菜单项数组**。子菜单通过 `parentId` 引用同次返回的一个 `submenu`，**整个协议也无需 `children`**。

```json
{
  "items": [
    {"id":"archive.compress","type":"submenu","title":"压缩为","icon":"archive"},
    {"id":"archive.zip","parentId":"archive.compress","type":"action","title":"ZIP"},
    {"id":"archive.7z","parentId":"archive.compress","type":"action","title":"7z"},
    {"id":"archive.extract","type":"action","title":"解压到此处","enabled":true},
    {"id":"archive.advanced","type":"action","title":"高级选项…"}
  ]
}
```

| 响应字段 | 是否必需 | 说明 |
|---|---|---|
| `items[]` | 必须 | 本次响应全部菜单节点；允许空数组 |
| `id` | 必须 | 本次菜单与本 Provider 内唯一，供执行回调 |
| `type` | 必须 | `action / submenu / separator`；是否创建 Panel 由点击后的回调决定 |
| `title` | 非 separator 必须 | 可显示菜单标题，最长 128 字符 |
| `icon` | 可选 | 宿主受限图标名 |
| `parentId` | 可选 | 指向同次返回的 `type:submenu` 项；不使用 `children` |
| `enabled` | 可选 | 默认 true；false 显示禁用且不可执行 |
| `checked` | 可选 | 复选标记，不表示有修改数据的权限 |
| `group` / `order` | 可选 | 宿主按分组稳定排序，不能指定宿主绝对位置 |

Core 在用户点击 `action` 后发送 `contextMenu.execute({menuSessionId,itemId,interactionId})`；插件依据 itemId 执行业务操作，或者通过通用 `host.ui.open` 在当前用户交互会话中动态创建 Panel。**菜单项不区分 action/panel，Manifest 不写任何 `panelId`。** 对 Toolbar、Preview、Info、Search、Connection 等其他用户主动触发的 Surface，使用完全相同的运行时 PanelHost 服务。

Core 按单次菜单 Session 保存已返回的 `itemId/type/owner`，执行前核验 Session 未过期、资源未被无声替换、item 已启用且归属同一插件；异步返回必须绑定当前 Session，不能覆盖下一次右键的菜单。

**性能预算（目标）**：Core 并发调用所有 Provider，每个插件仅一次 IPC；内置菜单优先显示，插件菜单增量合并且不改变用户已悬停条目的位置。采用软预算 120 ms、硬截止 250 ms（数值需跨平台基准测试校准），超时/崩溃不阻塞主菜单，丢弃过期结果；每个 Provider 最多 80 项、最大四层，拒绝 parentId 环及超大响应。常驻进程复用，绝不为一次右键启动一个进程。每次右键都必须重新查询，不把旧菜单缓存当成实时结果。

**最小授权**：`resolve` 只有 Core 依据已批准的 `permissions.metadata` 主动推送的本次事件元数据；**它不是插件主动查询许可**，不能调用 `ctx.files().metadata()` 或其他 `host.fs.*` 来额外查看其他对象，更不能读取内容或遍历目录。`execute` 在用户点击后才由 Core 根据真实 selection 建立 OperationScope，并按每一种文件或文件夹权限重新检查。`contextMenus:true` 不自动授予元数据或内容读取权限。

```mermaid
sequenceDiagram
  participant U as 用户
  participant C as Core Menu Host
  participant A as Archive 插件
  participant G as Git 插件
  U->>C: 右键 report.zip
  C->>C: 创建 menuSession + 元数据快照
  par 每插件一次并发查询
    C->>A: contextMenu.resolve(metadata,session)
    A-->>C: items: ZIP, 7z, Extract
  and
    C->>G: contextMenu.resolve(metadata,session)
    G-->>C: items: Git actions 或 []
  end
  C-->>U: 展示本次动态菜单
  U->>C: 点击 ZIP
  C->>C: 验证 menuSession/item/权限与选择
  C->>A: contextMenu.execute(itemId,scope)
  A-->>C: 执行结果
  C-->>U: 反馈
```


## 5. Surface / Provider 生命周期

```mermaid
stateDiagram-v2
  [*] --> Scan: 读取安装包
  Scan --> Reject: schema/资源/签名失败
  Scan --> Pending: 校验通过
  Pending --> Activated: 用户批准所需权限
  Activated --> Registered: 注册各贡献点
  Registered --> Running: UI/Native 按需调用
  Running --> Registered: 空闲回收
  Registered --> Pending: 权限或插件版本变更
  Registered --> Disabled: 用户禁用
  Disabled --> Activated: 重新授权
  Registered --> Removed: 卸载
  Removed --> [*]
```

## 6. 官方插件作为验收样例

| 贡献点 | 必须能用独立插件验证的场景 |
|---|---|
| Connections | WebDAV、SMB、OpenList、Google Drive，多连接实例 |
| VFS | Git 某个 commit 的只读树 |
| Toolbar / ContextMenus | Git、Archive、GPG |
| Search | Everything 搜索来源 |
| Preview | Data/Office 预览 |
| Info | 数据格式的“显示简介” |
| Hooks | Archive 对打开操作的接管 |
| Decorations | ZIP/RAR 徽标与 Git 状态 |
| Themes | Vista / Win11 主题包 |
| Settings | Secret、数字、布尔、枚举（目标扩展须有 schema） |

所有 Surface/Provider 注册项都要实现：禁止命名冲突、独立激活/撤销、错误降级、并发隔离、升级重载、UI 无权限直达 Provider 请求。
