# 格式、构建与 CI

Fiex 的验证按前端、Rust、桌面平台和文档站分层。阅读[软件需求与测试追溯](/developer/requirements)可从需求找到测试入口；是否通过仍以具体 PR 的 Actions 结果为准。

## 常用本地命令

```sh
# 前端
npm run format:check
npm run lint
npm run check:boundary-types
npm run check:architecture-boundaries
npm run check:size
npm run check:tauri-contracts
npm run check:i18n
npm test
npm run build
npm run e2e

# Rust
cargo fmt --manifest-path src-tauri/Cargo.toml --all --check
cargo test --locked --manifest-path src-tauri/Cargo.toml --workspace --no-default-features --features fiex/usbfs --lib
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --workspace --all-targets --no-default-features --features fiex/ci-driver -- -D warnings
```

Clippy 使用与 CI 相同的 `ci-driver` feature；需要先安装当前桌面平台的原生开发依赖。若只改文档，可运行 `npm ci --prefix docs/site` 和 `npm run docs:build --prefix docs/site`；文档构建不代替代码测试。

## 当前 Core CI 的主要层次

1. **Scope policy 形式化检查**：Kani 对 `fiex-scope-policy` 中的边界授权属性进行证明。
2. **Scope policy 覆盖率**：Rust 代码覆盖率 job 对该 policy crate 执行至少 85% 行覆盖率门槛检查。
3. **Scope policy mutation**：对派生授权决策做 mutation testing，确认测试能发现授权逻辑被反转或移除等变化。
4. **前端检查**：格式、Lint、严格类型边界、feature 导入方向、文件大小、Tauri command contract、i18n、Vitest 和前端构建。
5. **Playwright E2E**：在浏览器 UI harness 中运行用户流程并保留截图/报告 artifact。
6. **macOS / Windows / Linux 桌面 job**：Rust workspace tests、Clippy、桌面 bundle 构建和原生依赖检查。
7. **真实插件 IPC**：编译 SDK mock plugin，以隔离插件目录启动 Core host smoke、并发、搜索和 locale 示例；Windows 另有无控制台标准句柄检查，macOS 有 Trash recovery regression。

具体 job 顺序、触发条件、target 和 artifact 以[当前 workflow](https://github.com/kevinhuang001/fiex/blob/main/.github/workflows/ci.yml)为准。

## 构建复用与缓存

前端检查会生成可复用的生产前端资源与平台图标；macOS、Windows 和 Linux 桌面 job 使用同一份已检查 artifact，避免各自重新生成不同内容。格式、lint、类型边界、i18n、单测和前端构建仍按 workflow 的步骤执行。

Rust CI 按 OS、target、toolchain、依赖锁文件和相关源码识别 native Cargo cache。命中缓存只复用编译产物；Cargo tests、Clippy、平台 bundle 和 native dependency audit 仍运行。缓存命中也不保证零编译，Cargo 仍会按 fingerprint 判断需重建的内容。

文档页面和本地开发命令见[GitHub Pages 发布](/developer/publishing)。只改文档时可运行站点构建；它验证页面和链接能生成，不替代前端或 Rust 行为测试。

## 如何解读验证结果

- E2E fixture 成功，说明浏览器用户流程在测试数据下工作；它不是真实桌面系统回收站测试。
- 编译通过，说明该 target 的代码能构建；不等于插件远端服务器或系统对话框行为正确。
- Kani/覆盖率/mutation 只覆盖 scope-policy 中建模的属性和代码；不能证明 Core 所有功能没有缺陷。
- 单平台通过不能推断其他平台通过。查看矩阵中 macOS、Windows、Linux 各自的 job 状态。
- 跨仓库插件变更要看 Core、SDK 和官方插件各自的匹配 CI；单仓库绿色不足以证明产品组合兼容。

## 文档站 CI

文档改动由单独的 Documentation workflow 执行 VitePress build。PR 上它只构建并上传 Pages artifact；只有默认分支才部署正式站点。见[GitHub Pages 发布](/developer/publishing)。
