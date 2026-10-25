# 09 · ContentHost、PluginView 与运行时 UI

## 1. 右侧主内容区

Fiex 右侧主内容由 `ContentHost` 管理，导航类型如下：

```rust
enum ContentDestination {
    Explorer { location: ResourceRef },
    PluginView { owner: PluginId, entry: PackageEntry, state: JsonValue },
}
```

`Explorer` 负责本地或 VFS 的目录视图；`PluginView` 用一个 sandboxed iframe 完整接管右侧主内容区（目录面包屑、工具栏、列表、预览占位都不再显示）。左侧 Sidebar、应用全局标签页与窗口框架始终由 Core 管理。

## 2. Connection 导航

```text
Sidebar connection item / Add connection +
  → Core connection.open / connection.create
  → Plugin returns NavigationTarget
       kind=vfs        → ContentHost Explorer(VFS mount)
       kind=pluginView → ContentHost PluginView(package HTML)
```

导航只有上述**两种**。Connection 不使用弹窗、局部 iframe、独立侧面板或外部网页跳转作为点击结果。账号授权、服务状态、配置页面均可以放在完整的 PluginView 中；需要文件浏览时可切换到 VFS。网盘想导航到 VFS，必须同时注册 `extensions.connections` 与 `extensions.vfs`。

## 3. PluginView 目标合同

```json
{
  "kind": "pluginView",
  "view": { "source": "package", "entry": "ui/cloud.html" },
  "state": { "connectionId": "account-1", "page": "home" }
}
```

Core 校验本插件的 bundle 路径、归属、当前 InteractionSession、state 大小和 Schema，创建 PluginViewSession，生成只属于该 iframe 的 MessagePort，注入最小启动上下文（插件身份与获准的连接/导航引用）。**不会因为 iframe 初始化而自动提供本地文件 Metadata 或内容**。

`state` 是大小受限的业务视图状态，不是访问令牌，不含密码、OAuth Token、宿主路径或文件内容。插件 UI 要操作文件，必须通过 SDK/受控 UI Bridge 向 Core 申请当次 Scope 内的资源。

## 4. PluginView 生命周期

- ContentHost 保存标签页类型、所属插件、连接实例、当前导航历史引用与会话 ID。
- 前进/后退与切换标签由 Core 管理；PluginView 可内部更新页面状态，但不能伪造系统导航或跨插件切换。
- 重建/恢复时由 Core 检查插件仍启用、连接仍存在、UI 资产有效及权限批准；失败则显示不可用页面而非加载旧授权引用。
- 关闭标签、切换资源、注销/禁用插件、删除连接或撤销权限时终止 MessagePort、事件订阅、流引用、在途任务及 iframe。
- 主内容 iframe 不可直接覆盖 Sidebar、宿主菜单、全局标题栏或其他 tab 的 DOM。

## 5. 其他扩展点的运行时 Panel

Connection 主导航以外的用户操作（如 Toolbar 点击、有效右键 action、Preview 内明确的编辑动作、Info 项交互、Search 结果操作）可使用通用 `host.ui.open` 请求一个独立 Panel：

```json
{
  "method": "host.ui.open",
  "params": {
    "interactionId": "interaction-opaque",
    "view": { "source": "package", "entry": "ui/options.html" },
    "presentation": { "placement": "dialog", "title": "高级选项" }
  }
}
```

Plugin preview HTML renders content only: Core owns the preview title/close/navigation header, so the document must not draw its own title bar. Toolbars/tabs are allowed; a duplicated file/dataset header is not.

此 Panel 与 Connection 的完整 PluginView 是不同的宿主布局方式；二者共享 iframe 安全与受控 UI Bridge。自动 Decoration 刷新、后台同步、Menu.resolve 查询本身不生成有权弹出 Panel 的交互身份。插件不需要在 Manifest 列出具体 UI 页面。

## 6. UI Bridge 安全

iframe 使用无同源权限的 sandbox、受限 CSP、包内静态资产校验和专用 MessagePort；不获得 Tauri/Node/宿主 DOM/浏览器通用跨域权限。每条消息必须验证来源 Port、插件实例、Session、Schema、允许方法和尺寸上限；Native Plugin 处理器仍须经 Core PermissionPolicy 审核实际文件/网络/命令访问。大文件使用资源代理/流而非大型 JSON 消息。

## 7. 验收

点击连接 VFS 后出现 Core Explorer，点击 iframe 型连接后**整块右侧**切换到 PluginView；没有残留本地目录面包屑、原文件工具栏或其他 Explorer 元素。全局 Sidebar、TabHost 和前进/后退正常。无权的 mount、其它插件 HTML、过期交互、外部 URL、非法包内路径、跨 iframe DOM 和 Tauri API 请求均被拒绝。
