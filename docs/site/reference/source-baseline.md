# 文档代码基线

本次文档以 **2026-11-08** 默认分支上的实际代码为准，并核对 Core、SDK、官方插件仓库及其 SDK 子模块。Core 基线包含 Core PR #75–#78 的合并结果；本次文档 PR 只加入文档内容与 GitHub Pages 配置。

| 来源                                   | 核对版本                                   | 用途                                                      |
| -------------------------------------- | ------------------------------------------ | --------------------------------------------------------- |
| Core：fiex `main`                      | `0594357eb483fcc3b811955e01f49f3a2cb07159` | 用户行为、Tauri/Core 结构、Cargo workspace、平台实现与 CI |
| Core 的 SDK 子模块                     | `28586d4be27c6e5b23345a010fa7c70a12860820` | Core 实际锁定的 protocol 7.0 contracts                    |
| SDK：fiex-sdk `main`                   | `ab21f3a15ed993e5ad9a3c326060b446bb490347` | Rust SDK、共享 provider contracts、UI SDK 与 protocol 7.0 |
| 官方插件：fiex-official-plugins `main` | `51da0f5f71911ec3c91085cd667add61e727025d` | 官方插件实现、manifest、示例与集成验证                    |
| 官方插件仓库的 SDK 子模块              | `28586d4be27c6e5b23345a010fa7c70a12860820` | 官方插件构建和验证使用的 SDK 快照                         |

Core 与官方插件仓库当前都锁定 SDK commit `28586d4`；SDK 自己的 `main` 是包含该协议改动的合并提交。核对其他版本时，仍应分别查看每个仓库实际检出的 SDK 子模块，不能只看 SDK 的 `main`。

## 文档更新规则

当以下任一项变化时，同一 pull request 应更新对应页面：

- 用户可见的文件操作、搜索、设备或设置行为；
- Cargo workspace crate、Tauri command 边界、provider 路由；
- Manifest schema、权限、协议版本或 SDK API；
- 支持的平台、构建命令和 CI workflow；
- GitHub Pages 路径、workflow 或部署方式。

每次代码版本变更后，此页面中的基线 commit 和核对日期也应随之更新。遇到不一致时，优先核对默认分支代码、子模块 commit 和当前 CI，再更正文档。
