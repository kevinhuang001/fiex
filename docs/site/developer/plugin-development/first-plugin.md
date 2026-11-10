# 新建并运行插件

最省事的入门方式是编译官方仓库中已经可工作的 Hello 插件，再逐项替换示例能力。

## 1. 获取带 SDK 子模块的示例

```sh
git clone --recurse-submodules https://github.com/kevinhuang001/fiex-official-plugins.git
cd fiex-official-plugins
cargo build --manifest-path example/hello/Cargo.toml
```

Windows 生成 `.exe`；macOS/Linux 生成无扩展名可执行文件。示例使用 Rust 1.90 及 SDK 子模块声明的版本。Core 自身的最低 Rust 版本可能更高，见[本地开发环境](/developer/environment)。

## 2. 检查进程握手和一个方法

示例的自定义方法是 `example.greet`。运行 SDK 提供的进程检查脚本：

```sh
python sdk/rust/examples/check_plugin.py \
  --binary example/hello/target/debug/fiex-plugin-example \
  --plugin-id org.example.hello \
  --method example.greet
```

Windows 可把 `--binary` 写成 `example/hello/target/debug/fiex-plugin-example.exe`。检查器会启动真实插件进程，发送 `plugin.initialize`，验证 ID，再调用 `example.greet`，最后发送关闭请求并检查进程退出。

如果初始化失败：

- 检查 manifest 的插件 `id` 是否与 Rust 的 `serve_with` ID 完全相同。
- 检查 `runtime.protocolVersion` 与 SDK 的 `PROTOCOL_VERSION` 是否匹配。
- 确认 `stdout` 只输出协议帧；日志应写入 `stderr`。
- 检查构建产物是否存在且可执行。

## 3. 理解启动结构

示例末尾将 provider 注册到 SDK：

```rust
let providers = sdk::Providers::new()
    .connection(HelloConnection)
    .filesystem(HelloFiles)
    .info(HelloInfo)
    .preview(HelloPreview)
    .serve_with("org.example.hello", custom_method);
```

`Providers` 为标准事件提供类型化分发；最后一个闭包只处理自定义方法。未注册的标准 provider 不会自动生效。插件应该保持进程运行，等待 Core 请求；它不是每次命令启动一次就退出的 CLI 程序。

## 4. 在 Fiex 中手动加载

开发时可设置 `FIEX_PLUGIN_DIR` 指向包含插件包目录的父目录。每个插件目录至少需要 `manifest.json` 和当前平台可执行文件；若 manifest 声明 UI 页面，还要放入相应页面文件。启动 Fiex 后，在插件设置中检查并手动启用，然后审查授权。

手动复制的插件默认保持停用。开发时确认插件身份及来源，再授权运行。不要把用于本地开发的宽权限 manifest 直接用于公开发行。

## 5. 下一步

- 先读 [Manifest 与权限](/developer/plugin-development/manifest)，删掉示例不需要的权限和扩展。
- 选择需要的 [Provider](/developer/plugin-development/providers)。
- 若插件提供文件树，继续看 [VFS](/developer/plugin-development/vfs)。
- 若要添加面板或预览页面，读[插件界面与 UI SDK](/developer/plugin-development/ui)。
