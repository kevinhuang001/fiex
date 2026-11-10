---
title: Fiex 文档
description: 面向 Fiex 用户、插件作者和 Core 开发者的完整手册。
---

# Fiex 文档

Fiex 是一款用于整理本地文件和连接插件文件空间的桌面文件管理器。本网站按你要完成的事情组织：先看用户指南；要扩展 Fiex，请看插件开发手册；要修改应用或排查协议问题，请看开发者文档。

## 选择你的入口

| 你现在要做什么               | 从这里开始                                                |
| ---------------------------- | --------------------------------------------------------- |
| 安装 Fiex，开始浏览文件      | [安装与首次启动](/user-guide/start)                       |
| 学会浏览、选择和整理文件     | [认识主窗口与浏览文件](/user-guide/navigation)            |
| 安装插件或连接云端/远程服务  | [安装插件与连接服务](/user-guide/plugins-and-connections) |
| 使用 Git 插件管理仓库        | [使用源代码管理](/user-guide/source-control)              |
| 使用本地终端或 SSH 插件      | [本地终端与 SSH 连接](/user-guide/terminal-and-ssh)       |
| 自己编写一个 Fiex 插件       | [插件开发路线](/developer/plugin-development)             |
| 了解插件与 Core 如何通信     | [IPC 协议参考](/developer/ipc)                            |
| 了解产品需求对应哪些测试     | [软件需求与测试追溯](/developer/requirements)             |
| 修改桌面应用、运行检查或构建 | [开发者文档](/developer/overview)                         |

## 文档地图

### 用户指南

用日常语言说明界面、操作步骤、插件授权和常见问题。每一页都围绕用户实际会遇到的任务编写，并配有当前版本界面截图。

### 插件开发手册

从创建 Rust 插件开始，逐步说明 manifest、权限、provider、虚拟文件系统、插件界面、调试和打包。协议字段与源码链接以仓库当前版本为准。

### 开发者参考

说明 Core 的模块边界、IPC 消息格式、CI 和需求追溯。这里区分了自动化单元测试、跨进程集成检查和需要真实桌面/系统环境验证的行为。

## 项目仓库

| 仓库                                                                            | 包含的内容                                           |
| ------------------------------------------------------------------------------- | ---------------------------------------------------- |
| [fiex](https://github.com/kevinhuang001/fiex)                                   | 桌面应用、Core、内置位置、插件宿主和此文档站         |
| [fiex-sdk](https://github.com/kevinhuang001/fiex-sdk)                           | 插件 Rust SDK、共享 provider 契约、UI SDK 和协议规范 |
| [fiex-official-plugins](https://github.com/kevinhuang001/fiex-official-plugins) | 官方插件实现与可编译的 Hello 示例                    |

## 文档适用版本

文档随 Core 源码一起维护。页面涉及版本相关行为时，会标明 Core、manifest schema 或 IPC 协议版本；插件开发请先查看 [版本基线](/reference/source-baseline)。若某项描述与正在使用的 Fiex 版本不符，请以该版本界面和 Release 说明为准，并提交问题时附上版本号与复现步骤。

Fiex 的用户指南、开发手册、协议、需求追溯、版本说明和第三方许可统一维护在本 VitePress 站点。仓库根目录 README 只用于项目入口、下载和站点链接；产品操作说明以本站为准。

[查看最新发布版本](https://github.com/kevinhuang001/fiex/releases/latest) · [查看版本说明](/reference/release-notes) · [查看源代码](https://github.com/kevinhuang001/fiex) · [第三方软件与许可](/reference/third-party-notices)
