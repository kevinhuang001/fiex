# 09 · 运行时 UI 与面板

## 两类 UI

### Toolbar / Preview 面板

- 插件在 Manifest 中声明 `panel.entry`，只能指向包内 HTML；
- Core 校验路径、大小（1 MiB）和包归属；
- Core 提供面板外壳（标题、关闭、尺寸）；
- 面板 HTML 通过受控 CSP 加载；
- 面板与 Core 通过 MessagePort/UI Bridge 通信。

Toolbar 固定方法：

```text
toolbar.invoke
```

有 `panel` 时点击工具栏按钮打开面板；没有 panel 时调用 `toolbar.invoke`。

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
- 加载 HTML 本身不授予任何文件/网络权限。

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
