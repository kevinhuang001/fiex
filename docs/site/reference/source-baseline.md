# 文档依据的代码版本

此文档站对照 **2026-11-11** 的 Core `main` 基线及 Office 功能分支快照编写。涉及 Core wire contract、用户功能和测试追溯时，以 Fiex Core 锁定的 SDK submodule 为准；外部仓库的 `main` 或官方插件锁定的旧 SDK 不能覆盖 Core 实际使用的版本。

| 来源                       | 核对快照                                                             | 本站用途                                                    |
| -------------------------- | -------------------------------------------------------------------- | ----------------------------------------------------------- |
| Fiex Core `main`           | `9b1dc33644d534eaed24bb84ab267bf822f15e7f`                           | 用户界面行为、Core 结构、权限、测试和 GitHub Pages workflow |
| Office 功能 PR 快照        | `54b47fc38ed781305a952f8e7043ca1772c0b1a4`                           | 新建文件菜单、DOCX/XLSX/PPTX 模板、PDF 转换及其自动化测试   |
| Core 内 `sdk/` 子模块      | `24664a69bb411d0b2608461ae08a588f28cbe385` (`fiex-plugin-sdk` 0.3.4) | Core 实际使用的协议 7.0、Rust API 和共享 provider contracts |
| SDK 仓库 `main`            | `b89eb341f42552936c52d76b9ccf93568511982a`                           | 补充理解 Rust/UI SDK；仍需与 Core pinned SDK 对照           |
| 官方插件仓库 `main`        | `5c63527b57e31fc02c95629c42383723b6e035f5`                           | 可编译插件示例与业务实现；其 SDK 子模块见下一行             |
| 官方插件仓库 `sdk/` 子模块 | `24664a69bb411d0b2608461ae08a588f28cbe385` (`fiex-plugin-sdk` 0.3.4) | 官方仓库当前构建与示例所锁定的 SDK 快照，与 Core 子模块相同 |

因此，插件作者应先在目标 Core 版本上验证兼容性。特别是 SDK/UI API 的小版本变化、Core 子模块锁定版本和官方插件 CI 配置，不能仅凭仓库首页的 `main` 名称推断相同。

协议版本：manifest schema 2；native plugin IPC protocol 7.0。Core pinned SDK crate 版本：0.3.4。版本变更时请重新检查 [IPC 协议参考](/developer/ipc)、[插件开发手册](/developer/plugin-development)、[需求追溯](/developer/requirements)和 CI workflow。

## 更新基线的规则

发布 Core 新版本或文档变更合并前，维护者应更新本表的完整 commit SHA 和核对日期。对照代码时至少确认：

- Core 主分支 commit 与 SDK 子模块 commit；
- manifest schema、protocolVersion 与 Core 实际校验代码；
- 对应 UI/E2E/Rust/IPC/平台测试路径；
- Docs workflow 构建与部署目标。

若页面描述和代码不同，以对应目标版本源码和已运行 CI 为准，并在同一个功能 PR 中修正文档。
