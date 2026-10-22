# 14 · GitHub 插件安装、权限与运行时界面

> Draft 0.2 — Fiex 开发分支。普通插件、Fiex 作者发布的插件和第三方插件遵循**完全相同的权限流程**，不使用插件包数字签名，也没有内置插件清单或特殊插件 ID。

## 1. 用户安装过程

在「设置 → 插件 → 从 GitHub 安装」输入公开仓库的 `owner/repo`。

1. Core 用带超时的 HTTPS 客户端读取 `GET /repos/{owner}/{repo}/releases/latest`。
2. 找到最新 Release 的第一个 `.fiexplugin` 资产；若缺失则**报错**，不擅自执行任意 `.exe`/`.sh`。建议一个仓库的最新 Release 只包含一个插件包。
3. 限制下载大小（当前 60 MiB）、解压总字节数（256 MiB）和文件数量（2000），拒绝 ZIP Slip、软链接与特殊文件。
4. 只在临时目录解压、验证 `manifest.json` 和受支持的声明权限，**这个阶段绝不启动二进制**。
5. 前端展示 ID、名称、版本、作者、GitHub 来源和请求权限。用户点击“同意权限并安装”，才将临时目录移动到用户级插件目录并启用。
6. 安装失败则不启动；卸载停止插件进程、撤销授权并删除插件程序文件，但**默认保留用户数据与同步目录**。更新时需使用更明确的升级与回滚流程；初版要求卸载后重装。

作者字段示例：

```json
{
  "schemaVersion":1,
  "id":"org.example.archive",
  "name":"Archive",
  "version":"1.0.0",
  "author":{"name":"Example Developer","url":"https://github.com/example"},
  "permissions":{"filesystem.read":true,"filesystem.write":true,"ui.dialog":true},
  "entrypoints":{"linux-x86_64":"bin/plugin"},
  "contributes":{"contextMenus":[],"panels":[]}
}
```

`author` 只是展示元数据，不能作为可信发布者或权限凭据；GitHub HTTPS 下载也不等于对可执行插件源码/行为的审计。当前不使用插件签名机制。

## 2. 权限行为

- 所有插件都必须安装授权，不预置任何同意状态；首次安装批准的是当前 Manifest **已声明的权限**。
- 设置页可启用、停用和逐项撤销权限。撤销后 Core 反向 IPC 的 `host.storage.*` / `host.secret.*` 会立即拒绝新请求，旧进程应停止或重启。
- 新版本声明新增权限不会自动加入历史授权。
- Node/JS 插件 UI 不获得 Tauri 全局 invoke；通过隔离 iframe 的 MessageChannel 请求宿主。
- **当前原生插件进程启动仍未实现操作系统强制沙箱。** 普通 Rust 子进程可能绕过 Fiex API 直接使用系统调用。未在 Windows AppContainer、macOS App Sandbox/XPC 和 Linux seccomp/Landlock 分别实现前，不得宣称可以隔离恶意 Native 插件。

## 3. 官方功能插件

`plugins/official/{git,webdav,smb,archive,checksum}` 是**源码目录**，不属于 Fiex 应用的内置资源。客户端不读取它们或硬编码 ID。作者可以分别使用 CI 编译、测试，并将 `.fiexplugin` 发布为 GitHub Release 资产，用户手动选择安装。

扩展开发者提供预编译平台 Native 二进制与 `manifest.json`。带交互 UI 的插件还可以提交自包含的 `ui/index.html`（单文件内联 CSS/JS），使用 `contributes.panels[].entry` 声明。Fiex 的宿主模态框把它加载到 `sandbox="allow-scripts"` 的 opaque-origin iframe 中。

## 4. 文件菜单和文件选择器

`contributes.contextMenus` 可以声明递归 `children`（建议最多四层）、`command` 或 `openPanel`。插件菜单是数据，宿主统一渲染；不能注入 JSX/HTML 到宿主上下文菜单。

`openPanel` 对应宿主加载的 iframe 模态框。需要用户指定保存位置时，插件通过 `dialog.saveFile` 请求宿主的系统文件选择器，随后仅获得本次选择的目标文件路径。压缩插件的高级压缩 UI 是该机制的第一个参考实例。

## 5. 后续发布阻断

- [ ] 所有平台完整安装/卸载集成测试，含原生可执行位、平台架构、锁定的程序文件及回滚。
- [ ] 对 GitHub Release 资产命名实现多个候选可选择，而不是只取第一个。
- [ ] 真正的 OS 沙箱及权限撤销后现有 Native 句柄/任务的强制失效。
- [ ] VFS 授权资源句柄及二进制流接口取代裸绝对路径传递。
- [ ] 完整插件 iframe 资产策略、CSP 与事件总线跨平台压力测试。
- [ ] 内核缓存/双向同步与 Git/WebDAV/SMB 的遗留路由彻底迁移。
