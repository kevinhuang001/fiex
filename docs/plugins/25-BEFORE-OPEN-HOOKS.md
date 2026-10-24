# 25 · 打开前钩子（beforeOpen）

[索引](README.md) · 相关：[07 安全与打包](07-SECURITY-PACKAGING.md) · [18 激活与代理资源](18-ACTIVATION-AND-BROKER.md)

插件可以在 Core 打开文件**之前**接管这次打开。首个使用者是 archive：双击压缩包时直接解压到当前目录，而不是交给系统应用。

## 1. 权限与声明

钩子是单独授权的能力，manifest 需要声明：

```json
"permissions": {
  "hooks": { "enabled": true }
},
"contributions": {
  "hooks": [
    {
      "id": "beforeOpen",
      "event": "beforeOpen",
      "method": "archive.beforeOpen",
      "priority": 100,
      "extensions": ["zip", "rar", "7z", "tar", "gz"]
    }
  ]
}
```

| 字段         | 说明                                                      |
| ------------ | --------------------------------------------------------- |
| `id`         | 插件内唯一                                                |
| `event`      | v1 仅支持 `beforeOpen`                                    |
| `method`     | 插件方法名；Core 通过 `plugin.invoke` 调用                |
| `priority`   | 0–1000，越大越先执行；同优先级按 `<pluginId>:<id>` 字典序 |
| `extensions` | 可选的扩展名过滤；省略表示所有文件                        |

只有 **已启用、已授权** 且授予了 `hooks` 权限的插件会参与。

## 2. 执行语义

1. Core 按优先级从高到低依次调用钩子，参数为：

```json
{ "path": "/a/b.7z", "name": "b.7z", "extension": "7z", "directory": "/a" }
```

2. 返回值决定下一步：

| 返回                                   | 含义                                             |
| -------------------------------------- | ------------------------------------------------ |
| `{ "handled": true }`                  | 消费这次打开，**不再**执行后续钩子或默认打开     |
| `{ "handled": true, "preview": true }` | 消费打开，但改为打开预览弹窗（例如需要输入密码） |
| 其它 / 空                              | 放行，继续下一个钩子；都放行时 Core 执行默认打开 |

3. 钩子抛错会被记录到插件诊断日志，并且**继续**下一个钩子：坏掉的钩子不能导致文件打不开。

## 3. archive 的行为

- 双击压缩包 → 调用 `archive.beforeOpen` → 解压到压缩包所在目录下、以压缩包名命名的文件夹；若同名已存在，自动追加 ` (1)`、` (2)` 后缀（`unique()` 逻辑，与面板解压一致）。
- 解压成功返回 `{ "handled": true, "destination": "..." }`。
- 遇到需要密码的压缩包：返回 `{ "handled": true, "preview": true }`，由预览面板提示输入密码后再解压。
- 同时注册了 `contributions.preview`：普通预览（空格）可以在解压前查看条目，支持密码输入与"解压到当前目录"。

## 4. 约束

- 钩子与面板共用同一套沙箱、Bridge 与权限检查，不能获得额外能力。
- 单次打开最多执行 `hooks` 声明中的方法；Core 不重试、不级联调用。
- 隐藏文件、目录不触发 `beforeOpen`（目录由导航处理）。
