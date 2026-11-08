# 格式、构建与 CI

## 本地检查

Core 仓库常用前端命令：

```sh
npm run format:check
npm run check:i18n
npm test
npm run build
npm run e2e
```

Rust workspace 的格式、单元测试和 Clippy 参数应跟随当前 CI workflow。当前主要命令如下：

```sh
cargo fmt --manifest-path src-tauri/Cargo.toml --all --check
cargo test --locked --manifest-path src-tauri/Cargo.toml --workspace --no-default-features --features fiex/usbfs --lib
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --workspace --all-targets --no-default-features --features fiex/usbfs -- -D warnings
```

如需桌面应用实机验证，还需安装平台依赖并运行 Tauri 桌面构建。纯浏览器 UI 测试不能代替原生文件对话框、真实拖放、系统回收站、磁盘挂载或进程沙箱检查。

## GitHub Actions

Core CI 覆盖格式检查、i18n 校验、前端单元测试、前端构建、Rust 格式、Rust workspace 测试和 Clippy，以及浏览器和多平台桌面构建/集成步骤。真实原生集成视频及日志由对应 Actions 任务保存为构建产物。

官方插件仓库另有插件构建、打包和 Core-host 集成流程。跨仓库改动应在 PR 中说明 Core、SDK 和插件的匹配版本；不能因为一个仓库的测试通过，就推断其余仓库的组合也已验证。

CI job 和命令会变化。出现冲突时以当前默认分支的 workflow 文件为准：

- [Core CI](https://github.com/kevinhuang001/fiex/blob/main/.github/workflows/ci.yml)
- [SDK CI](https://github.com/kevinhuang001/fiex-sdk/blob/main/.github/workflows/ci.yml)
- [官方插件 CI](https://github.com/kevinhuang001/fiex-official-plugins/tree/main/.github/workflows)
