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

| 扩展点 | 形态 | Core 固定调用 |
| --- | --- | --- |
| `contextMenus` | `true` 或 `{match}` | `contextMenu.resolve` / `contextMenu.execute` |
| `decorations` | `{match?, priority?}` | `decorations.resolve` |
| `info` | `{match?, priority?}` | `info.resolve` |
| `preview` | `{match?, priority?, maxBytes?, panel}` | `preview.open` |
| `connections` | `{title, fields}` | `connection.validate/open/create` |
| `vfs` | `{title, readOnly?}` | `vfs.list/stat/read…` |
| `search` | `{icon?, scope?}` | `search.query` |
| `toolbar` | `{title, icon?, panel?}` | `toolbar.invoke`，有 panel 时打开面板 |
| `themes` | `{path}` | 主题资产加载 |
| `settings` | `[{id,label,type,…}]` | 设置字段；字段 id 是存储 key |
| `hooks` | `{event, priority?, match?}` | `hooks.invoke`（合同已固定，生命周期接线未全部完成） |

不再接受：

- `connections`/`vfs`/`toolbar`/`search`/`hooks` 的数组；
- `info`/`preview`/`connections`/`vfs`/`toolbar`/`search`/`hooks` 的贡献 `id`；
- `toolbar.command`、`info.command`、`hooks.method`、`search.command`、`search.title`、`info.title`、`preview.title`。

## 各扩展点示例

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
  "panel": { "entry": "ui/preview.html", "title": "Text" }
}
```

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

### Themes

```json
"themes": { "path": "theme.json" }
```

主题包只能包含一个主题文件；主题 id、名称和 token 在 `theme.json` 内定义。

## 权限模型

| 路径 | 含义 |
| --- | --- |
| `metadata.enabled` | 文件和文件夹共用的属性读取 |
| `files.read/write/create/delete/rename.enabled` | 文件内容/对象操作，逐项批准 |
| `folders.read/create/delete/rename.enabled` | 目录遍历/对象操作，逐项批准 |
| `network.enabled/domains/all` | HTTP/TCP 访问目标；`all` 表示任意主机 |
| `commands.enabled/allow` | Core 代理执行外部命令 |
| `hooks.enabled` | 生命周期钩子 |

申请不等于批准。实际有效范围：

```text
Manifest 申请 ∩ 用户 Grant ∩ PermissionPolicy ∩ 当前 Event/Operation Scope
```

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
