# 15 · 外部仓库与独立 CI

本插件系统的代码归属与仓库边界（Fiex 开发分支）：

- **[Fiex Core](https://github.com/kevinhuang001/fiex)**：本地资源管理、宿主进程生命周期与 IPC、安装/权限、VFS 与缓存/同步、React 设置与 UI 扩展。
- **[Flex SDK](https://github.com/kevinhuang001/flex-sdk/tree/feat/fiex-repo-split)**：`rust/` 提供 Rust JSON-RPC 进程 SDK；根目录 `src/` 提供预编译 iframe 的 TypeScript Bridge SDK。维护协议、Mock 与 SDK 测试。
- **[Flex Official Plugins](https://github.com/kevinhuang001/flex-official-plugins/tree/feat/fiex-repo-split)**：`plugins/official/{git,webdav,smb,archive,checksum}` 的真实插件实现、黑盒进程测试、Core-host 集成测试、三平台打包与发布。

Core 的 `sdk/` 是指向 **flex-sdk 特定 Git 提交**的 submodule（Gitlink），Core Git 对象中不存放 SDK 源码。Core **不再存在** `plugins/` Gitlink、插件实现源目录、插件编译或插件包 Release。插件独立仓库的 Rust Cargo.toml 使用 `git + rev` 固定 SDK 版本，而不是引用 Core 本地源码。

```sh
git clone https://github.com/kevinhuang001/fiex.git
cd fiex
git submodule update --init sdk
```

## 验证职责与触发

1. **Core CI**：普通文件操作及索引、设置、权限、安装器测试；使用 SDK 提供的模拟原生进程验证首次拒绝、用户批准、握手、IPC、禁用和重启；独立构建 Windows/macOS/Linux 客户端。不启动 SMB/WebDAV 测试服务器，也不构建官方插件。
2. **SDK CI**：Rust IPC 编解码/HostClient 单元测试、Rustfmt/Clippy，以及 TypeScript Bridge 类型和 Node 测试。SDK 的浏览器代码不要求最终用户安装 Node。
3. **Plugin CI**：每个平台单独构建和执行五个 Native 进程（真实 stdio Content-Length/JSON-RPC 握手、ping、错误响应、关闭），执行对应 crate 测试并独立打出 `.fiexplugin`。Linux 另检出 Core 并由其真实 PluginHost 启动已构建的校验和进程，验证用户授权与进程通信。各平台插件更深层的网络读写、异常处理和缓存一致性仍需扩展集成用例。

生产发行使用固定 SDK 提交，升级必须通过三仓库各自 CI；不要把 SDK 或插件 CI 合并到 Core 里。插件仍由用户在设置中从 `kevinhuang001/flex-official-plugins` 的 GitHub Releases 自行选择、安装和授予权限。

官方身份无特殊权限，不使用插件签名，也不预装任何官方插件。当前 Native 子进程**没有强制操作系统沙箱**，因此宿主授权只能约束经过宿主的 API，不能宣称它能完全限制不可信 Native 代码的系统调用。
