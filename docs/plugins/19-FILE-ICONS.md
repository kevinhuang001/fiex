# 插件静态资源与文件图标

插件可以在 manifest 里声明静态资源（图标、图片），并在 **dynamic decorations**
返回某个 entry 时附带资源图标。图标不是独立贡献项：它跟随 decoration 一起
产生，因此可以按文件状态、MIME、Git 状态或插件自己的分类动态决定。

资源引用必须带插件命名空间：

```text
fiex-resource://<plugin-id>/<resource-id>
```

Core 只接受当前产生 decoration 的插件自己的命名空间；跨插件引用、未知命名空间
和未声明资源都会被忽略。

## manifest

```json
{
  "contributions": {
    "resources": {
      "markdown": {
        "path": "assets/markdown.svg",
        "mediaType": "image/svg+xml"
      }
    }
  }
}
```

`resources` 是 `id -> 路径` 映射。值可以是路径字符串，也可以是带 `mediaType`
的对象；路径相对于插件包根目录。资源只声明，不直接决定匹配规则。

## dynamic decorations 返回图标

```json
{
  "decorations": [
    {
      "path": "/Users/kevin/Documents/notes.md",
      "icon": "fiex-resource://org.example.icons/markdown",
      "fields": {
        "name": {
          "badges": [{ "text": "M", "color": "orange" }]
        }
      }
    }
  ]
}
```

- `icon` 必须是 `fiex-resource://<plugin-id>/<resource-id>`。
- 命名空间必须等于当前插件 manifest 的 `id`。
- 资源必须在 `contributions.resources` 中声明并通过安装校验。
- Core 返回给前端的 decoration 会附带解析后的 `iconPath`；前端通过 Tauri
  asset protocol 加载为 `<img>`。
- 图标属于 decoration 的优先级合并结果；没有匹配时回退内置 Material Icon
  Theme 图标。

## 为什么用 dynamic decorations

- 图标可以依赖文件状态（Git 已修改、同步中、云端、加密等），而不仅是后缀。
- 不需要新增一套 `fileIcons` 匹配规则；decorations 已经是插件拿到 entry 列表
  的唯一入口。
- 同一套优先级、去重、资源校验和生命周期同时覆盖 badge、颜色和图标。

## 资源限制

- 每个插件最多 **64** 个资源。
- 单个资源最大 **256 KiB**。
- 只允许 `image/svg+xml`、`image/png`、`image/jpeg`、`image/webp`。
- 路径必须位于插件包内，禁止绝对路径、`..`、符号链接和目录。
- SVG 禁止 `<script>`、`<foreignObject>`、`<image>`、事件属性、`javascript:`、
  外部 `href` 和 `url(http...)`。
- 安装阶段校验失败会拒绝整个插件包。

## 插件自己访问资源

插件自己的 iframe 可以直接用包内相对路径访问 `ui/` / `assets/`。需要让 Core
或其他宿主 UI 解析时，必须使用完整命名空间：

```text
fiex-resource://org.example.icons/markdown
```

不允许把 `resources` 的裸 id、绝对路径或另一个插件的命名空间交给宿主解析。

## 不在本协议内

- 静态 `fileIcons` 贡献：已移除，图标统一走 dynamic decorations。
- 不允许远程 URL、data URL 或跨插件资源引用。
- 图标只影响展示，不改变文件类型识别、打开方式或 MIME。
