# 13 · Plugin Storage, Secrets and OAuth contract (draft)

[索引](README.md) · [IPC](04-IPC-PROTOCOL.md) · [SDK](08-SDK.md)

> **状态：初版已实现（非冻结协议）。** `host.storage.*` 与 `host.secret.*` 已可用；完整 OAuth 回调服务、存储 schema 迁移和 Windows 沙箱仍未实现。

## 1. 职责

- Fiex Core **MUST** 独占普通持久化 KV、设置存储和 SecretStore 的管理权，并按照真实插件进程身份或已验证 iframe session 进行隔离；不能信任请求体中的 pluginId。
- 插件 **MUST NOT** 通过将敏感 token、密码、OAuth authorization code 或 PKCE verifier 塞到普通 KV 来绕开凭据系统。缓存/同步数据库另由 Core 管理，不属于插件任意 KV。
- 持久状态不能依赖 Node.js、浏览器 localStorage、插件进程当前工作目录，或插件安装路径。插件升级时默认保留数据；卸载必须明确提供“保留用户数据/删除用户数据”的选择。

## 2. Scope 和 Key

所有键空间按 `(pluginId,scope,key)` 隔离。`pluginId` 由可信安装记录绑定；`scope` 是 `global`、`connection-<opaque-id>`、`account-<opaque-id>` 或其他被宿主授权的命名空间，不能包含路径分隔符。

| 接口                                   | 用途                                                    | 存储                                                               |
| -------------------------------------- | ------------------------------------------------------- | ------------------------------------------------------------------ |
| `host.storage.get/set/delete/keys`     | 公开配置、游标、用户规则、短期状态等 JSON               | Fiex app data 下的插件专有 KV                                      |
| `host.secret.get/set/delete/available` | WebDAV/SMB 密码、OAuth refresh/access token、API Key    | macOS Keychain / Windows Credential Manager / Linux Secret Service |
| `host.settings.*`（后续约定）          | 按 manifest settings schema 的设置校验、迁移、主题/展示 | Host 的设置 API                                                    |
| `host.auth.*`（后续实现）              | OAuth PKCE 登录、重定向回调、Token 生命周期             | Host + 服务商协议                                                  |

初版普通存储每个 value 最多 **1 MiB**，一个 scope 合计最多 **16 MiB**。二进制大数据不应放入 JSON KV，需申请受限文件存储 API；缓存数据走 CacheManager。

`host.storage.*` 接受 JSON 兼容数据类型，`null` 和键不存在不可混为一谈：`get` 未找到可返回 `null`，正式 v1 **必须**引入明确的 `found: boolean` 以消除歧义，并为 CAS/事务操作添加版本号。

### 2.1 Native 调用

```rust
// Native SDK：每个请求拿到独立上下文
fn handle(ctx: fiex_plugin_sdk::RequestContext) -> Result<serde_json::Value, fiex_plugin_sdk::RpcError> {
    ctx.storage_set("account-account1", "lastSyncCursor", serde_json::json!("opaque-token"))?;
    ctx.secret_set("account-account1", "oauthRefreshToken", "<token>")?;
    Ok(serde_json::Value::Null)
}
```

插件程序不能通过 host API 读取其他插件的任何 scope。`host.secret.get` 永远不对任意 iframe 暴露；iframe 可以通过权限受限命令请求其 Native 后端完成授权相关操作。

### 2.2 UI 设置集成

`manifest.contributions.settings` 声明字段，Fiex 设置 → 插件中的宿主面板渲染与持久化。非敏感字段使用 `plugin_storage_*`，`secret` 字段使用 Host SecretStore，仅允许写入/清除或显示“已配置”，**不回显秘密**。

### 设置字段类型

| type       | 存储值           | 渲染                                                           |
| ---------- | ---------------- | -------------------------------------------------------------- |
| `text`     | string           | 单行输入                                                       |
| `url`      | string           | 单行输入                                                       |
| `number`   | number           | 数字输入                                                       |
| `checkbox` | boolean          | 开关                                                           |
| `secret`   | SecretStore      | 密码输入；留空表示不修改                                       |
| `list`     | string[]（JSON） | 行编辑器：每行一个值，可添加/删除；`defaultValue` 为字符串数组 |

`list` 用于忽略列表、镜像地址这类无序集合。读取时兼容旧的逗号/换行分隔字符串，保存后统一为数组。

工具栏的插件菜单、侧栏连接创建入口与设置插件列表都必须读取同一份验证后的 manifest，不得硬编码官方插件名。

## 3. OAuth Authorization Code + PKCE

建议初版公共 Host Auth Broker：

1. 插件声明授权域名、callback 模式、所需 scope 及其 OAuth 公共 Client ID；不能分发 Client Secret 作为“秘密保护”，桌面应用是公共客户端。
2. 插件调用 `host.auth.start`（拟议）；Core 生成随机 `state`、高熵 `code_verifier`、`code_challenge(S256)`，绑定 `pluginId+accountId+windowId` 并设置短期过期。
3. Core 打开**系统浏览器**，监听临时的 `127.0.0.1` 随机端口 loopback callback 或经平台注册的 URI handler；不能把授权页塞入不可信插件 iframe。
4. 回调校验 `state`、来源及重定向 URI；一次性消费会话，避免 replay。`authorization code` 仅在内存中短暂保存。
5. 服务商 Token Exchange 由插件协议适配器执行（或经 Host 的受限 HTTP 代理），获得的 access/refresh token 通过 `host.secret.set` 持久化到对应 account scope。
6. Token 过期时间、授权账户 ID、游标可放普通 KV；**Token 字符串绝不进入 JSON KV**。Refresh token rotation 必须先提交新凭据成功，再替换旧有效状态；可重试且避免并发刷新。
7. 退出登录时撤销服务商 grant（如支持），删除 Keychain 项和该账户对应的本地缓存/状态引用，但默认保留用户同步目录并提示确认。

真正的 OAuth Broker 需要明确定义：回调监听器生命周期、CSRF、PKCE、取消/超时、并行账号登录、浏览器关闭、授权失败、权限升级、Token 刷新竞争以及系统钥匙串不可用场景；**未实现前不得在 UI 宣称“OAuth 已集成”**。

## 4. 版本与迁移

- 插件包版本 ≠ 数据 schema 版本。需要每个插件 `storageSchemaVersion`、增量 migration（原子应用）、失败回滚。
- 更新不清除数据；降级遇到更高 schema 时应禁止写入，避免破坏旧数据。
- 应有 `compareAndSet`/多键事务解决并发写入、跨进程竞态；目前只有宿主串行写 JSON 文档，暂不保证跨 OS 进程原子事务。
- 当前旧 `~/.fiex/connections.json` 的明文密码在可用的 SecretStore 中迁移后，重写时不再序列化。SecretStore 不可用时不得静默保存新明文，也不能声称旧磁盘副本已物理擦除。

## 5. 安全与验收

- [ ] 不同插件、不同账号、不同连接 scope 不可互读；伪造插件 ID 被忽略。
- [ ] SecretStore 不可用时报错，**不得**退回明文 JSON。
- [ ] 插件卸载、禁用、升级、重启、异常退出保留/清理行为明确。
- [ ] 二进制/超大值、路径穿越、无权限 host 请求全部拒绝。
- [ ] OAuth 失败、取消、重放、state mismatch、refresh rotation 可重复测试。
- [ ] Windows、macOS、Linux 的实际 Keychain/Secret Service 集成验证，包含钥匙串锁定状态。
