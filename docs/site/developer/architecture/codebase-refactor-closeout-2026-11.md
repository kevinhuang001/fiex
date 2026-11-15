# Fiex 代码库重构完成记录

> 更新日期：2026-11-16。此记录总结任务书中的代码迁移和本地复核；开放 PR 仍按堆叠顺序合并。未取得的 CI、原生平台和性能证据单独列出，不以本地结果代替。

## 阶段交付

| 阶段                                          | 交付状态         | 记录                                                                                                                                                                                             |
| --------------------------------------------- | ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| P0 基线与保护网                               | 已建立           | 源码与测试盘点、前端/Rust 覆盖率趋势基线、用户操作需求追溯和故障路径回归分批加入；近期补上 Recent 正常导航、移除、失效映射拒绝及键盘撤销的用户流程回归。覆盖率目前作为趋势报告，不虚设全局门槛。 |
| P1 架构与 IPC 合同                            | 已完成代码迁移   | feature/import 与 Rust crate DAG 检查已接入；typed command/validator 漂移受门禁检查。PR #262 将共享 IPC DTO 集中到 `src/shared/ipc/contracts`，清除历史类型循环并删除旧 installer 类型路径。     |
| P2 UI feature 归属                            | 已完成主要迁移   | Explorer、File Picker、Preview、Info、Context Menu、File Operations、Connections、设置与 app/layout 组合按职责拆分；旧入口与转发 shim 随迁移删除，行为与需求追溯同步更新。                       |
| P3 Rust FS、Operations、Plugin Host 与 Remote | 已完成分批迁移   | ResourceBroker、目录缓存、preflight、transfer、事务恢复、插件 RPC/生命周期和远端 reconcile 按服务拆分；取消、断连、失败回滚与重启恢复已有定向测试。                                              |
| P4-01 大文件与旧入口清理                      | 已完成           | `scripts/file-size-baseline.json` 为零项；架构 type-cycle baseline 为零；不保留已删除内部入口的兼容转发。                                                                                        |
| P4-02 最终代码复核                            | 本地代码检查完成 | 当前测试、构建、静态门禁和 Linux Rust 检查结果见下方；跨平台性能样本和完整 CI 结果尚未作为本地验收证据。                                                                                         |

## 本地验证记录

- 前端：Vitest 110 个测试文件、548 项通过；生产构建、VitePress、全仓格式、边界类型、架构边界、文件尺寸、Tauri contracts、i18n 通过；ESLint 0 errors、25 warnings。
- 架构扫描：145 个 feature 文件、430 个模块，0 个运行时循环、0 个类型循环。
- Rust 1.100.0：Core `cargo fmt --all -- --check` 通过；Linux/no-desktop profile 的 workspace library tests 为 496/496 通过（`--workspace --lib --no-default-features --features fiex/usbfs`）。不依赖桌面 crate 的 library targets 严格 Clippy 通过（`--workspace --exclude fiex --exclude fiex-platform --exclude fiex-trash --lib -- -D warnings`）；SDK workspace fmt、64 项测试、1 项 doctest 和 `--all-targets -- -D warnings` Clippy 也通过。`-D warnings` 下对整个 feature-disabled Core 根 crate 运行 Clippy 会把桌面 profile 不编译的代码报为 dead code；默认桌面 workspace 测试当前在 `gdk-sys` 停止，因为环境没有 `gdk-3.0.pc`（GTK/WebKit 开发包也不可用）。
- Playwright `--list` 收集到 69 个用例。Recent 正常导航、移除、失效映射拒绝与键盘撤销流程已加入 E2E；本地尝试用 `/usr/bin/chromium` 执行 Recent 用例，但 Chromium 在启动阶段遇到 `socket() failed: Operation not permitted`，尚未进入浏览器断言。
- SDK/API 配套：Core submodule、SDK PR #37 与官方插件 PR #37 均锁定 SDK `c304aff91dc9a8a3564b7fa67c1aac89ed363f41`；插件协议为 8.0。统一文件流/TCP API 没有 `OwnedFileReader`、`OwnedFileWriter` 或业务侧 `HostProxy` 调用；SDK 测试覆盖 Reader 离开 `Files` 局部作用域后仍满足 `Read + Seek + Send + 'static`，以及流式读写、Seek、共享窗口复用、IPC fallback、事务提交和 TCP 标准流接口。

## 合并与发布检查表

- [x] 功能迁移配套更新 architecture audit、source baseline 和受影响的需求/测试追溯。
- [x] 前端完整单测与静态门禁，以及不依赖 GTK 的 Rust 格式、library tests 和 Clippy 完成复核。
- [ ] GTK 桌面 profile 的本地 workspace test/Clippy；当前环境缺少 `gdk-3.0.pc` 和 GTK/WebKit 开发包。
- [x] 不缩减既有 Scope Policy、Coverage、Mutation、三平台桌面、插件 IPC 和文档构建工作流。
- [ ] 合并前按 GitHub PR 堆叠顺序运行完整 Linux、macOS、Windows、插件 IPC 与文档 CI；本轮按任务要求不等待 CI。
- [ ] 收集 10,000 项目录场景的 Linux、macOS、Windows p95 artifacts；三平台数据齐全后再校准阈值，不提前宣称性能无回退。
- [ ] 在可启动浏览器环境中完成全部 Playwright 用户流程断言；当前新增 Recent/Undo E2E 只能确认已收集，不能视为浏览器断言通过。
- [x] 确认目标 Core commit、SDK submodule、官方插件 SDK pin 与 IPC protocol 一致（SDK `c304aff91dc9a8a3564b7fa67c1aac89ed363f41`，protocol 8.0）。

## 相关记录

- [代码库重构基线](/developer/architecture/codebase-audit-2026-11-12)
- [架构决策 ADR-0001：Feature 与 IPC 边界](/developer/architecture/decisions/0001-feature-and-ipc-boundaries)
- [软件需求与测试追溯](/developer/requirements)
- [文档代码基线](/reference/source-baseline)
- [格式、构建与 CI](/developer/quality-ci)
