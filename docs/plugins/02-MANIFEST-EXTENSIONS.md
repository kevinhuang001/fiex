# 02 · Manifest 与 extensions

## 1. 结构

`extensions` 是所有扩展点的唯一注册入口。每个扩展点有自己的参数 Schema 和事件协议；`permissions` 独立表达插件申请的宿主资源权限。

```json
{
  "schemaVersion": 2,
  "id": "org.example.cloud",
  "name": "Example Cloud",
  "version": "1.0.0",
  "runtime": {
    "protocolVersion": "3.0",
    "entrypoints": {
      "windows-x86_64": "bin/cloud.exe",
      "linux-x86_64": "bin/cloud"
    }
  },
  "permissions": {
    "metadata": { "enabled": true },
    "files": {
      "read": { "enabled": true },
      "write": { "enabled": true },
      "create": { "enabled": true }
    },
    "network": {
      "enabled": true,
      "domains": ["cloud.example.com"]
    }
  },
  "extensions": {
    "connections": [{ "id": "cloud", "title": "Example Cloud", "icon": "cloud" }],
    "vfs": [{ "id": "remote", "title": "Cloud Files" }],
    "info": {
      "match": { "mimeTypes": ["image/*"], "kinds": ["file"] }
    },
    "decorations": {
      "match": { "contains": [".cloud"] },
      "priority": 50
    }
  }
}
```

该实例同时注册连接入口、VFS 文件树、标准 Info 以及目录装饰。单独注册 `connections` 不自动创建 VFS；只有 VFS 注册的 Provider 能处理相应文件树协议。

## 2. 扩展点

| 字段                      | 类型                         | 触发与返回                                                                         |
| ------------------------- | ---------------------------- | ---------------------------------------------------------------------------------- |
| `extensions.contextMenus` | `true` 或 `{match}`          | 用户右键 → `contextMenu.resolve` 返回扁平 items；点 action → `contextMenu.execute` |
| `extensions.decorations`  | `{match?,priority?}`         | 每次切换目录 → 标准可见成员 Context → `decorations.resolve`（仅动态）              |
| `extensions.info`         | `{id,match?,priority?}`      | 用户打开简介 → 标准 InfoContext → InfoResponse                                     |
| `extensions.preview`      | 同上                         | 用户打开预览 → 标准 PreviewContext → PreviewView                                   |
| `extensions.connections`  | `[{id,title,icon?}]`         | 添加连接 + 或侧栏点击 → `connection.create/open` → NavigationTarget                |
| `extensions.vfs`          | `[{id,title}]`               | VFS Broker 的 mount/list/stat/read/write 处理器                                    |
| `extensions.search`       | `[{icon?,scope?}]`           | 当前限定 root 的固定 `search.query`                                                |
| `extensions.toolbar`      | `[{id,title,icon?,command}]` | 点击工具栏按钮时执行命令                                                           |
| `extensions.themes`       | `[{id,title,entry}]`         | 加载经过审核的主题资产                                                             |
| `extensions.settings`     | `[{id,title,command?}]`      | 插件设置入口                                                                       |
| `extensions.hooks`        | `[{event,priority?,match?}]` | 文件操作生命周期中的特定钩子；固定 `hooks.invoke`                                  |

注册不等于权限。每个插件最多注册一个 Info 和一个 Preview；VFS、搜索、工具栏等仍可注册多个来源，其 ID 在该插件内唯一。Manifest 不声明每个具体 iframe 页面，也不需要标记某个扩展点是 UI 还是服务。

## 3. `match`：满足条件才调用插件

`info`、`preview`、`decorations`、`contextMenus` 都支持 `match`。它是**包含谓词**：只有资源满足条件时，Core 才会把该插件加入候选。缺省或空对象表示不限制。

```json
"match": {
  "extensions": ["md", "txt"],
  "mimeTypes": ["text/*"],
  "kinds": ["file"]
}
```

| 字段         | 适用扩展点            | 含义与匹配方式                                              |
| ------------ | --------------------- | ----------------------------------------------------------- |
| `extensions` | 全部                  | 大小写无关；可写 `md`、`.md`、`*.md`、`*`                   |
| `mimeTypes`  | 全部                  | 精确值或 `type/*`，例如 `text/*`、`image/*`；`*/*` 匹配全部 |
| `kinds`      | 全部                  | `file` / `directory`（兼容 `dir`）                          |
| `contains`   | **仅 `decorations`**  | 目录或其祖先包含 `.git`、`package.json` 等名称              |
| `selection`  | **仅 `contextMenus`** | `single`（恰好 1 项）、`multi`（≥2 项）或 `any`（默认）     |

- 同一字段内是 OR，不同字段之间是 AND；
- **`info` / `preview`**：用户在单个资源上打开简介/预览时评估；不会对多选逐个调用。`extensions.info` / `extensions.preview` 是**单个对象**，不匹配就不会启动/调用该插件；
- **`decorations`**：先看 `contains`（目录或祖先），再看当前可见成员中是否至少有一项满足 `extensions/mimeTypes/kinds`；都不满足时不调用插件；
- **`contextMenus`**：先看 `selection` 与选中数量，再对选中集合中的任意一项判断 `extensions/mimeTypes/kinds`；不满足时插件不会收到菜单请求；
- `contains` 和 `selection` 是扩展点专用字段；放在其他扩展点会被 manifest 校验拒绝；
- 旧 manifest 的 `extensions: [...]` 仍作为兼容写法（等价于 `match.extensions`），新插件应使用 `match`。

`decorations` **只有动态模式**，不存在 `mode`、`method`、`scope`、`rules`。Core 固定调用标准方法 `decorations.resolve`。

### 3.1 `decorations.resolve` 的响应只用目录成员名

请求里的 entries 同时带 `path` 和 `name`（插件做 Git 状态等内部查询时可用 `path`），但响应应只返回当前目录中的直接成员名：

```json
{
  "decorations": [
    {
      "name": "main.ts",
      "badge": { "text": "M", "color": "orange", "tooltip": "Git：已修改" },
      "style": { "color": "#c2710c", "fontWeight": "600" }
    }
  ]
}
```

- Core 在当前目录的可见 entries 里按 `name` 精确解析真实路径；未知名称会被忽略；
- 响应不再接受任意绝对路径；旧 `path` 仅作为迁移兼容，且必须命中当前目录的可见项；
- 这样远程 VFS、Windows 分隔符、符号链接和大小写差异都由 Core 处理，插件不需要拼路径。

## 4. 权限模型

| 路径                                                        | 含义                                                    |
| ----------------------------------------------------------- | ------------------------------------------------------- |
| `permissions.metadata.enabled`                              | 文件与文件夹共用的 stat 属性读取授权                    |
| `permissions.files.read/write/create/delete/rename.enabled` | 文件内容与文件对象操作，逐项分别批准                    |
| `permissions.folders.read/create/delete/rename.enabled`     | 目录遍历与目录对象操作，逐项分别批准                    |
| `permissions.network.enabled/domains/all`                   | HTTP/TCP 经 Core 网络代理访问的目标；`all` 表示任意主机 |
| `permissions.commands.enabled/allow`                        | Core 执行外部命令的高风险授权                           |
| `permissions.hooks.enabled`                                 | 特定宿主生命周期钩子授权（如需要）                      |

### 网络权限：白名单与全部网络

```json
"permissions": {
  "network": { "enabled": true, "domains": ["api.example.com"] }
}
```

默认是**精确白名单**。如果确实需要访问任意主机：

```json
"permissions": {
  "network": { "enabled": true, "all": true, "domains": [] }
}
```

`all: true` 会在授权界面显示红色危险警告；用户可以拒绝，或切换为白名单后只添加需要的域名。设置里的“网络域名”也可以随时在“全部网络”和“白名单”之间切换；切换为白名单不会自动重新授权全部网络。

每个文件/目录权限项均是带 `enabled` 的对象；其中 `files.create` 是创建不存在的文件，不表示可覆盖现有文件，`files.write` 是修改已存在文件。权限授权实际有效范围 = **Manifest 申请 ∩ 用户批准 ∩ 本次 EventScope/OperationScope ∩ PermissionPolicy 的对象和方法规则**。Metadata 不意味着可读取内容或列举任意目录。

## 5. 标准事件的输入

`info`、`preview` 和 `decorations` 的上下文结构由 Core 的标准 Schema 预先定义，插件注册事件即可，不需要在 Manifest 另外列 `inputs`。

- **Info / Preview**：ResourceRef、会话信息、获准的基础 Metadata；如已获准读取本次文件，可提供受限只读流引用。完整文件内容从来不以 JSON 内联。
- **Decorations**：本次目录及当前可见文件/文件夹的 ResourceRef 与获准 Metadata；无 Metadata 授权时仍接到事件，但不会收到受保护的属性。
- **ContextMenus**：默认只提供 Session、当前目标与选择集合的 opaque ResourceRef；插件使用通用 SDK 申请额外数据。右键查询期间可以申请所选对象 stat，不能借此读文件字节。
- **Connection**：只提供连接实例引用及交互 Session；导航目标由插件选择。
- **自定义 HTML**：不在 Manifest 预定义输入合同；UI 使用它所属操作的 ResourceRef，并通过受控 UI Bridge/SDK 按需申请资源。

标准事件的候选字段先由 ContextBuilder 收集，然后由独立 PermissionPolicy 根据插件授权和具体 Scope 做字段投影，再交给 PluginRuntime 发送。相同一个目录的候选 Metadata 可以缓存复用，**插件间的批准结果不可复用**。

## 6. Connection 导航的唯一两种结果

```json
{
  "kind": "vfs",
  "mountId": "m-opaque",
  "nodeRef": { "mountId": "m-opaque", "nodeId": "root" }
}
```

```json
{
  "kind": "pluginView",
  "view": { "source": "package", "entry": "ui/account.html" },
  "state": { "connectionId": "account-1" },
  "tabTitle": "账号"
}
```

`pluginView` 可带可选的 `tabTitle`（最多 80 个字符，不能含控制字符），作为标签页标题；没有时使用连接名称。`kind` 枚举**只有** `vfs` 与 `pluginView`。后者由 Core ContentHost 用插件 iframe **完全替换右侧主内容区**（包含原目录面包屑/工具栏/列表），保留左侧导航和全局标签页。插件不能借 Connection 导航打开部分宽度 Panel、对话框或任意外部 URL。用户点击连接加号同样由 `connection.create` 选择此二选一结果；登录配置界面可以是 `pluginView`。

Core 校验目标 mount 的插件/连接归属、iframe 资源所属包、当前交互身份和会话有效期，执行导航并记录标签页历史。单独的通用 `host.ui.open` 可用于**其他**明确用户触发的 UI 操作，但不属于 Connection 导航返回类型。

## 7. 动态右键菜单

`extensions.contextMenus` 可写 `true` 或 `{match}`。`match.selection` 是 contextMenus 特有的：`single` 仅单选、`multi` 仅多选、`any`/缺省不限制数量。菜单项由插件实时返回 `{items:[...]}`，每项可为 `action/submenu/separator`；子菜单使用 `parentId`，不内嵌脚本。Core 按 owner、Session、item ID 和资源身份校验执行。菜单请求里不含完整文件内容，具体读取/写入必须在有效用户操作中经 Broker 授权。

## 8. Schema 验收

- `extensions` 是唯一扩展注册容器；`permissions` 是唯一宿主权限声明位置。
- `Connection NavigationTarget` 仅接受两种枚举；不依赖特殊插件 ID。
- `ContextMenu`、Info、Preview、Decoration 的输入输出 DTO 均有对应版本化 Schema 和合同测试。
- 所有 Registry 注销、会话失效、插件禁用和权限撤销均触发视图及资源清理。
