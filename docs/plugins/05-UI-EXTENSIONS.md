# 05 · UI 扩展与功能插件

[索引](README.md) · [上一篇：IPC](04-IPC-PROTOCOL.md) · [下一篇：任务](06-TASKS.md)

## 1. 扩展位置和原则

**插件提供 UI 结构和行为，Core 负责实际渲染。** 插件不直接依赖 React、Tauri、DOM、shadcn 或应用内部 hook；官方与第三方均使用相同的贡献声明。连接插件也可以提供菜单/面板，功能插件也可以提供 VFS 目录树。

v1 公开扩展点：

| contribution | 位置 | 来源 |
| --- | --- | --- |
| `contextMenus` | 文件、目录、空白区域、多选、标签等右键 | manifest 静态声明 + 动态可见性 |
| `toolbar` | 主工具栏按钮/下拉 | manifest |
| `panels` | popover、侧栏、模态面板 | schema + plugin.panel.* |
| `commands` | 统一命令 ID、快捷键候选 | manifest |
| `connectionFactories` | “新建连接”入口与设置表单 | manifest/schema |
| `mounts` | 侧栏设备/云端/虚拟目录 | 插件请求 host.mount.create |

`contributionId` 必须带插件命名空间，如 `org.example.git:stage`；Core 可以重新排序/隐藏贡献，但不可直接调用该插件的私有 UI 代码。

## 2. manifest 贡献示例

```json
{
  "contributes": {
    "toolbar": [
      {
        "id": "git.main",
        "title": "源代码管理",
        "icon": "git-branch",
        "when": "context.selection.allLocal",
        "openPanel": "git.mainPanel"
      }
    ],
    "contextMenus": [
      {
        "id": "git.stage",
        "title": "暂存选中文件",
        "icon": "plus",
        "when": "context.selection.canRead && context.selection.count > 0",
        "command": "git.stage"
      }
    ],
    "panels": [
      {
        "id": "git.mainPanel",
        "title": "源代码管理",
        "placement": "popover",
        "schemaResource": "ui/git-panel.json"
      }
    ]
  }
}
```

示例为目标设计，条件语法在正式 Schema 中需要枚举合法字段及操作符。**绝不把 `when` 作为任意 JavaScript 求值**；支持的表达式只读宿主白名单上下文。复杂 Git 仓库探测按目录缓存、异步 `plugin.command.probe` 查询，必须限时且避免逐帧/逐鼠标动作启动子进程。

## 3. 上下文快照与执行

插件每次执行命令获得一个**受限上下文快照**，而非任意应用内存引用：

```json
{
  "invocationId": "inv-77",
  "windowId": "main",
  "paneId": "left",
  "directory": { "mountId": "local-001", "nodeId": "dir-1" },
  "selection": [
    { "mountId": "local-001", "nodeId": "file-5", "name": "src.rs" }
  ],
  "capabilities": ["read"],
  "expiresAt": "2026-10-22T12:15:00Z"
}
```

在调用前重新校验节点是否仍存在、权限/版本是否变化。过期的菜单动作要失败并提示刷新；插件不能仅凭 snapshot ID 读取未授权目录。多窗口、双面板中使用 `windowId/paneId` 避免串状态。

## 4. 声明式面板 Schema

推荐 v1 标准组件白名单：`row/column`、`text`、`icon`、`button`、`input`、`select`、`checkbox`、`tabs`、`list`、`tree`、`table`、`progress`、`diff`、`filePicker`。每种组件的属性与事件由 JSON Schema 严格限定；文本默认转义，图标必须在允许资源范围内，路径/链接不得自动执行。样例：

```json
{
  "panelId": "git.mainPanel",
  "revision": 1,
  "root": {
    "type": "column",
    "children": [
      { "type": "text", "value": "当前分支：main" },
      {
        "type": "input",
        "id": "commitMessage",
        "label": "提交说明",
        "value": ""
      },
      {
        "type": "button",
        "label": "提交",
        "command": "git.commit",
        "submitValues": ["commitMessage"]
      }
    ]
  }
}
```

打开面板：Core 创建 `panelInstanceId` → `plugin.panel.open` → 收到 schema → Fiex 原生 UI 渲染；输入事件经 `plugin.panel.event` 传送，仅允许当前实例消费，后续 `host.ui.patch` 按 `revision` 有序应用增量数据。关闭时释放实例/订阅。表单校验在 Core 作基础约束，插件做业务校验。插件的计算过程不能阻塞 UI 线程。

## 5. 连接配置 UI 同样由插件声明

“添加连接”由已安装的 `connectionFactories` 枚举生成，不准在 `ConnectionDialog.tsx` 写死“WebDAV / SMB”。

```json
{
  "id": "webdav.connection",
  "displayName": "WebDAV",
  "fields": [
    { "id": "url", "type": "url", "label": "地址", "required": true },
    { "id": "username", "type": "text", "label": "用户名" },
    { "id": "password", "type": "secret", "label": "密码" }
  ]
}
```

`secret` 字段交由 Core SecretStore 持久化；插件只在获得授权的会话获取值或受限凭据句柄，Core 数据库、普通日志与 UI 面板状态不得持久化密码明文。点击连接由 `plugin.connection.validate/open` 完成。

## 6. 插件管理设置页

设置 → 插件：列出已安装、版本、发布者、权限、平台兼容、健康状态、更新与启用开关，提供“安装本地插件包”入口及第三方来源链接。官方/第三方**使用相同卡片与安装流程**；可以展示可核实的发布者信息，但发布者标识不赋予私有 Core API。

连接实例单独展示在侧边栏“连接”中；插件启用状态和连接实例状态是两个概念。禁用/卸载时 UI 贡献立即撤销，进行中任务按 [06](06-TASKS.md) 的取消策略结束。

## 7. 自定义 WebView（后续扩展，非 v1 阻塞项）

未来可为无法用 schema 表达的复杂 UI 增加隔离 WebView 容器。但不能在主 React 窗口直接执行插件 HTML/JS、也不得暴露 Tauri 原生命令。必须先定义 CSP、origin 隔离、权限消息代理、资源访问策略、导航/外链限制和生命周期，否则不启用该能力。
