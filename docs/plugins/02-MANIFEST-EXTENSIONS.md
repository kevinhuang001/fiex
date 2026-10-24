# 02 · Manifest 完整规范与所有扩展点（目标 schemaVersion 2）

## 1. Manifest 核心模型

插件是身份、运行入口、授权请求、扩展点的组合。**扩展点不是权限；权限不是可用性的保证**。顶层仅允许以下字段（其余安装报错）：

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
| `contributions` | object | 宿主贡献数组集合 |

`runtime.entrypoints` 按 `windows-x86_64`、`macos-aarch64`、`macos-x86_64`、`linux-x86_64` 等平台映射已打包二进制的相对路径；Native `protocolVersion: "3.0"`；不允许 `..`、绝对路径或包外软链接。禁止在 `contributions` 中声明 `scheme/schemes/uriPrefix/mountId/mountPath/capabilities`，也禁止 `sync/writeMode/conflictControl` 状态开关。Provider 的具体安全能力在运行时依据协议和回包验证，不由未经验证的静态自报决策。

## 2. 权限对象（完整字段）

| 路径 | 类型 | 解释 |
|---|---|---|
| `permissions.folders.read.enabled` | bool | 读取操作范围内的目录列表 |
| `permissions.folders.read.decorations` | object | 目录装饰规则/动态方法声明，装饰不是额外目录授权 |
| `permissions.folders.create` | bool | 创建目录/目录项 |
| `permissions.folders.delete` | bool | 删除目录项 |
| `permissions.folders.rename` | bool | 改名/移动目录项 |
| `permissions.files.read` | bool | 读取操作范围内的文件 |
| `permissions.files.write` | bool | 写已有文件；创建需目录创建权限 |
| `permissions.network.enabled` | bool | 请求 Core HTTP/TCP Broker |
| `permissions.network.domains` | string[] | 安装时声明初始可请求精确域名/IP，连接保存的主机需显式批准 |
| `permissions.commands.enabled` | bool | 请求执行宿主进程的高风险权限 |
| `permissions.commands.allow` | string[] | 可调用的命令名白名单；危险性独立提示 |
| `permissions.hooks.enabled` | bool | 允许处理文件打开前钩子，不等于无限文件读取权限 |

**运行时授权 = Manifest 请求 ∩ 用户批准 ∩ 本次操作所选资源 ∩ Core 方法规则。** 连接 Provider 的远端元数据读取仍需要连接实例拥有者批准与网络权限，但不意味着必须给插件整个本地磁盘的目录权限。

## 3. Contribution 全量目录

| `contributions` 字段 | 必要字段 | Core 注册/调用方式 | Native handler / UI |
|---|---|---|---|
| `connections[]` | `id,title,fields[]` | Core 渲染连接表单；每次保存生成持久 mount | `connection.validate`, `fs.*` |
| `vfs[]` | `id,title,readOnly:true` | 插件用 `open_vfs_view` 建立挂载 | `vfs.*` |
| `toolbar[]` | `id,title,icon?` | 在工具栏添加静态按钮 | `command` 或 `panelId` |
| `contextMenus` | `true` | 注册一次右键菜单 Provider；每次右键动态查询 | `contextMenu.resolve` / `contextMenu.execute` |
| `search[]` | `id,title,scope,command` | Core 并发或有界聚合检索与结果去重 | 默认 `search.query` 或显式命令 |
| `panels[]` | `id,title,entry,defaultSize?` | 注册本包内已编译 iframe 页面；由工具栏/右键菜单引用 | UI Bridge / 可选 Native 处理器 |
| `preview[]` | `id,title,extensions,priority,panelId` | 在预览优先级选择器中匹配 | 已注册的 iframe + 可选 Native 命令 |
| `info[]` | `id,title,extensions?,command,priority?` | 文件信息抽屉扩展卡片 | 返回标题/rows 的插件方法 |
| `hooks[]` | `id,event,command,priority?` | 在批准的文件操作前调用，必须限时 | `hook.beforeOpen` 等自定义命令 |
| `settings[]` | `id,label,type,defaultValue?` | Core 根据字段生成设置界面、私有存储/Secret | Core 表单，无自动 Native 请求 |
| `themes[]` | `path` | 加载包内 JSON 主题 token，不执行脚本 | 无 Native 要求 |

`toolbar[]` 的每一项使用已声明的 `command` 或 `panelId`；`panels[]` 统一注册包内 iframe，`defaultSize = {width,height}`。`contextMenus: true` 不声明静态菜单项、不声明 `command`、`when`、`children`，也不在 Manifest 中嵌套子菜单；该插件的所有右键项由一次 `contextMenu.resolve` IPC 返回。所有可引用的 `panelId` 必须是本插件 `panels[]` 中已安装、校验过的静态资源 ID。其他贡献 ID 在本插件内唯一，且绝不跨插件调用。

**Contributions 进一步字段约定**：
- `connections.fields[]`：`id,label,type,required?,placeholder?,defaultValue?`；`type` 为 `text|url|number|checkbox|secret`。Secret 始终由 Keyring 代理，日志及普通 JSON 存储禁止泄露。
- `vfs`：为临时只读视图，创建时必须绑定选中 resourceRoot 与小型不可变 viewContext。
- `search.scope`：`global|directory`；来源不能伪造 Core 文件授权。
- `preview.extensions`：小写不带点，`maxBytes` 可限制可预览对象；扩展选择失败回退内置预览。
- `info`：统一返回 `{title?, rows:[{label,value}]}`；不可由插件提交 HTML 直接插入宿主 DOM。
- `hooks`：动作回复应固定为 `{action:"pass"|"consume", reason?}`；Core 最终裁决。
- `themes[]` 对应 JSON `{id,name,mode,tokens,darkTokens?}`，`mode` 只能 light/dark；限制 token 字符与值，不接受 CSS 任意选择器或远程资源。
- `folders.read.decorations`：`mode:"declarative"` 时 `rules[]` 含 `match` 与 `badge`；`mode:"dynamic"` 时需 `method,priority,scope?`，由 Core 有界批量请求。

## 4. 完整示范：一个同时提供云连接、面板和信息扩展的插件

```json
{
  "schemaVersion": 2,
  "id": "org.example.cloud",
  "name": "Example Cloud",
  "version": "1.0.0",
  "description": "远端目录、离线同步和文件信息",
  "runtime": {
    "protocolVersion": "3.0",
    "entrypoints": {
      "windows-x86_64": "bin/example-cloud.exe",
      "macos-aarch64": "bin/example-cloud",
      "linux-x86_64": "bin/example-cloud"
    }
  },
  "permissions": {
    "files": {"read": true, "write": false},
    "network": {"enabled": true, "domains": ["cloud.example.com"]}
  },
  "contributions": {
    "connections": [{
      "id": "cloud", "title": "Example Cloud",
      "fields": [
        {"id":"url","label":"服务地址","type":"url","required":true},
        {"id":"accessToken","label":"访问令牌","type":"secret","required":true}
      ]
    }],
    "panels": [{"id":"cloud.accountPanel","title":"账户信息","entry":"ui/index.html","defaultSize":{"width":480,"height":380}}],
    "toolbar": [{"id":"cloud.account","title":"账户信息","icon":"cloud","panelId":"cloud.accountPanel"}],
    "contextMenus": true,
    "info": [{
      "id":"cloud.info","title":"远端信息","extensions":["txt","pdf"],
      "priority":50,"command":"cloud.fileInfo"
    }]
  },
  "author": {"name":"Example Contributors"}
}
```

示例故意 **不提供** `contributions.vfs`：声明 Connection 后，Core 按用户创建的每个连接生成 VFS mount；不需要重复声明。示例只表示 JSON 结构目标契约，并不表示无需提供校验、实现、打包签名。


## 4.1 右键菜单完全动态：Manifest 只注册 Provider

本设计废除静态右键项：**没有 `when`，也没有 `children`**。所有插件只需声明：

```json
{"contributions":{"contextMenus":true}}
```

Core 在用户每次右键时，并发调用每个已启用、已批准并声明该贡献的 Native 插件（每个插件恰好一次）：`contextMenu.resolve`。响应即本次菜单的完整内容；没有独立的静态根菜单或 Core 内置 `isArchive` 判断。UI-only 插件若想使用这一扩展点，必须提供 Native 后端；纯静态 iframe 不是 IPC Provider。

**请求（目标结构）**：

```json
{
  "menuSessionId": "core-ephemeral-session",
  "target": {
    "ref": {"mountId":"m1","nodeId":"n42"},
    "name":"report.zip","kind":"file","extension":"zip",
    "mime":"application/zip","size":8192,"modified":1800000000,
    "readOnly":false,"source":"local"
  },
  "selection": [{"ref":{"mountId":"m1","nodeId":"n42"},"name":"report.zip","kind":"file","size":8192}],
  "directory": {"source":"local","readOnly":false}
}
```

若右键目标不在当前多选集合中，Core 以本次右键目标建立操作选中快照；不得将其他无关已选文件发送给插件。请求只提供用户当前显式操作的可见元数据与**不透明资源引用**，不提供文件内容、密钥、全局目录树或默认完整主机绝对路径。不透明 `ref` 是定位符，不是后续读写授权。

**响应：一份扁平的菜单项数组**。子菜单通过 `parentId` 引用同次返回的一个 `submenu`，**整个协议也无需 `children`**。

```json
{
  "items": [
    {"id":"archive.compress","type":"submenu","title":"压缩为","icon":"archive"},
    {"id":"archive.zip","parentId":"archive.compress","type":"action","title":"ZIP"},
    {"id":"archive.7z","parentId":"archive.compress","type":"action","title":"7z"},
    {"id":"archive.extract","type":"action","title":"解压到此处","enabled":true},
    {"id":"archive.advanced","type":"panel","title":"高级选项…","panelId":"archive.options"}
  ]
}
```

| 响应字段 | 是否必需 | 说明 |
|---|---|---|
| `items[]` | 必须 | 本次响应全部菜单节点；允许空数组 |
| `id` | 必须 | 本次菜单与本 Provider 内唯一，供执行回调 |
| `type` | 必须 | `action / submenu / panel / separator` |
| `title` | 非 separator 必须 | 可显示菜单标题，最长 128 字符 |
| `icon` | 可选 | 宿主受限图标名 |
| `parentId` | 可选 | 指向同次返回的 `type:submenu` 项；不使用 `children` |
| `enabled` | 可选 | 默认 true；false 显示禁用且不可执行 |
| `checked` | 可选 | 复选标记，不表示有修改数据的权限 |
| `group` / `order` | 可选 | 宿主按分组稳定排序，不能指定宿主绝对位置 |
| `panelId` | type=panel 必须 | 必须引用本插件 `contributions.panels[]` 注册的页面 |

用户点击 `action` 后只调用 `contextMenu.execute({menuSessionId,itemId})`；插件根据 `itemId` 决定 ZIP、7z、解压等业务分支。点击 `panel` 时 Core 打开预先注册的 `panelId`，将该次选中资源传给 UI Bridge；无需回调执行。禁止插件通过响应伪造任意其他插件的命令、Tauri API 或动态未安装 HTML 路径。

Core 按单次菜单 Session 保存已返回的 `itemId/type/panelId/owner`，执行前核验 Session 未过期、资源未被无声替换、item 已启用且归属同一插件；异步返回必须绑定当前 Session，不能覆盖下一次右键的菜单。

**性能预算（目标）**：Core 并发调用所有 Provider，每个插件仅一次 IPC；内置菜单优先显示，插件菜单增量合并且不改变用户已悬停条目的位置。采用软预算 120 ms、硬截止 250 ms（数值需跨平台基准测试校准），超时/崩溃不阻塞主菜单，丢弃过期结果；每个 Provider 最多 80 项、最大四层，拒绝 parentId 环及超大响应。常驻进程复用，绝不为一次右键启动一个进程。每次右键都必须重新查询，不把旧菜单缓存当成实时结果。

**最小授权**：`resolve` 只有本次目标的基础元数据查询权限，不得借此使用 `ctx.files()` 读取内容或遍历未选目录；`execute` 在用户点击后才由 Core 根据真实 selection 创建 operation scope，并执行正常权限检查。`contextMenus:true` **不是** `files.read` 权限。

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


## 5. Contribution 生命周期

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

所有贡献点都要实现：禁止命名冲突、独立激活/撤销、错误降级、并发隔离、升级重载、UI 无权限直达 Provider 请求。
