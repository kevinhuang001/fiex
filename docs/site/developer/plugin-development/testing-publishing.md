# 调试、验证与发布

插件正确构建不等于能够在 Fiex 中安装。验证应从进程协议、provider 行为、Core 授权和实际 UI 四层逐步进行。

## 1. Rust 格式与单元检查

在插件 crate 目录执行：

```sh
cargo fmt --manifest-path example/hello/Cargo.toml --check
cargo test --manifest-path example/hello/Cargo.toml
cargo clippy --manifest-path example/hello/Cargo.toml --all-targets -- -D warnings
```

没有单元测试也要明确列出尚未验证的行为。测试远端网络协议时，优先使用本地 mock server，避免测试依赖个人账号或生产数据。

## 2. 启动真实插件进程

使用 SDK `check_plugin.py` 检查 initialize、业务方法调用、response ID 和正常关闭。对于并发请求和任务取消，SDK 仓库与 Core CI 还提供 mock plugin/process smoke 示例。标准输出只能写协议帧，`eprintln!` 日志写入 stderr。

如果插件启动即退出，先检查可执行文件路径、运行库、架构和 stdout 内容。若 Core 报协议不兼容，检查 Core 实际锁定的 SDK 与 manifest 中 `protocolVersion`。

## 3. 在 Fiex 中验证授权

通过 `FIEX_PLUGIN_DIR` 将本地构建放入开发插件目录，再从插件设置手动启用。验证以下状态：

- 用户取消授权时，插件没有获得权限。
- 关闭某一项权限后，对应宿主调用确实被拒绝。
- 撤销后旧权限不会继续被复用；需要权限的下一次动作会失败或要求重新授权。
- 对不同文件、连接和 VFS mount 的并发请求没有共享错误的资源身份。
- 网络重定向、文件写入失败和任务取消都按可预期方式收尾。

用户级 UI 流程可参考 Core 的 `e2e/explorer/plugins-lifecycle.spec.ts`、`plugins-providers.spec.ts`。这些用例使用自动化夹具；实际系统沙箱、凭据库、远端服务和平台文件对话框还需要对应环境验证。

## 4. 验证 UI 和 package

检查每个 manifest `entry` 都在插件包中，大小写与目录名完全一致；页面调用 UI Bridge 前应等待客户端初始化。若预览页面有异步首屏加载，首屏确实就绪后再调用 `preview.ready()`。

在 UI 中验证窄窗口布局、长文件名/服务器错误、主题切换、语言切换和关闭/重开。对于需要面板状态的操作，核对传入 `context` 中的贡献 ID 和当前选择。

## 5. 打包

官方插件仓库提供 [`package-fiex-plugin.py`](https://github.com/kevinhuang001/fiex-official-plugins/blob/main/scripts/package-fiex-plugin.py) 按平台生成 `.fiexplugin`。官方仓库的构建流程会先编译插件并生成 UI 静态资源，再按照 manifest 选择二进制和 package 文件打包。不同插件的 crate 路径与 feature 不同，请以对应 workflow 为准。

发布前逐项检查：

1. 插件 ID、版本号、协议版本和 platform entrypoint 正确。
2. Manifest 仅请求实际需要的最小权限。
3. 包内 UI/主题/辅助文件完整，路径都在包目录内。
4. `cargo test`、格式、Clippy、manifest/i18n 检查通过。
5. 用将发布的 `.fiexplugin` 在目标 Fiex 版本上完成导入、身份检查、授权、启用、主要功能和卸载检查。
6. 使用正式发布流程签名并校验包；包签名说明身份/完整性，不代替用户审核行为。

## 6. 上游变更与兼容

当变更协议、共享 DTO、permissions 或 provider API 时，更新 SDK，之后同步 Core 的 SDK 子模块和所有目标插件。把四部分放在相互匹配的 PR/提交中；分别验证 Core host integration 和官方插件业务 integration。只在某一仓库的单元测试通过，不能证明整套组合可用。

当前已知限制见[当前限制](/reference/limitations)，协议字段见[IPC 参考](/developer/ipc)。
