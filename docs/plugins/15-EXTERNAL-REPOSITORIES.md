# 15 · 外部仓库、Release 契约与 CI

## 仓库边界

| 仓库                                                                            | 内容                                                         |
| ------------------------------------------------------------------------------- | ------------------------------------------------------------ |
| [Fiex Core](https://github.com/kevinhuang001/fiex)                              | 文件管理、插件宿主、安装与权限、通用 VFS、React UI           |
| [Fiex SDK](https://github.com/kevinhuang001/fiex-sdk)                           | `rust/` Native IPC SDK；`ui/` iframe Bridge SDK              |
| [Fiex Official Plugins](https://github.com/kevinhuang001/fiex-official-plugins) | Git/WebDAV/SMB/Archive/Checksum/Everything 独立 crate 与打包 |

Core 的 `sdk/` 是指向 `kevinhuang001/fiex-sdk` 特定提交的 submodule。Core 不包含插件实现源码、插件编译产物或预装插件。

```sh
git clone https://github.com/kevinhuang001/fiex.git
cd fiex
git submodule update --init sdk
```

## Release 契约

官方插件仓库发布 tag：`plugins-v<semver>`。

Release 必须包含：

1. `fiex-catalog.json`
2. 每个插件/平台的 `.fiexplugin`：
   `<slug>-<platform>.fiexplugin`

`slug`：`git | webdav | smb | archive | checksum | everything`

`platform`：`linux-x86_64 | macos-aarch64 | macos-x86_64 | windows-x86_64`

`fiex-catalog.json`：

```json
{
  "schemaVersion": 1,
  "plugins": [
    {
      "id": "org.fiex.git",
      "name": "Git",
      "version": "0.1.0",
      "description": "源代码管理与历史浏览",
      "author": { "name": "Fiex Contributors" },
      "permissions": ["folders.read", "files.read", "files.write", "network"],
      "networkDomains": ["github.com"],
      "assets": {
        "linux-x86_64": "git-linux-x86_64.fiexplugin",
        "macos-aarch64": "git-macos-aarch64.fiexplugin",
        "windows-x86_64": "git-windows-x86_64.fiexplugin"
      }
    }
  ]
}
```

Core 的插件商店读取该文件，只展示当前平台存在 asset 的条目；安装仍走下载、manifest 校验、权限确认和三阶段安装。

## CI 分工

- **Core CI**：前端格式/单测/构建/e2e；Rust fmt/clippy/单测；SDK mock 进程 IPC；三平台安装包。
- **SDK CI**：Rust 帧解析、HostClient、TypeScript Bridge 类型与 Node 测试。
- **Plugin CI**：六个 crate 的 fmt/test/clippy/build、真实进程握手、三平台打包、WebDAV/SMB 真实服务器测试、Core 宿主集成。

任何仓库升级 SDK 都必须固定 commit 并各自跑 CI；不把插件 CI 合并进 Core。
