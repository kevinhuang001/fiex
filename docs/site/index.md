---
title: Fiex 项目文档
description: Fiex 用户指南、开发者手册与架构参考。
---

# Fiex 项目文档

Fiex 是面向 macOS、Windows 与 Linux 的桌面文件管理器。这个站点汇总日常使用说明、当前架构、插件开发入口和安全边界。

<div class="vp-doc">

| 你要做什么               | 从这里开始                                            |
| ------------------------ | ----------------------------------------------------- |
| 下载并启动 Fiex          | [安装与首次启动](/user-guide/start)                   |
| 管理本地文件、搜索和预览 | [用户指南](/user-guide/navigation)                    |
| 查看文件与文件夹的元数据 | [文件信息与按需计算](/user-guide/file-info)           |
| 配置插件或远程连接       | [插件与远程连接](/user-guide/plugins-and-connections) |
| 构建 Fiex 或了解模块职责 | [开发者文档](/developer/overview)                     |
| 开始开发插件             | [插件开发](/developer/plugin-development)             |
| 查阅权限和当前限制       | [参考](/reference/security)                           |

</div>

## 项目仓库

| 仓库                                                                            | 职责                                                      |
| ------------------------------------------------------------------------------- | --------------------------------------------------------- |
| [fiex](https://github.com/kevinhuang001/fiex)                                   | 桌面应用、Core、Tauri 命令、插件宿主与内置文件系统模块    |
| [fiex-sdk](https://github.com/kevinhuang001/fiex-sdk)                           | Rust 插件 SDK、共享 provider contracts、UI SDK 与协议资料 |
| [fiex-official-plugins](https://github.com/kevinhuang001/fiex-official-plugins) | 官方插件实现、可编译示例和插件集成验证                    |

## 文档范围

本网站优先描述三个仓库当前代码中已经实现的行为。设计计划、历史迁移记录和未来方案不作为现状依据。插件协议、权限声明和能力列表以目标 Fiex 版本实际接受的 manifest 与协议握手为准。

版本发行说明见 [Fiex Releases](https://github.com/kevinhuang001/fiex/releases)。源码中的详细架构材料仍保存在 [Core 的 docs 目录](https://github.com/kevinhuang001/fiex/tree/main/docs)。

::: tip 发现文档与代码不一致
请在对应仓库提交 issue，注明页面、Fiex 版本和相关页面截图或复现步骤。涉及行为变化的文档应与对应代码改动在同一个 pull request 中更新。
:::
