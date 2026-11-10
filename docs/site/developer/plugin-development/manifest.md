# Manifest 与权限

Manifest 是插件包的声明文件，文件名必须为 `manifest.json`。Core 当前识别 `schemaVersion: 2`；Native 插件声明 `runtime.protocolVersion: "7.0"`。未知字段或无效扩展注册会导致 manifest 被拒绝。

## 最小结构

```json
{
  "schemaVersion": 2,
  "id": "org.example.notes",
  "name": "Notes connector",
  "version": "0.1.0",
  "description": "Connects to a notes service.",
  "runtime": {
    "protocolVersion": "7.0",
    "entrypoints": {
      "windows-x86_64": "fiex-notes.exe",
      "macos-aarch64": "fiex-notes",
      "macos-x86_64": "fiex-notes",
      "linux-x86_64": "fiex-notes"
    }
  },
  "permissions": {},
  "extensions": {}
}
```

插件 ID 必须是包含点号的稳定反向域名风格 ID，例如 `org.example.notes`。平台入口必须是插件包内的相对文件名，不能使用绝对路径、上级目录或任意 URL。只列出实际构建和打包的平台。

## 三个声明层不要混为一谈

| Manifest 部分 | 说明                                         | 它不代表什么                   |
| ------------- | -------------------------------------------- | ------------------------------ |
| `runtime`     | 插件进程入口、native 协议版本                | 不代表已安装、已启用或已获授权 |
| `extensions`  | 该插件能提供哪些连接、菜单、预览、VFS 等扩展 | 不代表有权访问任意文件         |
| `permissions` | 插件请求的文件、网络、命令等能力             | 不代表用户已经批准这些能力     |

Core 根据 `extensions` 建立 provider 索引，再按事件、匹配规则和运行时注册信息分发请求。用户需要在启用时审查权限并作出选择。详见[插件权限与安全边界](/reference/security)。

## 权限字段

权限采用带 `enabled` 字段的对象。逐项声明所需能力，避免为了“以后可能需要”而过度申请：

```json
{
  "permissions": {
    "metadata": { "enabled": true },
    "folders": {
      "read": { "enabled": true },
      "create": { "enabled": false },
      "delete": { "enabled": false },
      "rename": { "enabled": false }
    },
    "files": {
      "read": { "enabled": true },
      "write": { "enabled": false },
      "create": { "enabled": false },
      "delete": { "enabled": false },
      "rename": { "enabled": false }
    },
    "network": {
      "enabled": true,
      "all": false,
      "domains": ["api.example.com"]
    },
    "commands": { "enabled": false, "allow": [] },
    "hooks": { "enabled": false }
  }
}
```

- `metadata` 仅表示可读取经过授权投影的属性；列目录和读取内容分别需要目录/文件权限。
- 网络目标写精确域名；`network.all: true` 会请求更宽的任意主机访问。
- `commands.allow` 列出允许请求的命令名称。命令以当前用户身份运行，权限范围可能超出 Core 文件 API，必须说明必要性。
- Hooks 需要 `hooks.enabled`，且仍受 hook 事件与匹配规则约束。
- 没有使用的权限保持 `false`，网络域名和命令列表保持为空。

当前字段的规范类型见 [`PluginPermissions`](https://github.com/kevinhuang001/fiex/blob/main/src-tauri/src/plugins/host/manifest.rs) 与 [`declared_permissions`](https://github.com/kevinhuang001/fiex/blob/main/src-tauri/src/plugins/host/manifest/permissions.rs)。

## Extensions 的组织方式

扩展全部放在 `extensions` 下。常见项如下：

| 键                       | 用途                                   | 实现入口                         |
| ------------------------ | -------------------------------------- | -------------------------------- |
| `connections`            | 声明用户可创建的连接类型和输入字段     | `ConnectionProvider`             |
| `vfs`                    | 声明插件提供文件树/文件空间            | `VfsProvider`                    |
| `info`                   | 声明信息面板区块与字段                 | `InfoProvider::compute`          |
| `preview`                | 注册匹配文件的预览能力                 | `PreviewProvider`                |
| `decorations`            | 注册动态列表装饰                       | `DecorationProvider`             |
| `contextMenus`           | 注册文件/目录菜单项                    | `ContextMenuProvider`            |
| `connectionContextMenus` | 注册连接本身的菜单项                   | Connection Context Menu provider |
| `search`                 | 提供可合并到主搜索的结果               | `SearchProvider`                 |
| `hooks`                  | 声明 `beforeOpen` / `afterOpen` 等事件 | `HookProvider`                   |
| `toolbar`                | 工具栏命令或插件面板                   | `ToolbarProvider`                |
| `settings`               | 声明插件设置字段                       | 插件设置 UI/API                  |
| `themes`                 | 注册 package 内的声明式主题            | 无 native provider               |

扩展通常可设置 `match` 和 `priority`。Match 仅控制哪些对象适合接收 provider 请求，不是权限规则。Info 字段可在 manifest 的 `extensions.info.sections` 静态声明，并为每个字段选择 `compute: "onOpen"` 或 `"onDemand"`；Core 打开信息面板时不会为枚举字段而临时启动所有插件。

## 插件包内的文件

Manifest 引用的 entrypoint 和 UI 页面必须位于插件包目录中。页面路径用 `/` 分隔的包内相对路径；不要通过 `../`、绝对路径、URL 或符号链接试图跳出包目录。打包时检查所有声明的入口都确实进入 `.fiexplugin`。

示例完整 manifest：[Hello 插件](https://github.com/kevinhuang001/fiex-official-plugins/blob/main/example/hello/manifest.json)。当前细分实现见 [Provider 实现](/developer/plugin-development/providers)与[插件界面](/developer/plugin-development/ui)。
