# 文档代码基线

本次文档站点以 **2026-11-08** 检出的默认分支代码为依据，并直接核对源代码、Cargo manifests、SDK 子模块和官方插件仓库。文档不是从旧的设计计划自动生成。

| 来源                                 | 本次核对版本                             | 用途                                                              |
| ------------------------------------ | ---------------------------------------- | ----------------------------------------------------------------- |
| Core：fiex main                      | 1437e97e99fef624f3253a7a897340011f0f0e1a | 用户行为、Tauri/Core 结构、Cargo workspace、平台安全实现、CI 命令 |
| Core 的 SDK 子模块                   | 8de01a3e5ea2a4b8de027ac6769e37d273b04118 | Core 实际锁定的 SDK/contracts                                     |
| SDK：fiex-sdk main                   | 8de01a3e5ea2a4b8de027ac6769e37d273b04118 | Rust SDK、protocol 6.0、示例和接口说明                            |
| 官方插件：fiex-official-plugins main | d014511a2ca40c2352e4af6cf6cf9ee1fdae1901 | 当前插件清单、示例、UI 和集成组织方式                             |
| 官方插件仓库的 SDK 子模块            | fa48cc35d556944fba163b6d8b8a7fdfd694914e | 官方插件仓库检出的 SDK 快照                                       |

Core 与官方插件仓库分别锁定自己的 SDK 子模块提交。开发和验证某仓库时应使用该仓库实际检出的 SDK，不要假设所有 main 分支的子模块 commit 完全相同。协议兼容性应通过 SDK 握手与对应仓库 CI/集成任务验证。

## 文档更新规则

当以下任一项变化时，同一 pull request 应更新对应页面：

- 用户可见的文件操作、搜索、设备或设置行为；
- Cargo workspace crate、Tauri command 边界、provider 路由；
- Manifest schema、权限、协议版本或 SDK API；
- 支持的平台、构建命令和 CI workflow；
- GitHub Pages 路径、workflow 或部署方式。

每次代码版本变更后，此页面中的基线 commit 和核对日期也应随之更新。遇到不一致时，优先核对默认分支代码、子模块 commit 和当前 CI，再更正文档。
