# 21 · 插件 UI（iframe 面板）开发指南

[索引](README.md) · 相关：[05 UI 扩展点](05-UI-EXTENSIONS.md) · [11 UI SDK 与 Bridge](11-UI-SDK.md) · [22 主题包开发](22-THEME-AUTHORING.md)

本文是**可照着做**的面板开发手册：从零写出一个面板、在本地 dev server 里看到它、正确继承 Core 的主题色，以及避开沙箱和 CSP 的坑。协议细节仍以 05 / 11 为准。

## 1. 面板的运行边界

面板是一个**跨源 iframe**（`sandbox="allow-scripts"`，opaque origin），由 Core 加载包内已构建好的 HTML。它自带严格 CSP：

```text
default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline';
img-src data:; font-src data:; connect-src 'none'; form-action 'none'; base-uri 'none'
```

直接后果：

| 可以做                        | 不可以做                                                                    |
| ----------------------------- | --------------------------------------------------------------------------- |
| 内联/打包进 HTML 的 JS 与 CSS | `fetch` / XHR / WebSocket / 任何远程请求                                    |
| `data:` 图片、内联 SVG        | 远程图片、远程字体、CDN、`eval` / `new Function`                            |
| 通过 SDK 调用宿主能力         | `localStorage`（opaque origin 会抛异常）、`window.open`、表单提交、顶层跳转 |
| 通过插件后端读写文件          | 直接访问 `window.__TAURI__` / Tauri invoke                                  |

唯一对外通道是 `@fiex/ui-sdk`：它负责握手，并向宿主换一条专属 `MessagePort`。**不要自己写 `window.parent.postMessage`**，宿主只接受带 nonce 与会话 ID 的端口消息。

## 2. 最小可用面板

官方插件（`fiex-official-plugins`）的目录结构：

```text
official/<slug>/
  manifest.json
  ui/
    index.html
    package.json
    vite.config.ts
    src/main.tsx
    src/App.tsx
    src/style.css
    dist/index.html        # 构建产物，不进 git
```

`vite.config.ts` 使用 `vite-plugin-singlefile`，把所有 JS/CSS 内联进一个 `index.html`——因为面板只能加载包内静态文件：

```ts
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import { viteSingleFile } from 'vite-plugin-singlefile'

export default defineConfig({
  plugins: [react(), viteSingleFile()],
  build: { outDir: 'dist', emptyOutDir: true, target: 'es2022', assetsInlineLimit: 100000000 },
})
```

`src/main.tsx` 与普通 React 应用无异。面板代码这样接入宿主：

```tsx
import { useEffect, useState } from 'react'
import { FiexUiClient } from '@fiex/ui-sdk' // 官方插件在当前仓库内用 SDK 子模块的相对路径

const client = new FiexUiClient()

export function App() {
  const [files, setFiles] = useState<string[]>([])

  useEffect(() => {
    void client.context.get().then((context) => setFiles(context.selectionPaths))
  }, [])

  return (
    <main>
      <h1>{files.length} 个文件</h1>
      <button onClick={() => void client.plugin.invoke('myplugin.run', { paths: files })}>
        执行
      </button>
    </main>
  )
}
```

> 官方插件里的 import 是 `../../../../sdk/ui/src/index`（直接编译 SDK 源码）；第三方插件发布时依赖 npm 包 `@fiex/ui-sdk`。

## 3. 在 manifest 里声明面板

面板**内联**在 `toolbar` 或 `contextMenus` 条目上，**没有**独立的 `panels` 注册。条目三者只能选一：`children` / `command` / `panel`。

```json
{
  "contributions": {
    "toolbar": [
      {
        "id": "gpg.keys",
        "title": "公钥管理",
        "icon": "key-round",
        "panel": {
          "entry": "ui/dist/index.html",
          "title": "GPG 公钥管理",
          "defaultSize": { "width": 420, "height": 460 }
        }
      }
    ],
    "contextMenus": [
      {
        "id": "gpg.menu",
        "title": "GPG",
        "when": "context.selection.count > 0",
        "children": [
          {
            "id": "gpg.encrypt",
            "title": "加密…",
            "panel": {
              "entry": "ui/dist/index.html",
              "title": "GPG 加密",
              "defaultSize": { "width": 540, "height": 480 }
            }
          }
        ]
      }
    ]
  }
}
```

约束（Host 与前端都会校验）：

- `id` 在插件内唯一，同时就是 `panelId`；不同插件之间天然命名空间隔离。
- 同一份 HTML 可以挂在多个条目上，用 `context.get().panelId` 区分（见第 5 节）。
- 右键菜单条目：嵌套深度 ≤ 4（与前端解析一致），单插件合计 ≤ 100。
- `entry` 必须是包内相对路径，不能 `..` 跳出；单文件 ≤ 1 MiB。
- `defaultSize` 省略时为 `560 × 560`；初始尺寸会被夹到窗口内且不超过 **720 × 680**（内容区，不含 52px 宿主标题栏）。`panel.resize` 请求的下限是 **320 × 220**。
- 子菜单条目的面板同样可用——`PluginHost` 会递归解析 `children`。

## 4. 打开位置、生命周期与会话

| 来源          | 位置                                                 |
| ------------- | ---------------------------------------------------- |
| `toolbar`     | 顶部对齐，横向锚定点击图标时的鼠标 x（从右向左展开） |
| `contextMenu` | 窗口居中                                             |

- 同一窗口最多保留 **3 个**面板会话，键为 `pluginId + panelId + directory + selection`；切换目录/选择会产生新会话。
- 面板被判定隐藏时收到 `panel.visibility = false`，此后新的副作用请求会被拒绝；`theme.get`、`context.get`、`tasks.update/finish` 仍可用。
- 关闭、禁用、修改授权或插件变更都会销毁会话；重新打开是**新会话**，不要依赖模块级状态。
- 插件升级不会复用旧会话，旧端口失效。

## 5. 上下文：一个 HTML 服务多个入口

```ts
const context = await client.context.get()
// {
//   directoryPath: '/Users/kevin/Documents',
//   selectionPaths: ['/Users/kevin/Documents/Notes.md'],
//   panelId: 'gpg.encrypt',
//   panelTitle: 'GPG 加密',
//   source: 'contextMenu' | 'toolbar'
// }
```

`panelId` 来自 Host 已安装的 manifest，不是 iframe 能伪造的值。推荐模式：

```ts
const action = context.panelId?.split('.').pop() // 'encrypt' | 'keys' | ...
```

GPG 插件就是这么做的：右键四个动作 + 工具栏一个入口共用同一份 `ui/dist/index.html`，面板只渲染 `panelId` 对应的那一个动作，不做「全能 tab 面板」。

`context.changed` 通道已保留，但当前宿主不会主动推送——需要最新值时重新调用 `client.context.get()`。`windowId` / `paneId` 字段同样已保留、暂不下发。

## 6. SDK 能力一览

```ts
const client = new FiexUiClient({ timeoutMs?: number })

await client.ready()                    // 一般不用显式调用，其余方法都会等握手
await client.context.get()
client.context.subscribe((context) => {}) // 通道保留，当前不推送
await client.plugin.invoke<T>('method', params, { signal, timeoutMs })
await client.theme.get()
client.theme.subscribe((snapshot) => {})
await client.dialog.openFile({ directory, extensions })
await client.dialog.openFiles({ ... })
await client.dialog.openDirectory({ ... })
await client.dialog.saveFile({ suggestedName, extensions })
await client.panel.resize(width, height)
await client.panel.close()
client.events.on('some.event', (payload) => {})
```

另有 `tasks.*`（`create` / `spinner` / `update` / `finish` / `cancel` / `list`，把长任务显示在宿主状态中心）和 `vfs.*`（`stat` / `list` / `open`，用于连接型插件的虚拟目录）。

白名单里还有两个没有专门封装的方法，可直接走底层 `invoke`：

```ts
await client.invoke('clipboard.writeText', { text })
await client.invoke('navigation.open', { path: '/Users/kevin/Documents' })
```

`plugin.invoke` 默认 **120 秒**超时；用 `signal` 取消只是停止等待，已经交给后端的操作不会因此中断。

## 7. 继承 Core 的主题（重点）

插件**不引用 Core 的样式表**，也不需要知道 Fiex 的调色板。主题通过三步传进来：

```text
Core 主窗口
  themeSnapshot()  // src/lib/themes.ts：读取 <html> 上的计算值
        │  { mode, tokens: { text, canvas, accent, ... } }
        ▼
  PluginPanelHost  →  面板专属 MessagePort  →  'theme.changed' 事件
        │
        ▼
  SDK applyTheme()  // 把每个 token 写成 iframe <html> 的内联 CSS 变量
        │
        ▼
  面板 CSS：var(--canvas) / var(--text) / var(--radius-control) ...
```

所以：

- **切主题时面板实时变色**，面板代码一行不用改。
- 每次握手完成会立刻收到一次 `theme.changed`；首帧之前 token 还不存在，**CSS 必须写回退值**。
- iframe 里同时会切 `html.dark` 和 `color-scheme`，需要特殊处理时可以写 `html.dark .foo`。

### 7.1 推荐写法

先建一层别名，再在组件里用别名：

```css
/* style.css */
:root {
  --panel-bg: var(--canvas, #fbfcfd);
  --panel-surface: var(--surface, #f8fafc);
  --panel-text: var(--text, #304252);
  --panel-muted: var(--muted, #85929e);
  --panel-line: var(--line, #dee5eb);
  --panel-accent: var(--accent, #387ca8);
  --panel-radius: var(--radius-control, 7px);
  color: var(--panel-text);
  background: var(--panel-bg);
}
```

`official/panel.css` 已经提供了最小的公共层（`--panel-bg: var(--canvas)`、控件的 `--radius-control`、隐藏时暂停动画），插件在自己的 `style.css` 里 `@import "../../../panel.css"` 即可。

**不要**在面板里硬编码 `#f3f3f3`、`rgba(...)` 这类界面色——那是主题不生效的最常见原因。只有语义色之外的业务色（例如“已签名/未签名”徽标）才适合自定义变量，并且要同时给出深浅两套回退。

### 7.2 面板能拿到哪些 token

默认快照包含 `THEME_TOKENS`（`src/lib/themes.ts`）里的全部语义 token，再加上当前主题自己声明的 token。常用：

```text
基础色   text muted surface canvas hover line accent selected danger success warning on-accent
　　　   glass shadow overlay
表面     sidebar-background sidebar-line toolbar-background toolbar-line
　　　   tabbar-background tab-background tab-hover-background tab-active-background tab-active-line
　　　   row-alt-background row-hover-background row-selected-background row-selected-line row-selected-text
　　　   nav-active-background nav-active-line statusbar-background
　　　   menu-background context-menu-background menu-line
　　　   input-background input-line control-hover-background
　　　   segmented-background segmented-active-background
圆角     radius-control radius-panel radius-row radius-tab radius-menu radius-submenu
字体     font-family
```

面板还可以用 `panel-bg`（Host 派生的 `canvas` 别名）。主题若额外声明了 `code-keyword`、`accent-soft` 之类的自定义 token，也会一起送进面板；这是「合作插件」可用的约定，Core 自身不保证每个组件都消费它。

完整 token 含义与取值规则见 [22 主题包开发](22-THEME-AUTHORING.md)。

## 8. 尺寸：让面板贴着内容

Host 初始尺寸来自 `panel.defaultSize`，之后由面板自己请求：

```ts
useEffect(() => {
  const root = document.querySelector('.panel') as HTMLElement | null
  if (!root) return
  const previous = root.style.height
  root.style.height = 'auto'
  const measured = Math.ceil(root.scrollHeight)
  root.style.height = previous
  void client.panel.resize(540, Math.max(300, measured))
}, [state])
```

- 请求值会被夹到 320×220 … 720×680，且不超过当前窗口。
- `panel.resize` 只改容器，不改宿主标题栏、焦点和沙箱边界。
- 内容较多时保持上限，在 iframe 内部滚动，不要试图突破。

## 9. 本地开发循环（dev server）

```bash
# 1) 起 Core（127.0.0.1:1420）
npm run dev

# 2) 构建你要看的面板
cd ../fiex-official-plugins/official/gpg/ui
npm ci && npm run build

# 3) 刷新页面，从右键菜单或工具栏打开面板
```

开发模式的行为：

- Core 的 dev server 直接读 `../fiex-official-plugins/official/<slug>/ui/dist/index.html`，通过 `/__fiex_official_plugin/<slug>` 提供。
- dev 下默认开启 `officialMode`，官方插件会出现在右键菜单/工具栏（工具栏插件工具在 DEV 会自动固定）。
- 面板 HTML 由宿主按会话缓存；**改完 UI 必须重新 build 并关闭重开面板**，热更新只作用于 Core 主界面。
- `?pluginFixture` 是旧的单面板夹具，`?demo` 是隔离 UI 夹具，两者都不代表真实插件。

## 10. 构建与打包

- `ui/dist` 由 CI 构建并打进发布包，**不提交到仓库**。
- 包内是 `manifest.json` + `ui/dist/index.html`（+ 可选的平台二进制）。面板必须是自包含单文件，资源用相对路径。
- 打包脚本 `scripts/package-fiex-plugin.py` 会校验 `entry` 文件名、拒绝越界路径。

## 11. 安全检查清单

- 文件路径、选中项来自 `context.get()`，可以直接用；但**不要拼 shell 字符串**，需要执行外部命令时把参数数组交给插件后端。
- 面板只做展示与编排，真正的文件/网络操作放在插件 Native 进程，并受 manifest `permissions` 约束。
- 绝不在面板里收集口令/私钥；这类输入应交给系统组件（例如 GnuPG 的 pinentry）。
- 不实现“自己画一个文件选择器”去绕过 `dialog.*`；宿主对话框是唯一受信任的路径来源。

## 12. 排错表

| 现象                              | 原因 / 处理                                                                                      |
| --------------------------------- | ------------------------------------------------------------------------------------------------ |
| `plugin declares no such panel`   | `panelId` 没有内联在 `toolbar` / `contextMenus`（含子菜单）条目上；或 `entry` 越界、文件名不合法 |
| 面板一片空白                      | `ui/dist/index.html` 没构建；或构建时没有单文件内联（外链脚本被 CSP 拦）                         |
| 打开面板报「插件未安装 / 未授权」 | 插件被禁用、未完成激活，或授权被撤销                                                             |
| 主题色不生效                      | 面板里写了硬编码颜色；token 名拼错；主题值被校验拒绝（见 22）                                    |
| 面板不跟着内容收缩                | 没有调用 `panel.resize`，或请求值被夹取                                                          |
| 请求被拒绝                        | 面板被隐藏（`panel.visibility = false`）或方法不在白名单                                         |
| 自己 `postMessage` 没有响应       | 宿主只认 SDK 建立的端口；改用 `FiexUiClient`                                                     |
