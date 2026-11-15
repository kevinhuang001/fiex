# ADR-0001：Feature 与 IPC 边界

- 状态：Accepted
- 日期：2026-11-16
- 范围：Fiex Core 前端与 Rust 主仓库；SDK 和官方插件继续由各自仓库维护

## 背景

前端按组件、hooks、lib 横向分散后，同一业务能力需要跨多个目录维护；IPC DTO 又被 UI 组件和运行时模块共同声明，导致依赖反向和类型循环。Rust 主仓库已经有 foundation、built-ins 与 platform workspace crates，问题主要在主 crate 的职责编排和边界检查，不需要另建一套平行框架。

## 决策

1. UI 功能按 `src/features/<feature>` 组织；`src/app` 负责启动、布局和组合，`src/shared` 只放稳定的跨功能能力。
2. `src/shared/ipc/contracts` 是 UI 与 Core command DTO 的唯一前端类型来源。API、command args/results 和 runtime validators 依赖这些合同；组件不再为同一 IPC payload 声明第二份 DTO。
3. Tauri、插件进程、错误中心和系统 API 由 app/layout 或 service adapter 注入 feature。feature 不反向依赖应用壳，也不直接取得插件可访问的原始本地路径。
4. ResourceRef、Operation Scope、ProviderRegistry 和 Core resolver 保持现有权限事实来源；缓存仅用于展示，不构成资源事实或授权依据。
5. Rust workspace 保留现有 foundation、built-ins、platform 分层。桌面入口负责 DTO/context/error 转换，业务规则留在 core/application 服务；只有依赖可独立且稳定的适配器才抽 crate。
6. 不保留已废弃的内部路径 alias、双路解析或永久兼容 DTO。协议升级必须按 Core 锁定的 SDK commit 与官方插件测试成组验证。

## 验证与执行

- 前端使用 `check:architecture-boundaries` 检查运行时与类型级循环，并用 `tsconfig.boundary.json` 对 IPC、Explorer 和业务模型启用严格可选字段及索引检查。
- `check:tauri-contracts` 对照 Rust command 注册、TypeScript args/results 和 runtime decoder。
- Rust 使用 workspace crate DAG 检查；FS/transfer 代码禁止反向依赖 operations 门面。
- 文件尺寸门禁保持 300 行上限和零例外；职责迁移必须带相邻单测、集成测试或需求追溯。
- 迁移进度、源码快照、测试限制与本地复核记录见[代码库重构完成记录](/developer/architecture/codebase-refactor-closeout-2026-11)。

## 后果

该决策要求迁移以小 PR 堆叠；每个 PR 都应能单独解释职责变化和本地验证。短期会有 app adapter 显式转换 props 的样板代码，但减少了 feature 对应用壳及运行时实现的隐式依赖。未来若新增跨层类型，必须先确定唯一所有者，再增加共享合同或明确的 adapter，不应从便利性出发扩展万能 `lib`。
