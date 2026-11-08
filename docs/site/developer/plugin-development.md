# 插件开发

## 先检查版本

当前 Core 的插件 manifest 使用 `schemaVersion: 2`，Native SDK 握手使用 `protocolVersion: 7.0`。Core 会在插件初始化时核对协议版本；不能只改 manifest 字符串来绕过 SDK 不兼容。

项目有独立的 Rust SDK 和 UI SDK。Native 后端通过 Rust SDK 访问 Core 提供的能力；插件面板/预览界面通过 UI SDK 与受控的 Bridge 通信。插件可以只有 Native 后端，也可以同时包含 UI。

## Info 字段与按需计算

Info 字段写在 manifest 的 `extensions.info.sections` 中。每个 section 提供稳定的 `id`、标题和字段列表；每个字段提供 `id`、标签和 `compute` 策略。字段声明由 Core 在安装或启用插件时建立索引，打开“显示简介”时不需要启动插件来询问它有哪些字段。

`compute` 目前支持 `onOpen` 与 `onDemand`：前者在面板打开后自动请求；后者先显示为“尚未计算”，用户点击“计算”后才请求该字段。SDK 的 `InfoProvider` 只实现 `compute(ctx, fields)`，并返回与请求字段对应的文本结果。Core 负责字段校验、缓存、会话、取消和权限检查。旧版一次性 Info 行结果与旧 RPC 格式不属于 protocol 7.0。

示例 manifest：

```json
{
  "extensions": {
    "info": {
      "priority": 80,
      "match": { "extensions": ["txt"], "kinds": ["file"] },
      "sections": [
        {
          "id": "integrity",
          "title": "完整性",
          "fields": [
            { "id": "sha256", "label": "SHA-256", "compute": "onDemand" }
          ]
        }
      ]
    }
  }
}
```

只有字段计算确实需要文件内容时，才声明并请求相应的读取权限；声明字段本身不等于读取文件。

## 从真实示例开始

官方插件仓库维护可编译示例及每种扩展点的说明。建议直接跟随仓库中的 Hello 插件，而不是复制网上旧的 manifest：

- [插件示例新手手册](https://github.com/kevinhuang001/fiex-official-plugins/blob/main/example/README.md)
- [扩展点指南](https://github.com/kevinhuang001/fiex-official-plugins/tree/main/example/docs)
- [Rust SDK 新手手册](https://github.com/kevinhuang001/fiex-sdk/blob/main/rust/README.md)
- [UI SDK](https://github.com/kevinhuang001/fiex-sdk/tree/main/ui)

快速编译示例：

```sh
git clone --recurse-submodules https://github.com/kevinhuang001/fiex-official-plugins.git
cd fiex-official-plugins
cargo build --manifest-path example/hello/Cargo.toml
```

SDK 仓库还提供真实进程检查工具，可按示例手册验证 initialize 握手、一次调用和正常退出。

## Manifest 与注册

Manifest 的顶层结构包含 schemaVersion、插件 id、名称、版本、runtime、permissions 和 extensions。标准扩展注册写在 extensions 中；权限声明写在 permissions 中。扩展声明只表示插件提供功能，不会自动授予文件、网络或命令访问权限。

当前常见扩展包括连接、VFS、Info、Preview、Decorations、Context Menu、Search、Hooks、Toolbar、Settings 和 Themes。只注册实际实现的扩展，并使用与目标 Core/SDK 一致的字段。

## 常用 Provider

Rust SDK 的 Providers builder 可以注册标准 provider trait：

| Provider            | 标准事件                                 |
| ------------------- | ---------------------------------------- |
| ConnectionProvider  | 连接校验、打开和创建生命周期             |
| VfsProvider         | 虚拟目录发现、列出、查询、读写和文件操作 |
| InfoProvider        | 按请求计算 Info 字段                     |
| PreviewProvider     | 文件预览视图                             |
| DecorationProvider  | 文件行和列的装饰                         |
| ContextMenuProvider | 右键菜单项及执行                         |
| SearchProvider      | 插件搜索来源                             |
| HookProvider        | 文件打开等生命周期 hook                  |
| ToolbarProvider     | 工具栏命令和面板                         |

自定义 RPC 可作为标准分发器的 fallback；文件系统能力应优先使用共享 VFS/provider 接口。

## 文件与 VFS

Core 通过 ResourceRef 将当前操作中授权的对象传给插件。它是受范围约束的资源身份，不是任意宿主路径或永久令牌。读取和写入使用 SDK 的 reader/writer；写入先暂存，调用 commit 后才提交，未提交或失败的写入可取消。

VFS provider 应声明实现的 operation，并可根据当前连接或对象返回更窄的发现能力。Core 负责挂载身份、URI、宿主授权范围、能力预检和数据转发。不要在插件中拼造 Core 管理的 fiex:// URI 或直接访问用户文件。

## 权限声明

权限逐项声明，例如 metadata、folders.read、files.read、files.write、network 和 commands。Core 安装/激活流程记录用户批准的声明快照；运行时还会校验当前授权、当前操作范围和具体目标。网络域名需精确声明；commands 只允许 manifest 列出的命令。

外部命令是高风险能力：批准的命令在宿主授权流程下以当前用户身份运行，可能访问 SDK 文件范围以外的文件、网络或启动其他进程。插件不能把它当作普通的文件读取 API。

## 发布与兼容

官方插件包由 official 仓库脚本构建为 Fiex 可安装包。发布前检查平台 entrypoint、schema、protocolVersion、权限声明和包内资源路径，并验证包可在目标 Core 版本中启动。协议升级要同时协调 Core、SDK 和插件；不要保留已移除的旧 manifest 或 RPC 格式。
