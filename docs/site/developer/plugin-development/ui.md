# 插件界面与 UI SDK

Rust 插件适合处理文件、网络和连接逻辑；随包提供的页面适合展示设置表单、连接内容或预览。两者通过 Fiex 提供的 UI bridge 交互，不共享任意 DOM 或文件路径访问权限。

## 三种常见页面

| 页面用途 | 注册位置 | 页面如何打开 |
| --- | --- | --- |
| 工具栏面板 | `extensions.toolbar.panel.entry` | 用户点击插件工具栏项时显示面板 |
| 连接自定义视图 | `ConnectionProvider::open/create` 返回 `PluginView` | 用户打开该连接后显示全宽插件视图 |
| 文件预览 | `PreviewProvider` 返回 package `PreviewView` | 用户选中匹配文件并打开预览 |

若连接是文件树，优先让 `ConnectionProvider` 返回 Core VFS 挂载，而不是自己在 UI 页面里仿造文件浏览器。UI Bridge 没有 VFS 文件读写 API。

## 页面文件与 package-relative path

Manifest 引用包内页面，例如：

```json
{
  "extensions": {
    "toolbar": {
      "title": "Notes",
      "icon": "note",
      "panel": {
        "entry": "ui/index.html",
        "title": "Notes",
        "defaultSize": { "width": 460, "height": 340 }
      }
    }
  }
}
```

`entry` 是插件包内部的相对路径。页面必须随插件包一起分发；Core 会拒绝绝对路径、URL、`..` 路径段和越出包目录的路径。若打包时漏掉 `ui/index.html`，插件构建成功仍会在运行时打不开面板。

## 初始化 UI SDK

使用 UI SDK 的客户端对象，而不是假设页面能访问任意父窗口对象：

```ts
import { FiexUiClient } from '@fiex/ui-sdk'

const fiex = new FiexUiClient()
const context = await fiex.context.get()
await fiex.clipboard.writeText('已复制')
await fiex.navigation.open('fiex://mount/example/')
```

具体 API 以当前 Core 锁定的 [UI SDK 源码](https://github.com/kevinhuang001/fiex-sdk/tree/24664a69bb411d0b2608461ae08a588f28cbe385/ui)为准。常用能力包括：

- `context.get()`：读取当前面板/预览上下文、贡献 ID、选择和主题信息。
- `plugin.invoke(...)`：向 native 插件请求自定义业务方法。
- `plugin.task(taskId)` 与 `tasks.*`：查看/操作 native 长任务。
- `panel.close()` / `panel.resize(...)`：控制插件面板生命周期。
- `preview.ready()`：预览完成首屏内容绘制后通知 Core。
- `dialog.*`：通过 Core 打开文件/目录/保存对话框。
- `theme.get()` / `theme.subscribe(...)`：读取并响应宿主主题变化。
- `i18n`：读取当前语言并订阅语言变更。
- `errors.report(...)`：将用户可理解的错误交给 Fiex 显示。

确切类型和参数可能随 SDK 小版本变化；不要直接把未检查的最新 UI SDK 构建结果与旧 Core 组合。

## 与 Rust 后端通信

页面通过 `plugin.invoke` 请求业务动作，native 后端在 `serve_with` 的 fallback 中处理相应方法。对于读取用户选中的文件，应由 native 方法使用传入的 `ResourceRef` 调用 `ctx.files()`；不要把路径字符串发给 UI 作为授权证明。Core 对每次 native Host call 独立校验用户 grant 和当前操作 scope。

低频、短小的 UI 查询可使用普通调用。耗时操作应由 native 插件创建 task，并通过 Task API 上报状态和取消；页面关闭不应让本地任务状态变成不可知的全局状态。

## 安全与交互

- 使用宿主提供的文件/目录对话框，不自行请求浏览器文件权限。
- 不把 access token、密码或私钥写进前端存储、URL、日志或错误文本；凭据使用 SDK Secrets/OAuth 服务。
- 将文件名、服务器响应和用户输入当作不可信文本渲染，避免插入原始 HTML。
- 为按钮提供进度、禁用/错误状态和可访问名称；关闭面板前提醒尚未保存的更改。
- 内容更改可能发生在运行期间，调用前应基于当前 `context` 和 native 端校验。

## 参考

- [Hello UI Bridge 示例](https://github.com/kevinhuang001/fiex-official-plugins/tree/main/example/hello/ui)
- [UI SDK API](https://github.com/kevinhuang001/fiex-sdk/tree/24664a69bb411d0b2608461ae08a588f28cbe385/ui)
- [UI bridge IPC 方法审计](https://github.com/kevinhuang001/fiex-sdk/blob/24664a69bb411d0b2608461ae08a588f28cbe385/docs/ipc-audit.md)
