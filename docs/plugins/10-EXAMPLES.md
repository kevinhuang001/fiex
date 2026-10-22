# 10 · 官方与第三方插件的完整能力场景

[索引](README.md) · [上一篇：迁移测试](09-MIGRATION-TESTING.md)

## 1. WebDAV（官方连接插件）

**声明能力**：`filesystem`、`connectionFactory`、`panel`；支持 read/list/stat、按服务器支持情况选择 write/delete/move/rangeRead/conditionalWrite/changeFeed。WebDAV 插件处理 HTTP、PROPFIND、认证、ETag、Range（按服务端能力）和状态码映射，**不**直接决定下载到哪里、哪些目录同步或何时弹出冲突。

```text
用户“新建连接”
→ Fiex 从插件 manifest 获取 URL/用户名/secret 字段
→ plugin.connection.validate/open
→ host.mount.create(pluginId, rootId, capabilities)
→ 侧边栏出现挂载
→ 访问目录：Core → plugin.fs.list
→ 用户“离线可用”：Core CacheManager → plugin.fs.openRead
→ 用户选择双向：Core SyncEngine → getChanges/stat/conditionalWrite
→ 下载、上传、冲突、取消均由 Core 执行或协调
```

原先 `~/.fiex/drives` 镜像不再是浏览树的权威来源，未下载文件不制造真实空文件。

## 2. SMB（官方连接插件）

SMB 插件负责主机发现（如选择提供）、共享、认证、列目录、文件属性、版本条件（若后端可用）、读写/移动/删除。仅作为普通独立进程运行；Core 同样按 `mountId/nodeId` 路由。与 WebDAV 不同的只是插件的远程协议和具体 capabilities，不应在 `transfer` 或 `Sidebar` 保留任何“这是 SMB”业务分支。

NAS 上数十万文件时列表分页，避免连接即全盘递归。同步引擎遇到无可靠变化游标时使用目录扫描；遇到弱版本时禁用无人值守的危险覆盖，要求用户处理冲突。

## 3. Git（官方功能插件，同时提供 VFS）

**声明能力**：`toolbar`、`contextMenu`、`panel`、`commands`、`filesystem`。Git 通过本机 `git` 命令操作授权的本地仓库路径，Fiex Core 不运行私有 `git_*` Tauri 命令。

- 工具栏显示 Git 图标，加载**Git 插件预编译 iframe 面板**（分支、远端、提交输入、变更列表、历史），经 `@fiex/ui-sdk` 调用 Git Native 命令，不得直接调用 Tauri invoke。
- 右键提供“暂存/取消暂存”等操作，按上下文判断目录是否为本地仓库。
- 用户选择历史 commit 后，Git 插件调用 `host.mount.create` 挂载只读提交树；目录 `fs.list/stat` 使用 Git 树对象，`fs.openRead` 返回对应 blob 流。
- 历史树由通用 Fiex Explorer 展示；只读内容可以由 Core CacheManager 缓存，但无意义的双向同步选项不出现。
- Git push/pull 启动活动中心任务，插件负责 Git 命令和请求取消后的子进程清理，Core 负责唯一任务 ID 与最终状态。

`git status` 是功能贡献的私有业务状态，只能通过权限允许的 probe/命令结果呈现，不能写入通用 VFS 文件类型内核特判。

## 4. Google Calendar（第三方或未来官方连接插件）

**声明能力**：`filesystem`、`connectionFactory`、`panel`，可选 `changeFeed`。插件登录 OAuth（由插件协议实现、Core 保存凭据引用），返回只读/按能力可编辑的日历树：

```text
Google Calendar
├── 个人
│   ├── 2026
│   │   ├── 10月
│   │   │   ├── 组会.ics
│   │   │   └── 项目会议.ics
│   └── 即将到来
└── 工作
```

`2026`、`10月`、`即将到来` 为插件合成的目录，不对应远端物理文件夹。同一事件出现在“月份”和“即将到来”时，视图节点有独立 nodeId，可用 `objectKey` 指向同一底层事件。`fs.openRead` 导出 iCalendar 字节，Core 负责内容缓存与离线钉选。

如果插件未实现安全的事件反向编辑与版本条件，Core **不得**显示通用文件夹双向同步；用户可以通过该插件提供的专用“编辑事件”面板修改 API 对象。即使拥有编辑面板，也不自动等于可将 `.ics` 变更双向同步。

## 5. 最小第三方演示插件

一个 `org.example.demo` 自带内存中的只读目录和 UI：

```json
{
  "schemaVersion": 1,
  "id": "org.example.demo",
  "name": "Demo Tree",
  "version": "0.1.0",
  "pluginProtocol": { "min": "2.0", "max": "2.x" },
  "capabilities": ["filesystem", "toolbar", "panel"],
  "ui": { "bridge": "1.0", "assetsRoot": "ui/" },
  "permissions": { "mount": true, "ui": true },
  "entrypoints": {
    "linux-x86_64-gnu": "bin/linux-x86_64-gnu/demo"
  },
  "contributes": {
    "toolbar": [
      {
        "id": "demo.open",
        "title": "示例",
        "icon": "folder",
        "openPanel": "demo.panel"
      }
    ],
    "panels": [
      {
        "id": "demo.panel",
        "title": "示例面板",
        "placement": "popover",
        "entry": "ui/main/index.html"
      }
    ]
  }
}
```

安装该包 → 接受授权 → 启动独立 Native 进程 → 自注册挂载 → 侧边栏出现虚拟目录 → 展开树/打开虚拟文本文件 → Core 自动缓存内容 → 点击工具栏加载预编译 iframe 面板 → 通过 UI Bridge 运行命令和控制 Core 任务 → 禁用插件后虚拟挂载与按钮一起消失，不影响 LocalFsAdapter。

额外提供**纯 UI 插件**测试：无 `bin/`，仅打包静态 `ui/main/index.html`、JS/CSS 与 manifest，验证 `@fiex/ui-sdk` 的主题、上下文和面板操作；由于没有后端，调用 `plugin.invoke` 必须得到 `unsupported`。

**此流程作为最终验收“官方与第三方无特殊差别”的基准**，在至少两个不同操作系统上进行端到端自动化，并搭配单独编写的非 Rust 协议客户端验证语言独立性。

更多线消息参见 [IPC](04-IPC-PROTOCOL.md)，实施与数据迁移参见 [09](09-MIGRATION-TESTING.md)。
