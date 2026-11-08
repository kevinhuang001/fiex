# 本地开发环境

## 必需工具

- Node.js 22 或更新的兼容版本，以及 npm；
- Rust stable，最低版本见 Core Cargo manifest（当前要求 Rust 1.90）；
- Git，并能初始化 SDK 子模块；
- 当前平台的 Tauri 系统依赖。具体依赖见 [Tauri v2 prerequisites](https://v2.tauri.app/start/prerequisites/)。

## 获取代码并启动

```sh
git clone --recurse-submodules https://github.com/kevinhuang001/fiex.git
cd fiex
npm ci
npm run icons
npm run desktop:dev
```

若已经克隆但 SDK 目录为空：

```sh
git submodule update --init sdk
```

## 前端开发

```sh
npm run dev
npm run build
npm test
npm run e2e
```

纯浏览器开发服务器可用来调试 UI；桌面应用通过 Tauri command 调用真实 Rust/Core 后端。不要把浏览器 demo 环境当作真实文件系统集成验证。

## Rust 开发

```sh
cargo fmt --manifest-path src-tauri/Cargo.toml --all
cargo test --locked --manifest-path src-tauri/Cargo.toml --workspace --no-default-features --features fiex/usbfs --lib
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --workspace --all-targets --no-default-features --features fiex/usbfs -- -D warnings
```

Linux 桌面构建需要 Tauri 对应的 GTK/WebKitGTK 系统包；Windows 和 macOS 也需要安装 Tauri 官方列出的工具链与 SDK。
