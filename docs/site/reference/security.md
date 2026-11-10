# 插件权限与安全边界

Fiex 插件是独立进程。插件不能因为安装成功就自动获得文件、网络或命令权限；它必须在 manifest 里声明用途，用户在启用时批准，Core 在每次调用时再校验具体资源范围。

## 每次调用经过的检查

| 层次               | 核对内容                                                         | 作用                         |
| ------------------ | ---------------------------------------------------------------- | ---------------------------- |
| Manifest 声明      | 插件是否申请该能力；命令/网络目标是否在声明内                    | 设定插件允许申请的上限       |
| 用户 grant         | 用户是否批准且没有撤销该权限                                     | 控制哪些声明可用于当前账户   |
| Operation Scope    | 当前操作选择了哪些文件、目录、连接和目标                         | 防止跨操作借用资源           |
| Core/Provider 校验 | 引用是否属于当前插件/mount、能力是否有效、路径/handle 是否仍存活 | 防止伪造资源和复用过期句柄   |
| OS process sandbox | 子进程系统级文件/网络/进程边界                                   | 为 Core 授权检查增加平台隔离 |

这几层互不替代。一个权限已获准的插件仍不能通过 Core API 任意访问 scope 外文件；Operation Scope 也不能给 manifest 未声明的能力开绿灯。

## 文件与目录权限

`metadata` 表示受控的属性读取；它不包含目录枚举或文件字节读取。`folders.read` 用于列目录，`files.read` 用于读文件内容。创建、写入、删除、重命名等操作分别有权限项。Core 通过 `ResourceRef` 传递对象身份，不把宿主绝对路径作为插件 API。

ResourceRef 是与当前操作或任务关联的资源引用，不是跨会话凭据。文件流、VFS 流和写入事务在关闭、commit/abort 或授权结束时回收。

## 网络

插件 HTTP/TCP 使用 Core 提供的代理 API，访问按 host 校验。Manifest 可列出精确的域名 allowlist；`network.all` 是更广泛的任意主机能力。对用户授权一个域名不会自动授权另一个 host。

插件应使用 SDK HTTP/TCP/OAuth API，而不是内置自有网络库或绕过 Core 的系统 socket。重定向和后续连接仍由 Core 的网络策略检查。

## 外部命令

`commands` 权限允许插件请求运行 manifest 明确列出的 executable。Core 逐项检查命令声明和用户批准，并以参数数组调用进程；不能将任意命令行字符串当 shell 脚本执行。

批准的命令以当前用户身份运行，可能访问 SDK file scope 以外的文件、网络或启动其他进程。因此命令权限风险高于 `ctx.files()`，应只在功能确实需要时申请。

## 连接凭据与本地存储

插件持久化存储适合保存非敏感配置和同步状态。密码、OAuth token 和私钥应使用 SDK Secrets/OAuth 接口或系统凭据管理，不要写入普通插件存储、日志、UI 页面或 manifest。

## 插件包身份与信任

签名用于校验包签名者身份和包完整性。它不证明插件代码安全，不替用户批准权限，也不让官方插件获得额外能力。启用前仍要看权限与来源；运行中仍由 Core 逐次授权。

## 进程隔离

Core 为 native 插件配置平台沙箱（macOS Seatbelt、Windows AppContainer/Job Object、Linux Landlock/seccomp 等）。沙箱无法按当前平台要求建立时，Core 不应降级为未受控插件进程继续运行。具体系统版本能力和限制以当前运行平台实现为准。

## 报告安全问题

请通过 [GitHub Security Advisories](https://github.com/kevinhuang001/fiex/security/advisories) 报告安全问题。提交公开问题前，移除访问令牌、账户信息、私人文件内容和敏感完整路径。
