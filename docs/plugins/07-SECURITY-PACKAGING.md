# 07 · 安全、权限与打包

## 权限

插件 manifest 用布尔值声明三项权限：

```json
{
  "permissions": {
    "filesystem.read": true,
    "filesystem.write": true,
    "network": true
  },
  "networkDomains": ["github.com"]
}
```

- `filesystem.read` / `filesystem.write`：用户文件读取/修改。
- `network`：只允许连接 manifest 申请的域名与用户后来添加的域名。
- `localhost`、`127.0.0.1`、`::1` 始终允许，不要求加入白名单。
- 插件私有 KV、凭据代理、宿主文件选择器、执行外部程序不是权限项。

安装时必须展示全部声明权限；安装成功后才写入授权状态。撤销权限会停止插件进程。域名可以在设置中逐个删除或添加。

## OS 沙箱

| 平台    | 机制                            | 状态                                              |
| ------- | ------------------------------- | ------------------------------------------------- |
| Linux   | Landlock 文件系统规则 + seccomp | 已实现并通过 Git/WebDAV/Archive/Checksum 集成测试 |
| macOS   | `sandbox-exec` Seatbelt 配置    | 已实现，需真机 CI 验证                            |
| Windows | 计划 AppContainer + Job Object  | 未实现；当前普通子进程                            |

Linux 规则：

- 无 `filesystem.read`：不授予用户目录读取。
- 无 `filesystem.write`：不授予用户目录写入。
- 无 `network`：seccomp 拒绝 `socket/connect/socketpair`。
- 有 `network`：允许网络；域名白名单由插件在连接前校验，因为 Landlock 不能按域名过滤。
- 执行外部程序不是权限项；插件始终可以执行系统程序，但子进程继承文件系统/网络沙箱。
- 沙箱初始化失败时拒绝启动插件。

**限制**：域名白名单是插件进程内的策略，不是 OS 级网络过滤；恶意 Native 插件仍可能绕过。Windows 当前没有 OS 级隔离。文档不得把这些描述成完全隔离。

## 安装包

- 只从公开 GitHub Release 下载 `.fiexplugin`（ZIP）。
- 限制下载 60 MiB、解压 256 MiB、2000 个文件。
- 拒绝路径穿越、符号链接、设备文件和未声明入口。
- 安装预览阶段不启动插件；用户确认后才写入安装目录。
- 卸载先撤销授权，再删除程序文件；插件数据和用户同步目录保留。

## 官方插件

官方插件与第三方使用相同包格式、权限、安装和沙箱。官方身份不提供签名或预装特权。发布源为 `kevinhuang001/fiex-official-plugins`。
