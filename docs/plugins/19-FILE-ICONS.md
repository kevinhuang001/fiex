# 插件文件图标

插件可以在 manifest 里声明静态资源和文件图标规则，让 Core 在文件列表、图标视图、画廊和分栏里替换某类文件的图标。注册发生在**安装时**，不是运行时：Core 校验路径、类型、大小和 SVG 内容后才允许使用。

## manifest

```json
{
  "contributions": {
    "resources": {
      "rust-icon": {
        "path": "assets/rust.svg",
        "mediaType": "image/svg+xml"
      },
      "designer-file": "assets/designer.png"
    },
    "fileIcons": [
      {
        "match": { "extensions": ["rs"] },
        "icon": "rust-icon",
        "priority": 10
      },
      {
        "match": { "names": ["design.psd"], "isDir": false },
        "icon": "designer-file",
        "priority": 20
      }
    ]
  }
}
```

`resources` 是 `id -> 路径` 映射。值可以是路径字符串，也可以是带 `mediaType` 的对象；路径相对于插件包根目录。`fileIcons` 的 `icon` 必须引用已声明的资源 id。

## 匹配字段

`match` 支持：

| 字段         | 说明                                                           |
| ------------ | -------------------------------------------------------------- |
| `extensions` | 后缀，不区分大小写，可带前导点；`["rs", "toml"]`               |
| `names`      | 完整文件名，不区分大小写；优先用于 `Dockerfile` 这类无后缀文件 |
| `mime`       | MIME，支持 `text/*` 这样的前缀通配                             |
| `isDir`      | 只匹配文件或只匹配目录；省略表示都匹配                         |

规则按 `priority` 从高到低匹配，`priority` 范围 `-1000..1000`，默认 `0`；同优先级按插件 id、资源 id 排序。没有命中时回退到 Fiex 内置的 Material Icon Theme 图标。

## 资源限制

- 每个插件最多 **64** 个资源、**128** 条图标规则。
- 单个资源最大 **256 KiB**。
- 只允许 `image/svg+xml`、`image/png`、`image/jpeg`、`image/webp`。
- 路径必须位于插件包内，禁止绝对路径、`..`、符号链接和目录。
- SVG 禁止 `<script>`、`<foreignObject>`、`<image>`、事件属性、`javascript:`、外部 `href` 和 `url(http...)`。
- 安装阶段校验失败会拒绝整个插件包。

## 加载方式

Core 的 `plugin_file_icons` 返回已解析的绝对路径和规则；前端通过 Tauri asset protocol 加载为 `<img>`，不给插件页面 Tauri capability，也不让资源走网络。

```text
manifest → install validation → plugin_file_icons
        → FileIcon matches entry → convertFileSrc(path) → <img>
```

- 插件被禁用或卸载后规则立即失效；`fiex:plugins-changed` 会让前端重新加载规则。
- `<img>` 加载失败时回退到内置图标。
- 静态资源不消耗插件进程，不参与 UI Bridge。

## 为什么不做运行时注册

运行时 `registerResource(path)` 会让插件在安装后继续暴露任意本地文件，破坏安装时的签名/清单校验，也无法在卸载时可靠回收。因此 v1 只支持 manifest 静态声明；需要动态生成的图标应生成在插件包内并重新发布。

## 不在本协议内

- 不替换系统应用图标（`.app`/`.exe` 仍可回退原生图标）。
- 不允许远程 URL、data URL 或跨插件资源引用。
- 图标只影响展示，不改变文件类型识别、打开方式或 MIME。
