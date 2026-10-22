# 07 · 安全模型、插件包与跨平台发布

[索引](README.md) · [上一篇：任务](06-TASKS.md) · [下一篇：SDK](08-SDK.md)

## 1. 威胁模型

安装包含 Native 后端的插件意味着运行第三方代码；即使 UI-only 插件不含 Native 二进制，iframe JavaScript 也必须隔离并对每条 Host API 请求鉴权。**独立进程只提供崩溃/内存隔离，不等于权限隔离**；普通用户进程仍可直接访问该用户可访问的文件、环境变量和网络资源。Fiex 的 API 权限检查可以防止通过宿主 API 的越权，但不能据此声称杜绝恶意插件的 OS 直接访问。

v1 必须清楚提示用户风险，执行来源/摘要校验、严格 IPC、最小化凭据暴露；真正的 OS 强制隔离（Windows AppContainer、macOS Sandbox、Linux 对应机制）应分平台实现、测试后再宣称具有沙箱。

## 2. 统一插件包

扩展名 `.fiexplugin`，本质是带 manifest 的可校验归档。官方与第三方完全相同：

```text
my-plugin.fiexplugin
  manifest.json
  icon.svg
  bin/
    windows-x86_64/plugin.exe
    windows-aarch64/plugin.exe
    macos-aarch64/plugin
    macos-x86_64/plugin
    linux-x86_64-gnu/plugin
    linux-aarch64-gnu/plugin
  ui/                       # 可选的预编译静态 UI
    main/index.html
    main/assets/index.js
    main/assets/index.css
```

允许只包含一个平台的归档；不能要求每个作者一次发布所有二进制。**UI-only 包允许没有 `bin/` 与 `entrypoints`；拥有 Native 后端的插件必须声明所支持平台的完整二进制入口。**安装时按 `OS + arch + ABI` 选择入口，二进制/资源路径必须是**包内相对路径**；防 ZIP Slip、软链接逃逸、异常压缩比及恶意大包。不可通过自报入口路径执行用户系统上的任意程序。

```json
{
  "schemaVersion": 1,
  "id": "org.example.webdav",
  "name": "WebDAV",
  "version": "1.0.0",
  "pluginProtocol": { "min": "1.0", "max": "1.x" },
  "capabilities": ["filesystem", "connectionFactory", "panel"],
  "permissions": {
    "network": { "hosts": ["example.com"] },
    "secrets": true
  },
  "entrypoints": {
    "windows-x86_64": "bin/windows-x86_64/plugin.exe",
    "macos-aarch64": "bin/macos-aarch64/plugin",
    "linux-x86_64-gnu": "bin/linux-x86_64-gnu/plugin"
  },
  "ui": { "bridge": "1.0", "assetsRoot": "ui/" },
  "contributes": {
    "panels": [{ "id": "webdav.settings", "entry": "ui/main/index.html", "placement": "modal" }]
  }
}
```

示例中的 `1.x` 为**设计意图**，最终需约定精确 semver 范围语法；声明可访问的主机支持运行时新增连接域名的授权请求，不要求提前知道任意 NAS 主机地址。发布渠道元数据与来源徽标**不授予特殊内核权限**。

## 3. 安装、更新与卸载

1. Core 从本地文件或 URL 获得包，在暂存目录中检查格式、manifest/schema、API、平台/ABI、压缩路径、每个资源摘要。
2. 在有已知发布者密钥的渠道检查签名；未知发布者明确警告并要求用户确认，不能把自签名误当已验证发布者。
3. 展示插件 ID、发布者、请求的资源权限、外部网络目的地、安装目标与自动更新策略。
4. 解包到用户级插件目录（由平台标准 app data 决定，不硬编码固定 `~/.fiex`）；以临时目录+原子切换版本安装。禁止插件更新时直接覆盖正在运行的二进制。
5. 更新前完成当前任务的停机/恢复处理、配置数据库迁移与回滚点；新版本握手失败回滚上一个已知可启动版本。
6. 卸载首先取消/完成任务，移除可执行文件；连接配置、缓存、同步目录是否删除分别让用户决定。**默认不得删除用户同步目录。**

内置安装的官方插件也通过插件扫描器验证加载；打包集成不等于私有启动路径。

## 4. 权限与凭据

授权主体是 `pluginId` 与精确安装实例。Core API 权限分离：`fs.read`、`fs.write`、`mount.create`、`network`、`secrets`、`ui.contribute`、`tasks.create`；挂载数据访问还要依据用户选择的目录/范围校验。不能仅信任插件传来的路径或 `pluginId`。

- 密码/token 放入 Core SecretStore，调用 OS Keychain（macOS）、Credential Manager/DPAPI（Windows）、Secret Service 等受支持系统设施（Linux）；若安全后端不可用需显式告知并提供合理降级，而非默默写明文。
- 插件获得授权后可按范围请求凭据，并只在必要时间保留。密文/凭据引用和日志分离，错误信息脱敏。
- 网络目的地主机、授权重定向和 SMB 网络范围应在适配器实现/权限提示中明确；仅 API 声明并不能阻止有恶意行为的普通子进程直连任意主机。
- IPC 内核入口校验所有请求的进程身份、权限、mount 归属、task 归属、路径限制、大小与速率；严禁使用未认证的本地公开 TCP 控制端口。

## 5. 三个平台的实际约束

| 平台    | 关键点                                                                                                                                |
| ------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| Windows | PE 架构/ABI、受限 Named Pipe ACL、SmartScreen/代码签名策略、更新时 exe 文件锁                                                         |
| macOS   | Apple Silicon/Intel、Gatekeeper/隔离属性、签名及 notarization、宿主签名包与动态插件关系；签名验证和运行授权不能靠删除 quarantine 绕过 |
| Linux   | glibc 最低版本或 musl 策略、桌面环境与动态库依赖、可执行位、Unix socket 文件权限；可选系统沙箱机制按发行版适配                        |

在每个平台都要验证第三方安装与官方预装**同协议、同生命周期**。不要认为将二进制加入 Tauri `externalBin` 就等于实现动态插件管理：官方预装可通过构建脚本分发，但运行由 PluginHost 统一完成。

## 6. 架构与安全验收

- 不允许用户覆盖 `manifest.json` 后直接绕过版本/权限校验运行旧签名；摘要链校验所有二进制和资源。
- 进程崩溃、恶意超长帧、路径穿越、伪造 mount/task ID、重复流票据、令牌过期、无权限的数据请求必须被拒绝。
- Core 不应向插件进程传递整个用户环境（特别是敏感 token）、继承多余句柄或以 root/UAC 提权启动未经特殊审查的插件。
- iframe 必须有与 Fiex 主窗口及其他插件不同的 origin，强 CSP 和受限 iframe sandbox；不得授予 Tauri 原生 capability。Host Bridge 采用专用 MessageChannel，按 iframe 实例与授权上下文绑定并限制请求频率。详见 [11 UI SDK](11-UI-SDK.md)。
- 所有用户数据删除均遵循显式用户确认，特别是取消离线钉选、取消双向同步、卸载插件。
