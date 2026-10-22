# 15 · 外部仓库与子模块版本管理

代码来源：

- [Fiex Core](https://github.com/kevinhuang001/fiex)：宿主、权限、安装器、Native IPC 管理、UI 以及业务内核。
- [Fiex SDK](https://github.com/kevinhuang001/fiex-sdk)：Rust IPC 与 TypeScript UI Bridge 的独立实现、测试与 CI。
- [Fiex Official Plugins](https://github.com/kevinhuang001/fiex-official-plugins)：Git、WebDAV、SMB、Archive、Checksum 的 Native 插件、三平台包和集成测试。

Core 中的 `sdk/`、`plugins/` 为 Git submodule 引用（GitHub 显示 `目录 @ SHA`），不是拷贝的代码、可写的源码所有权或应用资源。官方插件仓库中的 `sdk/` 同样指向明确的 SDK Commit。SDK 更新必须分别在两个使用方中显式更新 SHA，并经过各自的 CI。

本地开发：

```sh
git clone https://github.com/kevinhuang001/fiex.git
cd fiex
git submodule update --init sdk plugins
```

Core CI 仅初始化 `sdk`：测试通用文件管理、权限/安装器、模拟插件进程的初始化、IPC 调用、停用和重启；不构建官方插件，不发布插件包。

SDK CI 独立运行 Rust 协议单元测试、静态检查与 TypeScript UI SDK 编译测试。

官方插件 CI 初始化其 SDK 子模块，在 Linux、macOS、Windows 上分别执行插件测试、构建、黑盒进程 IPC 测试、打包；再在 Linux 上通过真实的 Fiex Core PluginHost 运行所有产物做集成测试。通过后才允许从该仓库的 `plugins-v*` Tag 发布 .fiexplugin Release。Core Release 不负责这些插件包。

所有插件需要用户从 GitHub Release 显式下载和授权，GitHub 源码仓库、作者信息及官方身份没有默认信任或免审特权。目前 Native 子进程没有系统级强制沙箱，不应把宿主 API 的授权范围误称为进程级操作系统权限隔离。
