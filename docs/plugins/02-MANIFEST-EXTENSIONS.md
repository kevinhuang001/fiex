# 02 · Manifest 与扩展

## 顶层结构

```json
{
  "schemaVersion": 2,
  "id": "org.example.plugin",
  "name": "Example",
  "version": "1.0.0",
  "description": "…",
  "icon": "puzzle-piece",
  "author": { "name": "Example", "url": "https://example.com" },
  "runtime": {
    "protocolVersion": "3.0",
    "entrypoints": {
      "linux-x86_64": "fiex-plugin-example",
      "macos-aarch64": "fiex-plugin-example",
      "windows-x86_64": "fiex-plugin-example.exe"
    }
  },
  "permissions": {},
  "extensions": {}
}
```

- `id` 是唯一插件身份，也是所有贡献身份。
- 有 native 后端的插件必须有 `runtime.entrypoints`；纯主题插件可以没有 runtime。
- `schemaVersion` 当前为 `2`，IPC 协议版本为 `3.0`。

## 扩展点

所有扩展点都在 `extensions` 下注册，**每插件最多一个贡献**。

| 扩展点         | 形态                                               | Core 固定调用                                 |
| -------------- | -------------------------------------------------- | --------------------------------------------- |
| `contextMenus` | `true` 或 `{match}`                                | `contextMenu.resolve` / `contextMenu.execute` |
| `decorations`  | `{match?, priority?}`                              | `decorations.resolve`                         |
| `info`         | `{match?, priority?}`                              | `info.resolve`                                |
| `preview`      | `{match?, priority?, maxBytes?, panel?, loading?}` | `preview.open`                                |
| `connections`  | `{title, fields}`                                  | `connection.validate/open/create`             |
| `vfs`          | `{title, readOnly?}`                               | `vfs.list/stat/read…`                         |
| `search`       | `{icon?, scope?}`                                  | `search.query`                                |
| `toolbar`      | `{title, icon?, panel?}`                           | `toolbar.invoke`，有 panel 时打开面板         |
| `themes`       | `{path}`                                           | 主题资产加载                                  |
| `settings`     | `[{id,label,type,…}]`                              | 设置字段；字段 id 是存储 key                  |
| `hooks`        | `{event, priority?, match?}`                       | `hooks.invoke`（`beforeOpen` 可消费打开）     |
| `ui`           | `{timeoutMs?}`                                     | 面板桥接与单次插件请求的超时预算              |

不再接受：

- `connections`/`vfs`/`toolbar`/`search`/`hooks` 的数组；
- `info`/`preview`/`connections`/`vfs`/`toolbar`/`search`/`hooks` 的贡献 `id`；
- `toolbar.command`、`info.command`、`hooks.method`、`search.command`、`search.title`、`info.title`、`preview.title`。

## 各扩展点示例

`match` 是 Core 在处理 provider 前检查的包含条件：不匹配时不查询该 provider 的权限或命令、不构造操作上下文、不发送事件 IPC。省略 `match` 或使用空对象表示匹配全部；格式错误的规则会拒绝加载，不能退化为匹配全部。

不同条件之间为 AND，同一 `extensions` / `mimeTypes` / `kinds` 列表内为 OR。批量目标至少一个满足资源条件才会选择 provider；匹配成功后仍接收本次完整目标集合。`selection` 按完整选择数量判断。缺少目标、目录或类型信息时，不能跳过对应条件；`contains` 仅检查实际本地目录及其祖先。右键子菜单和执行操作使用原会话的目标重新检查。

### Info

```json
"info": {
  "priority": 80,
  "match": { "extensions": ["txt", "md"], "kinds": ["file"] }
}
```

### Preview

```json
"preview": {
  "priority": 60,
  "maxBytes": 1048576,
  "match": { "extensions": ["txt"], "kinds": ["file"] },
  "panel": { "entry": "ui/preview.html", "title": "Text" },
  "loading": { "entry": "ui/loading.html" }
}
```

`loading` 是可选的**加载页**，与 `panel`（预览面板入口）相互独立：

- 声明后，Core 在调用 `preview.open` 前先解析该文档，并直接在 Core 拥有的沙箱 iframe 中打开，而不是先显示 Core 自己的加载/空白预览；`preview.open` 返回合法结果后再切换为最终预览。
- `loading.entry` 只能是插件包内的相对路径（仅普通路径段，不能是 URL、绝对路径、盘符、`..` 或反斜杠），读取时还会做 canonical 包含检查；文档上限 1 MiB。
- 加载页是自包含 HTML，不携带预览 Session，不能调用 `plugin.invoke`、文件、网络或命令；只能通过面板桥读取 `context.get`（文件展示名/扩展名）、`theme.get`/`theme.changed` 以及 `plugin.error`。
- 没有 `loading` 的旧声明保持原行为：Core 显示内置加载态，直到 `preview.open` 返回。
- 声明了 `loading` 时，Core 会先显示加载页，再在后台加载真正的预览 iframe；插件在首次数据渲染完成后调用桥接方法 `preview.ready`，Core 才移除加载页。这样不会出现“插件自己的 spinner/空壳”阶段。
- `preview.ready` 只能由真正的预览会话调用，加载页没有预览 Session，不能调用；30 秒未收到时 Core 会兜底显示预览，避免第三方插件不实现时永久卡住。

### Connections

```json
"connections": {
  "title": "WebDAV",
  "fields": [
    { "id": "url", "label": "服务器", "type": "url", "required": true },
    { "id": "username", "label": "用户名", "type": "text" },
    { "id": "password", "label": "密码", "type": "secret" }
  ]
}
```

字段类型：`text | url | number | checkbox | secret`。`secret` 只进入系统钥匙串，不写入普通 settings。

### VFS

```json
"vfs": { "title": "WebDAV", "readOnly": false }
```

连接想返回文件树时必须同时注册 `connections` 和 `vfs`。

### Toolbar / Search / Hooks

```json
"toolbar": {
  "title": "Git",
  "icon": "git-branch",
  "panel": { "entry": "ui/git.html", "title": "Git" }
}
```

```json
"search": { "icon": "magnifying-glass", "scope": "global" }
```

```json
"hooks": {
  "event": "beforeOpen",
  "priority": 100,
  "match": { "extensions": ["zip"], "kinds": ["file"] }
}
```

### UI timeout

```json
"ui": { "timeoutMs": 300000 }
```

`timeoutMs` 表示一次插件 UI 请求（面板握手、`plugin.invoke`、预览列举）允许的
最长时间，取值 1000–900000，默认 15000。Core 用它设置面板桥接超时，并把该插件
单次 RPC 的默认截止时间（120000）提高到至少该值（只延长、不缩短），同时通过
`context.get` 的 `uiTimeoutMs` 字段告诉插件 UI，使插件自己的请求计时器与 Core
一致。只有确实需要长时间列举/预览的插件（例如压缩包目录）才应调大该值。

### Themes

```json
"themes": { "path": "theme.json" }
```

主题包只能包含一个主题文件；主题 id、名称和 token 在 `theme.json` 内定义。

## 权限模型

| 路径                                            | 含义                                  |
| ----------------------------------------------- | ------------------------------------- |
| `metadata.enabled`                              | 文件和文件夹共用的属性读取            |
| `files.read/write/create/delete/rename.enabled` | 文件内容/对象操作，逐项批准           |
| `folders.read/create/delete/rename.enabled`     | 目录遍历/对象操作，逐项批准           |
| `network.enabled/domains/all`                   | HTTP/TCP 访问目标；`all` 表示任意主机 |
| `commands.enabled/allow`                        | Core 代理执行外部命令                 |
| `hooks.enabled`                                 | 生命周期钩子                          |

申请不等于批准，批准也不等于资源范围。实际有效范围：

```text
Manifest 申请 ∩ 安装时批准的声明快照 ∩ 用户 Grant
  ∩ PermissionPolicy ∩ 当前 Event/Operation Scope
```

- 安装/升级批准会记录当时的声明快照；之后 Manifest 即使新增权限，运行时也不能获得，必须替换为新版本并重新走安装/升级授权。
- 用户关闭单个权限不会停用插件；下一次真实操作用 Core 统一弹窗重新申请。
- 未声明的权限永远不会出现弹窗，直接拒绝。
- `commands.allow` 是命令的硬上限；`network` 的精确主机由 Operation Scope 和用户授权共同决定。

### 网络白名单与全部网络

```json
"network": { "enabled": true, "domains": ["api.example.com"] }
```

```json
"network": { "enabled": true, "all": true, "domains": [] }
```

`all: true` 在安装/激活界面标红警告；用户仍可切换为白名单。网络连接必须经 `ctx.http()` / `ctx.tcp()`，由 Core Broker 建立。

## 校验规则

Manifest 在安装、加载和插件进程启动前都会校验；未知/旧字段不会被静默接受，类型错误、路径逃逸、重复/保留 id 和不合法域名会直接拒绝。

## 界面语言

Core 支持 `zh-CN`（简体中文，默认）、`zh-TW`（繁体中文）和 `en`。在外观设置里选择语言，点击确认后立即应用并保存；不会重启或重新安装插件。

清单可在顶层声明 `locales`，以源文案为键。Core 只翻译插件名称、说明及贡献中的 `title`、`label`、`description`、`placeholder` 展示字段；`id`、`match`、路径、默认值及用户输入保持不变。缺失条目回退到原文案。翻译不影响权限声明。

```json
"locales": {
  "en": { "校验和": "Checksums" },
  "zh-TW": { "校验和": "校驗和" }
}
```

插件 UI 的桥接初始化携带当前 `locale`，每种 `context.get` 也返回 `locale`。所有沙箱面板、预览页、加载页及连接页都能调用只读 `locale.get`，并收到同一 MessagePort 上的 `locale.changed` 事件。插件不能修改 Core 语言。

UI SDK 提供 `client.i18n.getLocale()`、`subscribe()` 和 `bind()`；`createI18n()` 提供文案目录、插值、缺失回退和变更订阅。React 应在应用根组件用 `useSyncExternalStore` 订阅运行时；切换语言仅重新渲染，不清空表单或重启任务。

原生 RPC 在 JSON-RPC **请求信封**携带 `locale`，不占用或修改业务 `params`。Rust 使用 `ctx.locale()` 读取；数组、空值及业务自定义的 `params.locale` 均保留原义。SDK 将语言绑定到请求执行线程，后台任务继承创建请求的语言；同一进程中的并发请求不会互相修改语言。未知或缺失语言回退 `zh-CN`。

翻译仅用于界面文案。插值中的文件名、路径、邮件、提交说明及预览文档内容不得二次翻译。
