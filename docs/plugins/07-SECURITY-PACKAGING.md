# 07 · 安全、权限与打包

## 权限

插件 manifest 用布尔值声明三项权限：

```json
{
  "permissions": {
    "filesystem": { "read": true, "write": true },
    "network": { "enabled": true, "domains": ["github.com"] },
    "commands": { "enabled": true, "allow": ["git"] }
  }
}
```

- `filesystem.read` / `filesystem.write`：用户文件读取/修改。
- `network`：只允许连接 manifest 申请的域名与用户后来添加的域名。
- `localhost`、`127.0.0.1`、`::1` 等回环地址始终允许，不要求加入白名单。
- 插件进程本身永远不能执行外部程序；需要运行 Git、ffmpeg 等命令时，manifest 用 `permissions.commands` 申请可执行文件名，所有调用都经宿主 `host.exec` 代理。

安装时必须展示全部声明权限；安装成功后才写入授权状态。撤销权限会停止插件进程。域名可以在设置中逐个删除或添加。

## OS 沙箱

| 平台    | 机制                                  | 状态                                              |
| ------- | ------------------------------------- | ------------------------------------------------- |
| Linux   | Arapuca：Landlock + seccomp           | 已实现并通过 Git/WebDAV/Archive/Checksum 集成测试 |
| macOS   | Arapuca：`sandbox-exec` Seatbelt 配置 | 已实现，需真机 CI 验证                            |
| Windows | Arapuca：AppContainer + Job Object    | 已接入启动器，需 Windows CI 真机验证              |

Linux 规则：

- 无 `filesystem.read`：不授予用户目录读取。
- 无 `filesystem.write`：不授予用户目录写入。
- 无 `network`：seccomp 拒绝 `socket/connect/socketpair`。
- 有 `network`：允许网络；域名白名单由插件在连接前校验，因为 Landlock 不能按域名过滤。
- 沙箱禁止插件进程执行外部程序（Arapuca `allow_exec = false`）。命令只能通过宿主 `host.exec` 代理执行，宿主校验用户授权的命令名，拒绝 shell、路径和未授权程序。
- 沙箱初始化失败时拒绝启动插件。

**限制**：域名白名单是插件进程内的策略，不是 OS 级网络过滤；`host.exec` 代理执行的是宿主权限下的程序，因此用户授权命令等同于授予该命令的能力。Windows 的 AppContainer 启动器尚未在 CI 真机验证前视为完成。文档不得把这些描述成完全隔离。

## 安装包

- 只从公开 GitHub Release 下载 `.fiexplugin`（ZIP）。
- 限制下载 60 MiB、解压 256 MiB、2000 个文件。
- 拒绝路径穿越、符号链接、设备文件和未声明入口。
- 安装预览阶段不启动插件；用户确认后才写入安装目录。
- 卸载先撤销授权，再删除程序文件；插件数据和用户同步目录保留。

## 官方插件

官方插件与第三方使用相同包格式、权限、安装和沙箱。官方身份不提供签名或预装特权。发布源为 `kevinhuang001/fiex-official-plugins`。
