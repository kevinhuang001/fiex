# 插件开发手册

本手册带你从可编译的 Rust 进程开始，逐步注册扩展、处理文件、加入界面并制作可安装包。协议以 Core 当前锁定的 SDK 子模块为准；当前基线使用 manifest schema 2 和 native IPC protocol 7.0。升级协议时请同时检查 Core、SDK 和插件仓库。

## 推荐路线

1. [新建并运行插件](/developer/plugin-development/first-plugin)：编译官方 Hello 示例并检查启动握手。
2. [Manifest 与权限](/developer/plugin-development/manifest)：注册扩展、写平台入口并只申请所需权限。
3. [Provider 实现](/developer/plugin-development/providers)：选择连接、Info、Preview、菜单、搜索等 provider。
4. [虚拟文件系统 VFS](/developer/plugin-development/vfs)：实现云端、归档或其他非本地文件空间。
5. [插件界面与 UI SDK](/developer/plugin-development/ui)：实现面板、连接视图和预览文档。
6. [调试、验证与发布](/developer/plugin-development/testing-publishing)：本地运行进程、打包并验证 Core 集成。
7. 查阅独立的 [IPC 协议参考](/developer/ipc) 和 [软件需求/测试追溯](/developer/requirements)。

## 插件运行模型

Native 插件是由 Fiex 管理的独立进程。Core 启动进程并通过标准输入/输出发送带 Content-Length 的 JSON-RPC 请求。Rust SDK 负责握手、帧读写、并发分发、宿主调用和任务生命周期；业务代码通常只实现 provider trait。

插件界面（如果有）作为安装包中的本地页面运行在受控 UI 容器中，通过 UI SDK 的 bridge 调用允许的界面能力。UI 页面不直接通过 bridge 读取文件系统；文件和 VFS 工作应由 native provider 完成。

## 从当前示例开始

官方可编译样例位于 [`fiex-official-plugins/example/hello`](https://github.com/kevinhuang001/fiex-official-plugins/tree/main/example/hello)。它覆盖连接、VFS、Info、Preview、Decorations、Context Menu、Search、Hooks、Toolbar 和 UI。初学时建议先删除不用的示例 provider，再添加你需要的能力，避免复制无关权限。

当前 Core 锁定的 Rust API 以 [SDK providers 源码](https://github.com/kevinhuang001/fiex-sdk/blob/24664a69bb411d0b2608461ae08a588f28cbe385/rust/src/providers.rs)和 [protocol-7 contract](https://github.com/kevinhuang001/fiex-sdk/blob/24664a69bb411d0b2608461ae08a588f28cbe385/docs/protocol-7.md)为准。其他仓库的 `main` 可能已经领先于 Core 锁定版本。

## 何时选哪种扩展

| 目标                         | 入口                                                                               |
| ---------------------------- | ---------------------------------------------------------------------------------- |
| 用户保存连接后浏览服务端文件 | `extensions.connections` + `extensions.vfs` + `ConnectionProvider` + `VfsProvider` |
| 在文件信息面板增加字段       | Manifest 静态声明 `extensions.info.sections` + `InfoProvider::compute`             |
| 预览一种文件格式             | `extensions.preview` + `PreviewProvider`，返回包内预览页面或标准预览视图           |
| 添加文件右键菜单             | `extensions.contextMenus` + `ContextMenuProvider`                                  |
| 添加连接自身的右键菜单       | `extensions.connectionContextMenus` + 对应 provider                                |
| 向搜索合并结果               | `extensions.search` + `SearchProvider`                                             |
| 在打开文件前/后响应          | `extensions.hooks` + `HookProvider`                                                |
| 增加工具栏按钮或面板         | `extensions.toolbar` + `ToolbarProvider`                                           |
| 给文件行/列补充动态信息      | `extensions.decorations` + `DecorationProvider`                                    |

注册扩展只表示该 provider 有资格收到对应事件。调用时仍会受用户授权、资源范围、匹配条件和 Core 的能力校验约束。
