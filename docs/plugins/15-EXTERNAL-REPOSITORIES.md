# 15 · 外部仓库与子模块版本管理

代码来源：

- [Fiex Core](https://github.com/kevinhuang001/fiex)：宿主、权限、安装器、Native IPC 管理、UI 以及业务内核。
- [Fiex SDK](https://github.com/kevinhuang001/fiex-sdk)：Rust IPC 与 TypeScript UI Bridge 的独立实现、测试与 CI。
- [Fiex Official Plugins](https://github.com/kevinhuang001/fiex-official-plugins)：Git、WebDAV、SMB、Archive、Checksum 的 Native 插件、三平台包和集成测试。

Core **只引用 `sdk/` 子模块**。Core 不引用官方插件仓库，不构建或预装官方插件。官方插件仓库独立存在，并通过自己的 `sdk/` 子模块固定 SDK 版本；使用方更新 SDK 时必须显式更新 SHA 并通过 CI。

本地开发：

```sh
git clone https://github.com/kevinhuang001/fiex.git
cd fiex
git submodule update --init sdk
```

Core CI 仅初始化 `sdk`：测试本地文件管理、权限/安装器、模拟插件进程的初始化、IPC 调用、停用和重启。不部署 WebDAV/SMB 服务、不运行 WebDAV/SMB 协议测试，也不构建或发布官方插件。Core 原有 `remote.rs` 中仍存在待迁移的旧协议实现，此处只迁走测试职责，不将其伪称为已完成运行时代码解耦。

SDK CI 独立运行 Rust 协议单元测试、静态检查与 TypeScript UI SDK 编译测试。

官方插件 CI 初始化其 SDK 子模块，在 Linux、macOS、Windows 上分别执行插件测试、构建、黑盒进程 IPC 测试、打包；还需要在插件仓库测试真实 WebDAV/SMB 服务，再在 Linux 上通过真实的 Fiex Core PluginHost 运行所有产物做集成测试。通过后才允许从该仓库的 `plugins-v*` Tag 发布 .fiexplugin Release。Core Release 不负责这些插件包。

所有插件需要用户从 GitHub Release 显式下载和授权，GitHub 源码仓库、作者信息及官方身份没有默认信任或免审特权。目前 Native 子进程没有系统级强制沙箱，不应把宿主 API 的授权范围误称为进程级操作系统权限隔离。
