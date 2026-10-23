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
      "badge": { "text": "ZIP", "color": "neutral" }
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
  matches and the plugin has `filesystem.read`.

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
      "badge": { "text": "M", "color": "orange", "tooltip": "已修改" },
      "label": "a.txt",
      "description": "Git：已修改",
      "style": { "color": "#c2410c", "fontWeight": "600" }
    }
  ]
}
```

## Limits

- Badge text 1–4 characters; at most 3 badges per entry.
- `label` 120 characters, `description` 200, tooltip 200.
- Colors: `orange`, `green`, `red`, `blue`, `purple`, `gray`, `neutral` or
  validated hex colors.
- Style: `color`, `background`, `fontWeight`, `fontStyle` only.
- Unknown paths and unsafe values are ignored.
- Core renders files first and applies decorations asynchronously; empty
  results are not requested again for the same listing.
