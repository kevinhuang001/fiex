# 08 · Info 与 Preview

## 共同点

- Manifest 中 `info` / `preview` 都是**单个对象**，不是数组；
- 没有贡献 id；Provider 身份是插件 id；
- 内置 Provider 和插件 Provider 共享事件匹配规则、`EventContext.targets` 和调用入口；
- Info 可匹配多个插件 Provider 并汇总 sections；Preview 根据优先级选择一个 Provider；
- `match` 是包含谓词：`extensions`、`mimeTypes`、`kinds`；
- 元数据和读取权限由 `PermissionPolicy` + 当前 Scope 决定，缓存不会绕过授权。

## Info

插件 Manifest：

```json
"info": {
  "priority": 100,
  "match": { "extensions": ["pdf"], "kinds": ["file"] }
}
```

Core 调用 `info.resolve`，传入通用 `EventContext`（method 本身就是事件名称）：

```json
{
  "sessionId": "opaque",
  "targets": [
    {
      "ref": { "mountId": "m1", "nodeId": "n9" },
      "metadata": { "name": "a.pdf", "kind": "file", "size": 1024 },
      "read": { "resourceId": "read-opaque" }
    }
  ]
}
```

插件返回：

```json
{
  "sections": [
    {
      "id": "document",
      "title": "文档",
      "rows": [{ "label": "页数", "value": "12" }]
    }
  ]
}
```

- section id 由响应提供；Core 不要求 Manifest 提供 id；
- `read` 只有 `files.read` 被批准时才出现；
- 插件自己的 section 和 Core 的 metadata/audio/video section 合并展示；
- 插件失败只影响该 section。

## Preview

Manifest：

```json
"preview": {
  "priority": 80,
  "maxBytes": 1048576,
  "match": { "extensions": ["md"], "kinds": ["file"] },
  "panel": { "entry": "ui/preview.html", "title": "Markdown" }
}
```

Provider 列表 id：

```text
plugin:<plugin-id>
```

Core 调用 `preview.open`，传入相同的 `EventContext.targets`。插件返回 `PreviewView`：

- `kind: "text"` / `"html"` / `"pdf"` / `"image"` / `"audio"` / `"video"` 等 Core 支持的视图；
- html 面板必须是插件包内文件，通过 Core CSP/路径校验；
- 不允许返回任意外部 URL；
- `providerId` 使用 `plugin:<plugin-id>`。

## 候选选择

- 按 `priority` 降序；
- 同优先级按注册顺序/内置优先策略；
- 用户可在预览切换器选择其他候选；
- 没有 Provider 时使用 fallback。

## 资源读取

- Preview 的 `read.resourceId` 是会话绑定、限期、进程所有权的引用；
- 大文件分块读取，不内联到 JSON；
- 预览会话关闭、插件撤权或文件身份变化后，旧引用立即失效。
