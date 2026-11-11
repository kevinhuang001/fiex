# Provider 实现

Provider 将插件业务接入 Fiex 的标准事件。SDK 里的 `Providers` builder 会把 trait 实现登记到初始化能力中，并按 Core 指定的 provider ID 分发请求。未注册的请求可由 `serve_with` 的 fallback 处理。

## 注册标准 Provider

```rust
let providers = sdk::Providers::new()
    .connection(MyConnection)
    .info(MyInfo)
    .preview(MyPreview)
    .decorations(MyDecorations)
    .context_menu(MyFileMenu)
    .connection_context_menu(MyConnectionMenu)
    .search(MySearch)
    .hooks(MyHooks)
    .toolbar(MyToolbar)
    .filesystem(MyFiles);

providers.serve_with("org.example.plugin", custom_rpc)?;
```

每种 provider 可以省略；只注册实际由插件处理的事件。若同一种扩展需要多个实现，使用对应的 `*_named(id, provider)` 注册稳定的 provider ID。Core 负责选择被调用的实例，插件自己不能从请求参数构造可信 provider 身份。

## 标准事件与职责

| Trait                           | Core 调用的方法                         | 实现时要处理的内容                              |
| ------------------------------- | --------------------------------------- | ----------------------------------------------- |
| `ConnectionProvider`            | `connection.validate/open/create`       | 校验连接配置；打开连接后返回 VFS 位置或插件视图 |
| `InfoProvider`                  | `info.compute`                          | 只计算请求中列出的字段 ID，并按要求返回文本结果 |
| `PreviewProvider`               | `preview.open`                          | 返回插件包内预览页面或相应 PreviewView          |
| `DecorationProvider`            | `decorations.resolve`                   | 对请求对象返回动态 badge/样式贡献               |
| `ContextMenuProvider`           | `contextMenu.resolve/execute`           | 分离菜单项声明与用户执行后的动作                |
| `ConnectionContextMenuProvider` | `connectionContextMenu.resolve/execute` | 对已保存连接提供操作                            |
| `SearchProvider`                | `search.query`                          | 根据请求模式返回结果、路径引用和可选标签        |
| `HookProvider`                  | `hooks.invoke`                          | 按事件契约响应打开文件前/后的 hook 请求         |
| `ToolbarProvider`               | `toolbar.invoke`                        | 执行命令或返回面板相关动作                      |
| `VfsProvider`                   | `vfs.*`                                 | 见[VFS 章节](/developer/plugin-development/vfs) |

Trait 的具体签名和 DTO 以锁定 SDK 的 `sdk/rust/src/providers.rs`、`sdk/rust/contracts/src/providers.rs` 为准。SDK 类型会随协议演进；复制其他版本样例前先比对 Cargo submodule commit。

## Info 字段：声明一次，按键计算

Info Provider 在 manifest 的 `extensions.info.title` 和 `extensions.info.subsections` 中静态声明。每个 Provider 在界面只产生一个 Section；Subsection 没有可见标题，只用于资源筛选、字段顺序和计算路由。字段 ID 必须稳定，每个字段选择 `onOpen` 或 `onDemand`：

```json
{
  "extensions": {
    "info": {
      "match": { "extensions": ["md"], "kinds": ["file"] },
      "title": "文档信息",
      "subsections": [
        {
          "id": "document",
          "match": { "extensions": ["md", "markdown"] },
          "fields": [
            { "id": "title", "label": "标题", "compute": "onOpen" },
            { "id": "word-count", "label": "字数", "compute": "onDemand" }
          ]
        }
      ]
    }
  }
}
```

`InfoProvider::compute(ctx, fields)` 收到本次要计算的精确 `InfoFieldKey` 列表，每个键由 `(subsectionId, fieldId)` 组成。只返回这些键的结果；Core 管理字段索引、权限、缓存、面板会话和取消。一个 Provider 可声明多个 Subsection，字段按声明顺序平铺在同一个 Provider 标题下。未匹配的 Subsection 会在打开面板前移除；没有适用字段的 Provider 不创建空 Section。不要实现旧版 `info.describe` 或一次性 Info 行协议。

同一个插件可使用 `extensions.info.providers` 注册多个命名 Provider。每个实例必须声明自己的 `id`、`title` 和 `subsections`，并可声明自己的 `match` 与 `priority`；实例不继承另一个 Provider 的字段。Core 为每个实例建立独立 Provider ID 和计算命名空间。

Subsection 的 `match` 支持 `extensions`、`mimeTypes` 和 `kinds`。数组值在同一维度内按 OR 匹配；多个非空维度按 AND 匹配。扩展名会忽略大小写和开头的点，MIME 类型支持 `audio/*` 形式。匹配只读取 Core 可信资源快照中的元数据，不会为了列出字段读取远程或大型文件内容。MIME 通配符只能出现在子类型的完整 `*` 位置。

## 匹配规则与优先级

Manifest 中的 `match` 可以限定扩展名、对象类型等。Core 会先选出匹配的 provider，再校验对象范围和权限。Priority 决定多个合适候选间的排序/优先关系，不会强制运行 provider。Context menu 还可限制选择数量和选择类型，详细 schema 参照 Hello manifest 与当前 Core validator。

编写 provider 时：

1. 校验请求字段，不把 JSON 缺失/错型默认成可信值。
2. 从 SDK `RequestContext` 或 trait context 读取资源、用户选择和 locale；不要从请求拼接宿主磁盘路径。
3. 执行有副作用的命令前再次确认参数和当前授权。
4. 返回稳定、可序列化、适合 UI 显示的结果；原始凭据和大块文件内容不要塞入结果 JSON。
5. 长耗时工作使用 SDK task API，并响应协作取消。

## 自定义 RPC

自定义方法适合插件内部业务协议；文件操作仍走 Core 文件服务或 VFS provider。`serve_with` fallback 只应接收未被标准 provider 接管的方法。自定义方法名建议使用插件 ID 前缀，例如 `org.example.plugin.sync`，避免与标准方法冲突。

方法形状和错误码见 [IPC 协议参考](/developer/ipc)。
