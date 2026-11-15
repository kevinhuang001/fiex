# Fiex 代码库重构完成记录

> 更新日期：2026-11-16。此记录总结任务书中的代码迁移和本地复核；开放 PR 仍按堆叠顺序合并。未取得的 CI、原生平台和性能证据单独列出，不以本地结果代替。

## 阶段交付

| 阶段 | 交付状态 | 记录 |
| --- | --- | --- |
| P0 基线与保护网 | 已建立 | 源码与测试盘点、前端/Rust 覆盖率趋势基线、用户操作需求追溯和故障路径回归分批加入；覆盖率目前作为趋势报告，不虚设全局门槛。 |
| P1 架构与 IPC 合同 | 已完成代码迁移 | feature/import 与 Rust crate DAG 检查已接入；typed command/validator 漂移受门禁检查。PR #262 将共享 IPC DTO 集中到 `src/shared/ipc/contracts`，清除历史类型循环并删除旧 installer 类型路径。 |
| P2 UI feature 归属 | 已完成主要迁移 | Explorer、File Picker、Preview、Info、Context Menu、File Operations、Connections、设置与 app/layout 组合按职责拆分；旧入口与转发 shim 随迁移删除，行为与需求追溯同步更新。 |
| P3 Rust FS、Operations、Plugin Host 与 Remote | 已完成分批迁移 | ResourceBroker、目录缓存、preflight、transfer、事务恢复、插件 RPC/生命周期和远端 reconcile 按服务拆分；取消、断连、失败回滚与重启恢复已有定向测试。 |
| P4-01 大文件与旧入口清理 | 已完成 | `scripts/file-size-baseline.json` 为零项；架构 type-cycle baseline 为零；不保留已删除内部入口的兼容转发。 |
| P4-02 最终代码复核 | 本地代码检查完成 | 当前测试、构建、静态门禁和 Linux Rust 检查结果见下方；跨平台性能样本和完整 CI 结果尚未作为本地验收证据。 |

## 本地验证记录

- 前端：Vitest 110 个测试文件、548 项通过；生产构建、VitePress、格式、边界类型、架构边界、文件尺寸、Tauri contracts、i18n 通过；ESLint 0 errors、25 warnings。
- 架构扫描：145 个 feature 文件、430 个模块，0 个运行时循环、0 个类型循环。
- Rust 1.100.0：workspace `cargo fmt --all --check` 与 workspace library tests 通过。
- Linux headless `fiex/usbfs` Clippy 使用 `-D warnings -A dead_code -A unused_imports` 通过。被 headless feature 排除的桌面代码在该 profile 中会被判为未使用；桌面 `ci-driver` Clippy 需要 GDK/GTK 原生开发包，本地环境未提供 `gdk-3.0.pc`，因此该 profile 未能本地构建。
- Playwright 尝试启动 66 个 E2E 用例；Chromium 可执行文件缺失，浏览器安装包多次下载不完整，未进入浏览器断言。

## 合并与发布检查表

- [x] 功能迁移配套更新 architecture audit、source baseline 和受影响的需求/测试追溯。
- [x] 前端与 Rust 本地格式、测试、类型和静态门禁完成复核。
- [x] 不缩减既有 Scope Policy、Coverage、Mutation、三平台桌面、插件 IPC 和文档构建工作流。
- [ ] 合并前按 GitHub PR 堆叠顺序运行完整 Linux、macOS、Windows、插件 IPC 与文档 CI；本轮按任务要求不等待 CI。
- [ ] 收集 10,000 项目录场景的 Linux、macOS、Windows p95 artifacts；三平台数据齐全后再校准阈值，不提前宣称性能无回退。
- [ ] 在可启动浏览器环境中完成全部 Playwright 用户流程断言；本地 Chromium 环境限制不能视为 E2E 通过。
- [ ] 发布前确认目标 Core commit 锁定的 SDK submodule、manifest schema 和 IPC protocol 与官方插件发布矩阵一致。

## 相关记录

- [代码库重构基线](/developer/architecture/codebase-audit-2026-11-12)
- [架构决策 ADR-0001：Feature 与 IPC 边界](/developer/architecture/decisions/0001-feature-and-ipc-boundaries)
- [软件需求与测试追溯](/developer/requirements)
- [文档代码基线](/reference/source-baseline)
- [格式、构建与 CI](/developer/quality-ci)
