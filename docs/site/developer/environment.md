# 本地开发环境

Fiex 同时包含 React/TypeScript 前端和 Tauri/Rust 桌面后端。要验证桌面功能，需要两套工具链和平台原生依赖。

## 工具版本

| 工具           | Core 当前要求                                                              |
| -------------- | -------------------------------------------------------------------------- |
| Node.js        | CI 使用 Node 22；本地建议使用相同主版本                                    |
| npm            | 使用仓库锁文件，通过 `npm ci` 安装                                         |
| Rust           | Core Cargo manifest 要求 Rust 1.92 或兼容更新版本；SDK 子模块单独要求 1.90 |
| Git            | 需要支持子模块，用于获取 Core 锁定的 SDK                                   |
| Tauri 平台依赖 | 按操作系统安装 Tauri v2 prerequisites 中列出的 WebView、编译器和系统库     |

平台依赖以 [Tauri v2 官方前置要求](https://v2.tauri.app/start/prerequisites/)为准。Linux 打包还会用到 WebKitGTK/GTK 等系统库。

## 克隆与安装

```sh
git clone --recurse-submodules https://github.com/kevinhuang001/fiex.git
cd fiex
npm ci
```

若已克隆但 `sdk/` 为空，运行：

```sh
git submodule update --init sdk
```

Core 的 SDK 子模块是构建输入的一部分。不要用未锁定的 SDK `main` 覆盖它来“修复”版本不匹配；更新子模块需要与协议变更一起审阅。

## 启动桌面应用

```sh
npm run icons
npm run desktop:dev
```

`desktop:dev` 会运行 Tauri 应用并连接真实 Rust 后端。开发前端界面时可使用 `npm run dev` 启动 Vite，但独立浏览器下没有完整桌面后端，不能用它判断系统文件对话框、原生拖放、回收站、设备挂载或插件沙箱行为。

## 构建与检查

```sh
npm run build
npm test
npm run e2e

cargo fmt --manifest-path src-tauri/Cargo.toml --all
cargo test --locked --manifest-path src-tauri/Cargo.toml --workspace --no-default-features --features fiex/usbfs --lib
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --workspace --all-targets --no-default-features --features fiex/usbfs -- -D warnings
```

完整 CI 顺序和跨平台集成项见[格式、构建与 CI](/developer/quality-ci)。不要根据单台开发机的结果推断 macOS、Windows、Linux 的系统行为相同。

## 构建插件

Rust 插件在独立进程运行。可从[插件开发手册](/developer/plugin-development)的 Hello 示例开始。编译器目标、entrypoint 名称和包内相对路径必须匹配插件 manifest 的 `runtime.entrypoints`。
