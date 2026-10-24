# 12 · 实现状态（开发分支）

> 对应 `feat/plugin-host-v2` / `refactor/fiex-external-submodules` 开发代码。协议 2.0，未发布。

## 已实现

- **进程宿主**：manifest 发现、用户授权、独立进程、握手、身份校验、启用/停用、卸载。
- **权限**：六项目录/文件能力、`network`、外部命令；HTTP/TCP 共用精确主机白名单，回环地址也需授权。
- **通用虚拟 VFS**：独立 `vfs` Contribution、Connection 隐式 VFS、Core 统一 URI/Owner 路由、按需 Git View 和有界引用回收已经接入；无系统级挂载。详见 [VFS](02-VFS.md)。
- **Git 插件化**：Core 删除 `git.rs`/`git_history.rs`/`git_remote.rs` 业务实现；专用命令只做插件代理。
- **远程流**：WebDAV/SMB 使用 `fs.openRead/readStream/closeRead`，Core 负责暂存文件、块大小、EOF 和取消清理；不再交接绝对路径。
- **官方插件**：Git、WebDAV、SMB、Archive、Checksum、Everything 搜索独立 crate，独立构建与打包。
- **应用商店 UI**：设置 → 插件一行一个插件；权限管理按钮始终可见；官方插件列表从 Release 的 `fiex-catalog.json` 读取；无手填仓库地址。
- **沙箱**：Linux Landlock + seccomp、自定义 macOS Seatbelt、Windows AppContainer + Job Object；原生网络始终禁用，三平台验证由 CI 执行。

## 未完成 / 发布阻断

- Windows AppContainer 真机 CI 与 Everything 运行环境验证。
- 任务级 `taskId` 取消与插件进度上报；目前下载在块之间取消，Archive/Checksum 无进度。
- 内核级远程缓存/同步/冲突仲裁引擎；当前只做按需下载与本地镜像。
- 远程写入、OAuth Broker、凭据轮换。
- SMB/WebDAV 真实服务器跨平台 CI 目前主要在官方插件仓库；Core CI 只用 mock 插件。
- macOS 沙箱与三平台安装包的真机验证。

## 验证

```sh
npm run format:check && npm test && npm run build && npm run e2e
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --lib
# 跨仓库示例（需要先构建并安装官方插件）
cargo run --manifest-path ../fiex-official-plugins/integration/Cargo.toml --example plugin_git_e2e
cargo run --manifest-path ../fiex-official-plugins/integration/Cargo.toml --example plugin_webdav_e2e
cargo run --manifest-path ../fiex-official-plugins/integration/Cargo.toml --example plugin_optional_e2e
```

## 稳定性

协议仍标注 `2.0 experimental`；在 Windows 沙箱和任务/进度协议完成前，不承诺第三方兼容性。
