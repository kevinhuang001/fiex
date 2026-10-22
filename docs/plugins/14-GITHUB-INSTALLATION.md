# 14 · GitHub 插件安装、权限与运行时界面

> Draft 0.2 — Fiex 开发分支。普通插件、Fiex 作者发布的插件和第三方插件遵循**完全相同的权限流程**，不使用插件包数字签名，也没有内置插件清单或特殊插件 ID。

## 1. 用户安装过程

在「设置 → 插件 → 从 GitHub 安装」输入公开仓库的 `owner/repo`。

1. Core 使用带超时的 HTTPS 客户端读取 `GET /repos/{owner}/{repo}/releases?per_page=20`，从新到旧选择最近一版带有当前平台插件包的 Release。
2. 展示 Release 中全部匹配当前操作系统及 CPU 架构的 `.fiexplugin` 资产，**由用户选择具体插件**；不运行 `.exe`/`.sh`。一个 Release 可以包含 Git、WebDAV、SMB、压缩、校验和五种插件，各自独立安装。
3. 限制下载大小（当前 60 MiB）、解压总字节数（256 MiB）和文件数量（2000），拒绝 ZIP Slip、软链接与特殊文件。
4. 只在插件安装根目录同一文件系统的 `.staging` 临时目录解压、验证 `manifest.json` 和受支持的声明权限，**这个阶段绝不启动二进制**。
5. 前端展示 ID、名称、版本、作者、GitHub 来源和请求权限。用户点击“同意权限并安装”，才将临时目录移动到用户级插件目录并启用。
6. 安装失败则不启动；卸载停止插件进程、撤销授权并删除插件程序文件，但**默认保留用户数据与同步目录**。更新时需使用更明确的升级与回滚流程；初版要求卸载后重装。

作者字段示例：

```json
{
  "schemaVersion": 1,
  "id": "org.example.archive",
  "name": "Archive",
  "version": "1.0.0",
  "author": { "name": "Example Developer", "url": "https://github.com/example" },
  "permissions": { "filesystem.read": true, "filesystem.write": true, "network": true },
  "networkDomains": ["github.com"],
  "entrypoints": { "linux-x86_64": "bin/plugin" },
  "contributes": { "contextMenus": [], "panels": [] }
}
```

`author` 只是展示元数据，不能作为可信发布者或权限凭据；GitHub HTTPS 下载也不等于对可执行插件源码/行为的审计。当前不使用插件签名机制。

## 2. 权限行为

- 所有插件都必须安装授权，不预置任何同意状态；首次安装批准的是当前 Manifest **已声明的权限**。
- 设置页可启用、停用和逐项撤销 `filesystem.read`、`filesystem.write`、`network` 及单个域名。撤销后插件进程会停止并在下次启用时重启。私有 KV 与凭据代理不依赖这些权限。
- 新版本声明新增权限不会自动加入历史授权。
- Node/JS 插件 UI 不获得 Tauri 全局 invoke；通过隔离 iframe 的 MessageChannel 请求宿主。
- Linux 已有 Landlock + seccomp 沙箱，macOS 已生成 Seatbelt 配置；Windows AppContainer 尚未实现。域名白名单由插件进程校验，不等同于 OS 级网络过滤。

## 3. 官方功能插件

`fiex-official-plugins/official/{git,webdav,smb,archive,checksum}` 是**外部仓库中的源码目录**，不属于 Fiex 应用的内置资源。客户端不读取它们或硬编码 ID。作者可以分别使用 CI 编译、测试，并将 `.fiexplugin` 发布为 GitHub Release 资产，用户手动选择安装。

扩展开发者提供预编译平台 Native 二进制与 `manifest.json`。带交互 UI 的插件还可以提交自包含的 `ui/index.html`（单文件内联 CSS/JS），使用 `contributes.panels[].entry` 声明。Fiex 的宿主模态框把它加载到 `sandbox="allow-scripts"` 的 opaque-origin iframe 中。

## 4. 文件菜单和文件选择器

`contributes.contextMenus` 可以声明递归 `children`（建议最多四层）、`command` 或 `openPanel`。插件菜单是数据，宿主统一渲染；不能注入 JSX/HTML 到宿主上下文菜单。

`openPanel` 对应宿主加载的 iframe 模态框。需要用户指定保存位置时，插件通过 `dialog.saveFile` 请求宿主的系统文件选择器，随后仅获得本次选择的目标文件路径。压缩插件的高级压缩 UI 是该机制的第一个参考实例。

## 5. 后续发布阻断

- [ ] 所有平台完整安装/卸载集成测试，含原生可执行位、平台架构、锁定的程序文件及回滚。
- [x] Release 多插件包列表及选择；平台专用包选择；同分区 staging。
- [ ] 真正的 OS 沙箱及权限撤销后现有 Native 句柄/任务的强制失效。
- [ ] VFS 授权资源句柄及二进制流接口取代裸绝对路径传递。
- [ ] 完整插件 iframe 资产策略、CSP 与事件总线跨平台压力测试。
- [ ] 内核缓存/双向同步与 Git/WebDAV/SMB 的遗留路由彻底迁移。

## 6. 可选插件的三平台构建与发布

`fiex-official-plugins` 仓库维护 `scripts/package-fiex-plugin.py` 和 `.github/workflows/ci.yml`，构建和打包与 Fiex 主程序安装包分离：

- Rust 构建 `fiex-official-plugins/official/{git,webdav,smb,archive,checksum}`；`cargo test` 先于 `cargo build --release`。
- Windows、macOS、Linux 各自输出类似 `archive-macos-aarch64.fiexplugin`、`git-windows-x86_64.fiexplugin`，ZIP 顶层只包含 Manifest、本平台二进制和可选预编译 `ui/`。
- `workflow_dispatch` 可以生成供 CI 检查的工件；仅在推送 `plugins-v*` 标签且三个系统包构建成功后才创建 GitHub Release。
- 用户自己在 Fiex 设置页输入 `kevinhuang001/fiex-official-plugins`，从匹配当前平台的可安装资产中选具体插件并授权。**插件从未自动预装，也没有签名特权。**

CI 已分别验证 Windows x86-64、macOS ARM64 和 Linux x86-64 插件包的构建及工件上传；实际安装/卸载运行测试与操作系统原生沙箱仍属发布阻断项。主程序的 Rust 依赖锁文件由 Fiex Core 维护，桌面构建必须使用 `--locked` 验证。
