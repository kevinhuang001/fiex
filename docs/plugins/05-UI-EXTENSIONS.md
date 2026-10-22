# 05 · iframe UI 扩展规范（v1 提案）

[索引](README.md) · [上一篇：IPC](04-IPC-PROTOCOL.md) · [下一篇：任务](06-TASKS.md) · [UI SDK 与 Bridge](11-UI-SDK.md)

> **v1 决策**：插件开发者**必须交付已编译的 HTML/CSS/JavaScript 静态资源**；Fiex 将其作为隔离的 iframe 加载。Fiex 不编译 UI 源码、不运行 npm、不在产品运行时提供 Node.js。**不设计第二套声明式业务面板协议**。简单连接表单仍可由宿主根据数据字段定义生成，但所有自定义交互面板使用 iframe。以下为拟议规范，尚未实现。

## 1. 扩展点

| contribution          | 宿主负责                                  | 插件负责                       |
| --------------------- | ----------------------------------------- | ------------------------------ |
| `toolbar`             | 按钮尺寸、位置、排序、点击、可见性        | ID、标题、图标、动作           |
| `contextMenus`        | 文件/目录/多选右键、快捷键、权限          | 菜单项及命令                   |
| `panels`              | iframe 容器、位置、主题、会话、焦点、关闭 | 已编译 UI 目录、入口、前端逻辑 |
| `connectionFactories` | 标准连接入口、SecretStore、安全表单       | 字段定义与连接协议             |
| `commands`            | 权限和上下文校验、路由                    | 插件自定义操作                 |
| `mounts`              | 统一目录树、导航和缓存                    | VFS 树节点和内容               |

连接/功能仅是 UI 分类，不是互斥运行时类型。**功能插件可以提供完整虚拟目录**；连接插件也可提供自定义工具面板。一个插件可贡献多个面板和按钮。

## 2. 编译好的 UI 资源

```text
git.fiexplugin
  manifest.json
  bin/                       # 可选：需要 Native 后端时
    macos-aarch64/plugin
    windows-x86_64/plugin.exe
    linux-x86_64-gnu/plugin
  ui/
    main/index.html
    main/assets/index-XXX.js
    main/assets/index-XXX.css
    preferences/index.html
    preferences/assets/...
  icon.svg
```

UI 项须指定本包内静态入口（如 `ui/main/index.html`）；资源必须留在当前插件包的 `ui/` 下，构建器使用相对路径。构建后的 JS 可以包含 React/Vue/Svelte 的浏览器 bundle，**不得依赖 Node globals (`process`、`require`、`fs`)、dev server、远程 CDN 或 npm 安装**。

插件包可以：

- **UI-only**：只有 `ui/`，不包含 Native 后端；仅能使用经授权的 Host UI Bridge。
- **Native-only**：只有独立原生进程，无 UI。
- **Hybrid**：同时有 UI iframe 和独立 Native 进程；UI 使用 Bridge 调用其后端。

带 Native 的包仍按 [架构](01-ARCHITECTURE.md) 的进程管理/IPC 标准运行；UI-only 插件不应为了满足“独立进程”额外捆绑空的可执行文件。

## 3. manifest 示例

```json
{
  "contributes": {
    "toolbar": [
      {
        "id": "git.main",
        "title": "源代码管理",
        "icon": "git-branch",
        "when": "context.directory.isLocal",
        "openPanel": "git.panel"
      }
    ],
    "contextMenus": [
      {
        "id": "git.stage",
        "title": "暂存选中文件",
        "when": "context.selection.count > 0",
        "command": "git.stage"
      }
    ],
    "panels": [
      {
        "id": "git.panel",
        "title": "源代码管理",
        "entry": "ui/main/index.html",
        "placement": "popover",
        "defaultSize": { "width": 420, "height": 560 },
        "minSize": { "width": 320, "height": 260 },
        "maxSize": { "width": 900, "height": 900 }
      }
    ]
  }
}
```

ID 是 **pluginId 内部命名空间**：相同 ID 在不同插件中不得混用，不允许伪装宿主 command。`when` 仅使用有限、无副作用条件语法，不能运行任意 JS。复杂上下文探测由带限时和缓存的 Native 命令处理。

## 4. 资源加载与 iframe 隔离

- PluginHost 安装时校验所有资产的路径、长度和摘要。宿主使用**插件独立 origin** 加载 `index.html`；具体 URL scheme/host 是平台实现细节，不作为 Plugin Protocol 持久标识。不同插件和 Fiex 主界面不得同源。
- 默认禁止外网脚本/样式、任意表单跳转、弹窗、顶层导航与非授权媒体。采用严格 CSP（尤其 `script-src 'self'`、`connect-src 'none'` 或经审查的允许集）；不允许 `unsafe-eval`。资源仅加载已安装包中的静态文件；不要给插件页面 Tauri capability 或 `window.__TAURI__`。
- iframe 的 `sandbox` 策略按平台实施，至少限制 top navigation、非授权 popups、自动下载和表单提交。如果使用 `allow-scripts` 与 `allow-same-origin`，**MUST** 保证插件 iframe 与主页面永不属于同一 origin，并经过跨平台测试，不能只依赖 sandbox 进行隔离。
- 用户文件不可直接暴露为普通 iframe URL。预览、下载、外链和选取文件必须通过授权 Bridge 或 Core 控制的资源句柄。
- UI 资源版本随安装包固定，前端与 Native 可共用包版本，但 UI Bridge `major/minor` **独立于**插件 Native IPC 版本。

具体安全握手、消息和运行时能力由 [11 UI SDK](11-UI-SDK.md) 定义。

## 5. 面板生命周期

```text
Toolbar/context action → Core resolve contribution
    → create panelInstanceId + restricted context
    → create sandboxed iframe with per-plugin isolated origin
    → authenticated MessageChannel handshake
    → @fiex/ui-sdk.ready()
    → panel requests / events → HostBridge → Core → optional Native plugin
    → close/disable/update → revoke session, cancel panel subscriptions
```

UI 可以在**宿主给定的边界内**请求 resize/close/focus，但不能改变窗口安全边界、强制抢焦点或覆盖其他插件。popover 关闭后需要释放订阅和对象 URL，重新打开可创建新 `panelInstanceId`；插件升级时不复用旧会话端口。

多窗口、多面板并存时，每个 iframe 实例有独立会话、ID、速率限制和可访问资源范围。过期上下文、被撤销权限、节点发生改变等情况要返回明确错误。用户关闭面板**不应自动取消**已经转交 Native/Core 的持久任务；仅 UI 页面范围内的临时动作与订阅随面板释放。

## 6. 连接配置

连接插件可声明标准字段（URL、host、用户名、密码等），由 Fiex 生成一致的配置界面；此处字段定义是**数据配置 schema**，不是一个独立的业务面板 UI 引擎。插件如需高级配置、OAuth 引导或账号选择，可额外贡献 `ui/preferences/index.html` iframe 页面，所有凭据经 Core SecretStore 存储，不能通过 URL query、localStorage 或跨域脚本泄露。

## 7. 一致性和验收

- React/Vite 构建、纯 HTML 和 UI-only 示例插件均应通过相同 iframe Host Bridge 契约测试。
- UI 尝试调用 Tauri invoke、其他插件私有命令、未经授权本地节点必须被拒绝。
- 禁用/卸载插件时工具栏按钮、菜单、iframe、事件订阅全部释放；刷新/重启后无孤儿面板。
- 三平台（Windows/macOS/Linux）验证 ES module、CSS、字体/图片静态资源及 CSP 能正确加载；不能默认所有自定义 URL scheme 表现一致。
- **不将“以后再支持另一套 panel schema”作为兼容承诺**；未来可以增加可选 UI 辅助组件库，但 iframe + Bridge 是 v1 唯一可执行业务面板契约。
