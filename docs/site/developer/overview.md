# 项目与仓库

Fiex 由三个相关但职责分开的仓库组成。修改某项功能前先确定逻辑归属，避免把插件业务塞进 Core，或复制一份共享协议。

| 仓库                                                                            | 负责内容                                                              | 常用入口                                                                  |
| ------------------------------------------------------------------------------- | --------------------------------------------------------------------- | ------------------------------------------------------------------------- |
| [fiex](https://github.com/kevinhuang001/fiex)                                   | Tauri 桌面应用、Core 机制、内置模块、provider 注册/路由、插件进程宿主 | 前端在 src；Rust 应用在 src-tauri；可复用工作区 crate 在 src-tauri/crates |
| [fiex-sdk](https://github.com/kevinhuang001/fiex-sdk)                           | Rust SDK、provider contracts、插件 IPC 封装、UI Bridge                | rust、ui、contracts 与协议说明                                            |
| [fiex-official-plugins](https://github.com/kevinhuang001/fiex-official-plugins) | Git、网络连接、压缩、预览等插件业务和集成验证                         | official、example、integration                                            |

## 代码放置规则

- Core 负责通用能力：资源引用、授权范围、provider 路由、插件生命周期和平台抽象。
- 具有独立业务目的的连接器和工具属于官方插件仓库。
- SDK 定义 Core 与插件共享的协议 DTO、traits 和调用接口；不要在三个仓库中维护互不兼容的副本。
- Core 的 sdk 子模块锁定一份具体 SDK commit。构建 Core 时使用它所锁定的版本；开发插件时也要使用目标插件仓库锁定的 SDK。

源码结构和责任边界详见[当前架构](/developer/architecture)。
