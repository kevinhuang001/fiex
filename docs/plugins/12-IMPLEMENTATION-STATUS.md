# 插件架构实现进度（开发分支）

> 当前开发目标：`refactor/fiex-external-submodules` / [Draft PR #5](https://github.com/kevinhuang001/fiex/pull/5)。本页是**开发状态**，不是已发布功能承诺。实验协议尚未冻结。

## 已落地的仓库边界

- [fiex](https://github.com/kevinhuang001/fiex)：Core 文件操作、通用插件安装和权限、原生进程宿主、UI、模拟 IPC 测试；仅引入 `sdk/` 子模块。
- [fiex-sdk](https://github.com/kevinhuang001/fiex-sdk)：Rust JSON-RPC 协议、Native SDK、TypeScript UI SDK，独立 CI。
- [fiex-official-plugins](https://github.com/kevinhuang001/fiex-official-plugins)：Git、WebDAV、SMB、Archive、Checksum 插件实现及 Linux/macOS/Windows 打包；真实 WebDAV/SMB 服务器功能测试和调用真实 Core 宿主的集成测试。

不内置、预安装、特殊信任任何官方插件。安装从 GitHub Release 手动选择 `.fiexplugin`，预览并确认权限，卸载、禁用、权限撤销均由 Core 统一管理。Native 插件仍是**未受操作系统强制沙箱隔离的进程**，宿主侧权限门控不能防止恶意插件直接使用进程的 OS 用户权限。

## 已实现的运行时迁移

- Core `src-tauri/src/plugins/host.rs` 支持动态 manifest、原生进程握手、`plugin.invoke`、存储/凭据反向调用和生命周期管理。
- Core `src-tauri/src/remote.rs` 的 `provider_list`、挂载目录枚举 `list_remote`、连接验证和文件下载已改为经**已安装、已授权**插件进程 RPC 执行，不再在 Core 内启动 WebDAV/SMB 网络客户端；未授权时不得静默回退。
- `desktop.rs` 现在通过 `remote::list_listing` 将插件元数据映射为虚拟文件条目；目录列出时**不生成零字节本地占位文件**。只在显式下载、预览或拷贝时按需物化。
- WebDAV / SMB 插件均支持 `connection.validate`、游标式 `fs.list`、`fs.readSmall`、`fs.download`；各自的真实服务器测试位于**官方插件仓库**。
- `PluginPanelHost.tsx` 使用 sandboxed iframe（opaque origin）、限定 CSP、MessageChannel 桥；对话框权限由 Core 检查。插件配置和工具通过 manifest 加载。
- Core CI 不运行 WebDAV/SMB 服务、不构建官方插件；官方插件仓库 CI 拥有协议/服务端测试，避免职责回流。

## 尚未完成：不能宣称全面解耦

1. **受控二进制流**：目前 `fs.download` 经 Core 指定本地暂存文件路径，仍是过渡式原生路径交接；`fs.readSmall` 的预览体采用限额 Base64。尚未实现 `fs.openRead / beginWrite / commitWrite` 双向流、传输版本条件与可取消任务，也没有限制插件只能写入指定临时文件的 OS 沙箱。
2. **任意第三方连接 Provider**：连接表单由 manifest 提供，但 Core 连接持久化仍基于旧 WebDAV/SMB 两种字段和种类、与 `org.fiex.<kind>` 的旧映射；无法保证任意新连接插件可用，须把 schema、加密密钥名及 provider ID 完全动态化。
3. **Git**：Core 旧 `git.rs`、`git_history.rs`、`git_remote.rs` 及若干 Tauri Git 命令仍在使用；官方 Git 插件虽独立实现，UI 尚未彻底迁往插件 API。
4. **缓存/同步引擎**：Core 的远程连接注册、缓存元数据及下载历史仍在 `remote.rs`，需拆分 CacheManager/SyncEngine，具备断点续传、离线冲突与版本判定。
5. **进程隔离**：缺少按 OS（Windows/macOS/Linux）强制约束 Native 插件的沙箱；未授予的系统文件、网络和外部命令仍可能被插件进程直接访问。
6. **SDK 规范及生命周期**：宿主/SDK 契约、任务进度/可靠取消、OAuth Broker（PKCE/state/callback/refresh）、插件升级迁移、安全安装路径、全链路集成/录屏回归仍需补齐。
7. **依赖清理**：Core Cargo.lock 和部分旧 crate 声明尚未清理；应待 Rust 全平台构建稳定后统一更新 `Cargo.lock` 并以 `--locked` 回归。

## 发布准则

- [ ] SDK、Core（含主程序）及独立插件在 Linux、macOS、Windows 的检查全部成功。
- [ ] 官方插件真实 WebDAV/SMB 功能测试与真实 Core Host 集成通过。
- [ ] 所有 Git/远程协议后端仅存在于插件仓库，Core 不再有专有 Tauri 命令或协议依赖。
- [ ] 安装、授权、禁用、撤销与未安装失败回归均通过，Core 不包含 `plugins/` 子模块或预装包。
- [ ] 真实 VFS 数据传输、缓存与 UI 无回退，支持第三方自定义 Provider。
- [ ] 编写稳定 schema、协议兼容性/安全准则并通过完整回归测试。

在所有发布条件满足前继续保留 Draft PR，不将实验性接口称为 v1.0。
