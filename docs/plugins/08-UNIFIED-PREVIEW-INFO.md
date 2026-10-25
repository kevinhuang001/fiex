# 08 · Info 与 Preview 的统一 Provider 合同

## 1. Registry 与业务类型

Info 和 Preview 都支持多个内置 Provider 和多个第三方插件 Provider。Core Registry 做统一的匹配、优先级、取消、超时、失败隔离与聚合；无任何特定官方插件 ID 的业务分支。

```text
Built-in Info/Preview Adapter ─┐
                                ├─ InfoResponse / PreviewView → Core Renderer
Plugin Native RPC → deserialize ┘
```

内置 Provider 直接构造公共 Rust 领域类型，第三方返回遵循同一 Schema 的 JSON 并由 Core 解码成**相同**的类型。UI 只渲染统一领域结果，不维护内置/外部两套响应模型。

Core 内置实现应分别组织在 `info/{metadata,audio,video,types}` 与 `preview/{text,image,audio,video,pdf,fallback}` 模块。Registry 只负责注册和编排；音频、视频等解析来源可独立失效、不会拖垮基本属性或其他预览类型。

## 2. 标准注册事件

Manifest 使用单一的 `extensions`：

```json
{
  "permissions": {
    "metadata": {"enabled": true},
    "files": {"read":{"enabled":true}}
  },
  "extensions": {
    "info": [{"id":"audio","title":"音频信息","extensions":["mp3","flac"]}],
    "preview": [{"id":"audio","title":"音频预览","extensions":["mp3","flac"]}]
  }
}
```

插件仅注册对应扩展处理器。Core 已定义 InfoContext/PreviewContext 的输入；不要求在 Manifest 再声明 `inputs` 或单独“Context Provider”类型。

## 3. 标准输入 Context

```json
{
  "sessionId": "preview-session-opaque",
  "resource": {
    "ref": {"mountId":"m1","nodeId":"a18"},
    "metadata": {
      "name":"audio.mp3",
      "kind":"file",
      "extension":"mp3",
      "size":4194304
    },
    "read": {"resourceId":"scoped-read-1"}
  },
  "intent":"preview"
}
```

ContextBuilder 从 Explorer/VFS 候选资源中取得基础属性；**独立 PermissionPolicy** 根据 Manifest 权限、用户批准与本次查看 Scope 对每个字段和可读资源引用进行审核，之后事件一次交付给 Provider。`metadata` 可选，`read` 也可选；没有相应批准就不交付。Opaque read 引用只允许访问**这一个当前文件**、在会话中按 Range/Seek 延迟读取，关闭预览/撤权即失效，绝不在标准 Context 内预传完整音视频/PDF 字节。

内置 Provider 可以由 Core 内部适配器获取同一资源，第三方通过受控文件流访问，不因为内置类型而对外扩大权限。

## 4. InfoResponse

```json
{
  "sections": [
    {
      "id":"audio",
      "title":"音频",
      "rows":[
        {"label":"时长","value":"03:25"},
        {"label":"编码","value":"MP3"}
      ]
    }
  ]
}
```

InfoRenderer 只接收统一 `InfoResponse`，根据 sections 的稳定 ID、排序和展示规则渲染。基本文件属性是独立的 Core Info Provider；音频标签、视频编码、EXIF 等若需要读取文件字节，必须具有 `files.read` 和当前资源 Scope，不能仅靠 `metadata` 权限。

允许多个 Provider 并发生成 sections；Core 按优先级/稳定顺序合并，超时或单个来源错误不吞掉其他来源，防止恶意 HTML 注入或异常大文本。

## 5. PreviewView

Preview Provider 返回 Core 标准 `PreviewView`：可以是由 Core 支持的文本、图像、媒体、PDF、受限 HTML 预览描述或需要 Core 资源代理的 View。选择预览渲染器必须先完成类型匹配、权限审核和受信任资源 URL 生成。

对于大文件，Core 提供有界只读流和 Range/Seek 资源代理，关闭/切换标签回收句柄，不能整文件编码成 JSON，也不得把本地绝对路径直接交给插件 iframe。

## 6. 与自定义 UI 的边界

Info/Preview 作为**标准事件**，可以直接收到已审核的基础 Metadata 与本次可读资源引用。插件若在某个用户操作中打开专门的 HTML UI，可以通过通用 `host.ui.open` 创建受限 Panel 或使用相应 PreviewView 描述；但不能因此多读其他文件。

Connection 点击产生的 `PluginView` 则由 ContentHost 占满右侧主内容区，属于导航类型而非 Info/Preview 弹窗类型。所有 iframe 都使用相同 sandbox 和 UI Bridge 边界。

## 7. 验收

- 文件/目录 Metadata 未批准时，Context 仅有 ref；已批准时基础属性能随事件到达，不要求插件再发 `host.fs.stat`。
- 没有 `files.read` 时，不提供 read 资源；即便插件构造假 resourceId 也不能读取。
- 内置与第三方 Info/Preview 使用相同 DTO 并由同一组件渲染。
- 文本、图片、音频、视频、PDF、fallback 的模块独立测试；5 GiB 以上资源不会整文件载入内存。
- 本地、云盘 VFS、只读 Git VFS 的文件资源身份、撤权、取消、资源替换与超时行为一致。