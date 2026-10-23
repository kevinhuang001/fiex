# 07 · 安全、权限与打包

## 授权和资源代理

安装只保存插件，默认不激活。点击激活时展示当前 manifest 的全部能力、主机和外部命令，逐项同意后由 Core 原子提交授权；部分同意或过期声明不能激活。撤销任一项会停用并终止插件进程。

```json
{
  "permissions": {
    "folders": { "read": { "enabled": true }, "create": true, "delete": false, "rename": false },
    "files": { "read": true, "write": true },
    "network": { "enabled": true, "domains": ["example.com", "127.0.0.1"] },
    "commands": { "enabled": true, "allow": ["example-command"] }
  }
}
```

文件目录读取、创建目录项、删除、重命名、文件读取和写入分别授权。访问范围是任务启动时的当前目录及其子目录；符号链接不能越界。后台任务固定此范围。内容写入由 Core 暂存，提交前检查原文件变化，失败或取消时由 SDK 清理暂存。

HTTP(S) 和 TCP 共用 `network` 能力与精确域名/IP 白名单，不分协议授权。回环和局域网地址同样需要明确授权。重定向逐跳检查目标，跨源移除凭据，拒绝 HTTPS 降级。

外部命令由 Host 从环境变量 PATH 查找，以当前用户身份执行插件提供的参数，不施加文件 SDK 的目录范围限制。授权命令存在访问其他文件、联网和启动程序的风险；界面用统一的外部命令风险提示说明。

## OS 沙箱

插件原生进程不获得用户目录权限、可写安装目录、直接网络、其他 IPC 或子进程执行权限；只保留运行时系统读取和继承的控制管道。批准网络或文件能力不会放宽原生沙箱。

| 平台    | 实现                                                 | 验证      |
| ------- | ---------------------------------------------------- | --------- |
| Linux   | Landlock + seccomp，拒绝 Unix socket                 | GitHub CI |
| macOS   | 自定义 deny-default Seatbelt，仅允许指定插件入口执行 | GitHub CI |
| Windows | AppContainer + Job Object，关闭网络能力和子进程创建  | GitHub CI |

沙箱初始化失败时拒绝启动，禁止降级到无沙箱运行。三平台行为以当前 CI 结果为准。资源代理与原生命令授权边界详见 [激活和资源代理](18-ACTIVATION-AND-BROKER.md)。

## 安装包

- 只从公开 GitHub Release 下载 `.fiexplugin`（ZIP）。
- 限制下载 60 MiB、解压 256 MiB、2000 个文件。
- 拒绝路径穿越、符号链接、设备文件和未声明入口。
- 安装预览阶段不启动插件；用户确认后才写入安装目录。
- 卸载先撤销授权，再删除程序文件；插件数据和用户同步目录保留。

## 官方插件

官方插件与第三方使用相同包格式、权限、安装和沙箱。官方身份不提供签名或预装特权。发布源为 `kevinhuang001/fiex-official-plugins`。

## Package signatures

Every signed `.fiexplugin` carries an Ed25519 publisher signature over its
manifest, and the manifest carries SHA-256 digests for every executable and UI
file. Core verifies both at install time and again before spawning an installed
plugin. Official plugins are not special: they use the same trust-on-first-use
publisher fingerprint as third-party plugins. See [16 SIGNING](16-SIGNING.md).
