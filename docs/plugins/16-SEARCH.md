# 16 · 搜索来源插件

Core 拥有搜索框、防抖、取消、结果合并和展示；搜索插件只返回结构化条目。

## 注册

```json
{
  "contributions": {
    "search": [
      {
        "id": "everything",
        "title": "Everything",
        "icon": "magnifying-glass",
        "scope": "global",
        "command": "search.query"
      }
    ]
  }
}
```

`scope`：`global` 或 `directory`。搜索注册本身不需要权限；索引本地内容通常需要 `filesystem.read`。

## 查询

Core 调用：

```json
{
  "method": "search.query",
  "params": {
    "query": "fiex",
    "root": "C:\\Users\\kevin",
    "hidden": false,
    "ignores": ["node_modules"],
    "limit": 200
  }
}
```

返回：

```json
{
  "entries": [
    {
      "path": "C:\\Users\\kevin\\fiex-notes.md",
      "name": "fiex-notes.md",
      "isDir": false,
      "isSymlink": false,
      "size": 2451,
      "modified": 1770000000,
      "created": 1770000000,
      "extension": "md",
      "mime": "text/markdown",
      "readonly": false,
      "linkTarget": null,
      "tags": [],
      "sync": null
    }
  ],
  "truncated": false
}
```

- 每次最多 500 条。
- 不返回 HTML。
- 单个来源失败或超时不影响本地结果和其他来源。
- Core 去重后与本地索引结果合并。

## Everything 插件

- 只在 Windows 提供入口。
- 使用 Everything IPC 查询现有索引，不读取文件内容。
- Everything 未运行时返回错误；Core 忽略该来源。
- 不重建 Everything 索引；`search.rebuild` 返回明确错误。
