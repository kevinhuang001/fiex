# 09 · 运行时 UI 与面板

## 两类 UI

### Toolbar / Preview 面板

- 插件在 Manifest 中声明 `panel.entry`，只能指向包内 HTML；
- Core 校验路径、大小（1 MiB）和包归属；
- Core 提供面板外壳（标题、关闭、尺寸）；
- 面板 HTML 通过受控 CSP 加载；
- 面板与 Core 通过 MessagePort/UI Bridge 通信；
- 需要更长的握手/请求预算时，在 Manifest 声明
  `extensions.ui.timeoutMs`（默认 15000，范围 1000–900000）。Core 将其用于
  面板桥接，并把单次插件 RPC 的默认截止时间（120000）延长到至少该值，同时
  通过 `context.get.uiTimeoutMs` 暴露给插件 UI。

Toolbar 固定方法：

```text
toolbar.invoke
```

有 `panel` 时点击工具栏按钮打开面板；没有 panel 时调用 `toolbar.invoke`。

### Preview 加载页

预览提供者可以在 `extensions.preview.loading.entry` 额外声明一个包内加载页。
Core 启动插件预览时按以下顺序处理：

1. 校验插件审批状态、包归属与 `loading.entry`（与 panel 相同的路径、canonical
   包含和 1 MiB 限制）；
2. 通过 `preview_loading_asset`（桌面，asset 协议 + 面板 CSP）或
   `preview_loading`（Web/演示，srcDoc）取得加载文档；
3. 在 Core 拥有的 `sandbox="allow-scripts"` iframe 中立即显示加载页；
4. 调用 `preview.open`；合法结果返回后在同一个预览位置替换为最终预览。

加载页不是预览 Session：它没有资源 Scope，桥只允许 `context.get`、
`theme.get`/`theme.changed` 和 `plugin.error`。真正预览的 iframe 仍使用
`PreviewView` 对应的 Session，因此文件、网络和命令权限不会被加载页继承。
没有声明 `loading` 的插件保持旧的 Core 加载态行为。

### Connection PluginView

- `connection.open/create` 返回 `pluginView`；
- 完全替换右侧主内容区，不替换左侧导航、窗口和全局 TabHost；
- 只能加载所属插件包内的 HTML；
- 支持 `tabTitle`（最多 80 字符）；
- 不接受外部 URL、弹窗、窄面板或其他导航类型。

## 消息边界

- iframe 不直接访问 Tauri API、DOM、文件系统或 Keyring；
- 所有资源请求由 UI Bridge → native SDK → Host Broker；
- Core 校验 Session、插件身份、资源 Scope 和消息 Schema；
- 加载 HTML 本身不授予任何文件/网络权限；
- 声明了 `files.create` 或 `folders.create` 的插件可以在 `plugin.invoke`
  参数中显式命名当前目录（例如 `root`），Core 才会把该目录作为本次操作的
  输出根授予；未命名时仍只授予选中项。

## 面板生命周期

- 打开：Core 校验 Manifest、权限、入口路径；
- 切换标签：full-view 面板保持挂载；
- 关闭：释放 iframe、MessagePort、后台请求和授权引用；
- 插件停用、撤权、卸载、导航离开：面板失效并释放资源；
- 远程/异步响应在 Session 失效后必须丢弃。

## CSP

面板文档由 Core 生成独立的 CSP：

```text
default-src 'none';
script-src 'unsafe-inline';
style-src 'unsafe-inline';
img-src data:;
font-src data:;
connect-src 'none';
```

因此面板不能外链脚本/网络请求；需要网络时走 native SDK 的 Host Broker。

## 当前限制

- 面板不提供任意宿主快捷 API；
- 主题只支持单主题文件；
- 插件 iframe 与 Core 的 UI Bridge 方法以实际 SDK 为准，未列出的方法不存在。
