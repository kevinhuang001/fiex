# File decorations

Decorations are a read-only presentation layer. A plugin may add badges, a
display label, a short description and a small style whitelist to entries that
Core has already listed. It never changes paths, file contents, sorting or
context-menu behavior.

## Declaration

Plugins only participate when `contributions.decorations` is present.

```json
"decorations": {
  "mode": "none"
}
```

```json
"decorations": {
  "mode": "declarative",
  "priority": 10,
  "rules": [
    {
      "match": { "isDir": false, "extensions": ["zip", "rar"] },
      "fields": {
        "name": {
          "badges": [{ "text": "ZIP", "color": "neutral" }]
        },
        "size": {
          "badges": [{ "text": "包", "color": "blue" }],
          "style": { "color": "#2f6f9f" }
        }
      }
    }
  ]
}
```

```json
"decorations": {
  "mode": "dynamic",
  "method": "decorations.list",
  "priority": 50,
  "scope": { "marker": ".git" }
}
```

- `none` or absent: Core never calls the plugin.
- `declarative`: Core applies the rules locally; no IPC.
- `dynamic`: Core calls `method` once per directory/listing when the scope
  matches and the plugin has `files.read`.

## Targets

`fields` keys map to the visible entry cells:

| key        | 作用位置                   | 支持 badge | 支持 style |
| ---------- | -------------------------- | ---------- | ---------- |
| `name`     | 文件名                     | 是         | 是         |
| `modified` | 修改时间                   | 是         | 是         |
| `size`     | 大小                       | 是         | 是         |
| `kind`     | 种类                       | 是         | 是         |
| `row`      | 整行（列表 / 分栏 / 图标） | 否         | 是         |

- `badges` 最多 3 个，每个 1–4 个字符；相同文字会去重。
- `style` 只允许 `color`、`background`、`fontWeight`、`fontStyle`。
- `row` 只接受 `style`；给 `row` 配 `badges` 会被忽略。
- 未知 key 会被忽略。
- 兼容旧格式：顶层 `badge` / `badges` / `style` 仍然等价于
  `fields.name.badges` / `fields.name.style`。

## Dynamic input

```json
{
  "directory": "/Users/kevin/Documents",
  "entries": [
    { "path": "...", "name": "a.txt", "isDir": false, "extension": "txt", "mime": "text/plain" }
  ]
}
```

## Dynamic output

```json
{
  "decorations": [
    {
      "path": "/Users/kevin/Documents/a.txt",
      "icon": "fiex-resource://org.example.icons/markdown",
      "providers": ["Git"],
      "fields": {
        "name": {
          "badges": [{ "text": "M", "color": "orange", "tooltip": "已修改" }],
          "style": { "color": "#c2410c", "fontWeight": "600" }
        },
        "modified": {
          "style": { "color": "#c2410c" }
        },
        "size": {
          "badges": [{ "text": "新", "color": "green" }]
        },
        "kind": {
          "style": { "color": "#2f6f9f" }
        },
        "row": {
          "style": { "background": "#fff9f0" }
        }
      }
    }
  ]
}
```

旧的写法仍可用，等价于只装饰文件名：

```json
{
  "path": "...",
  "badges": [{ "text": "M", "color": "orange" }],
  "style": { "color": "#c2410c" },
  "label": "a.txt",
  "description": "Git：已修改"
}
```

## Limits

- 每个 target 最多 3 个 badge；`text` 1–4 个字符，tooltip 200。
- `label` 120 字符，`description` 200，tooltip 200。
- Colors: `orange`, `green`, `red`, `blue`, `purple`, `gray`, `neutral` or
  validated hex colors.
- Style: `color`, `background`, `fontWeight`, `fontStyle` only.
- Unknown paths, targets and unsafe values are ignored.
- `row` badges are ignored; only its style is applied.
- `icon` 必须是 `fiex-resource://<plugin-id>/<resource-id>`，命名空间必须等于
  当前插件 id；详见 [19 插件资源与文件图标](19-FILE-ICONS.md)。
- Core renders files first and applies decorations asynchronously; empty
  results are not requested again for the same listing.
