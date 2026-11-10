# 开发者文档

这部分面向修改 Fiex、编写插件或审阅工程质量的人。先确定改动属于哪个仓库，再选择对应的验证方式。

## 哪个仓库负责什么

| 仓库                                                                            | 负责内容                                                              | 典型改动                                     |
| ------------------------------------------------------------------------------- | --------------------------------------------------------------------- | -------------------------------------------- |
| [fiex](https://github.com/kevinhuang001/fiex)                                   | 桌面应用、Core、Tauri 命令、内置文件位置、插件宿主、权限和 Pages 文档 | 文件操作、窗口交互、系统集成、provider 路由  |
| [fiex-sdk](https://github.com/kevinhuang001/fiex-sdk)                           | Rust 插件 SDK、共享 provider contracts、UI SDK、IPC 协议              | 插件调用接口、跨仓库 DTO、JSON-RPC/Wire 变更 |
| [fiex-official-plugins](https://github.com/kevinhuang001/fiex-official-plugins) | 插件业务逻辑、官方示例、插件构建与发行包                              | Git、WebDAV、SMB、预览、归档等具体插件       |

Core 负责提供通用授权和运行时机制，不应把某个具体插件的远程协议或业务 UI 塞进内置模块。共享数据结构或行为契约需要变更时，先在 SDK 定义，再更新 Core 和所有受影响插件。

## 推荐阅读顺序

1. [系统架构](/developer/architecture)：了解前端、Tauri/Core、provider 与插件进程之间的边界。
2. [软件需求与测试追溯](/developer/requirements)：查看用户可见行为的需求编号、当前验证位置和仍需系统实测的部分。
3. [IPC 协议参考](/developer/ipc)：处理插件消息时查阅现行 wire contract。
4. [本地开发环境](/developer/environment) 与 [格式、构建和 CI](/developer/quality-ci)：准备环境并运行对应检查。

想创建插件，请从[插件开发路线](/developer/plugin-development)进入；插件不能只依赖 Fiex 的 `main` 文档，必须使用目标 Core 实际锁定的 SDK 子模块版本。

## 变更边界

- 代码行为变了，更新对应用户指南和需求追溯。
- SDK/协议改变，更新协议页和 SDK 中对应规范，并在 Core 与插件侧验证同一协议组合。
- 测试通过只能证明测试覆盖的行为；系统文件对话框、回收站、拖放、沙箱和平台权限还需要相应平台检查。
- 文档站构建成功只证明页面和链接可以生成，不代表产品行为已经通过测试。
