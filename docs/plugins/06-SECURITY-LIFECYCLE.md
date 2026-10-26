# 06 · 权限、安全与生命周期

## 信任边界

可信：

- Core；
- `plugin_permission_policy`；
- `plugin_permission_prompt`（Core 统一授权弹窗）；
- Host Broker；
- PluginRuntime；
- ContentHost / MountRegistry；
- 系统 Keyring 代理。

不可信：

- native 插件进程；
- 插件 iframe；
- 插件返回的路径、HTML、导航目标和网络响应。

native 插件是独立进程，但当前不是完整的 OS 沙箱：插件必须遵守“不直接使用 `std::fs` / `std::net` / 子进程”的合同，平台级强制隔离仍不是所有系统都已实现。

## 三层权限模型

代码：`src-tauri/src/plugin_permission_policy.rs`

```text
Manifest 声明
  → 用户 Permission 状态（approved / enabled / granted）
  → Operation Scope（本次操作的路径、主机、命令）
  → PermissionPolicy::check / domain_check / command_check
  → Host RPC / Broker 执行
```

三层缺一不可：

| Manifest 声明 | 用户设置         | 当前 Scope | Core 行为                  |
| ------------- | ---------------- | ---------- | -------------------------- |
| 有            | 已允许           | 范围内     | 直接执行                   |
| 有            | 用户手动关闭     | 范围内     | 弹窗重新申请               |
| 有            | 已允许           | 范围外     | 拒绝，不弹权限框           |
| 没有          | 任意             | 任意       | 直接拒绝                   |
| 有            | 用户拒绝重新授权 | 任意       | 拒绝本次操作（不保存允许） |

`PermissionPolicy::check` 只回答 Manifest + 用户状态两层，返回：

- `Allowed`：已声明、已批准、插件启用、该权限已允许；
- `Prompt`：已声明且可以申请，但用户当前关闭了该权限；
- `Denied(reason)`：未声明、安装未批准、插件停用，或 Manifest 声明超出了已批准的安装快照。

实现约束：

- 未声明的权限不能被运行时申请获得。插件必须发布声明了新权限的版本，并由用户在安装/升级流程中批准。
- `plugin_permissions` 在安装批准时保存 Manifest 声明快照（`approvedDeclared`）；运行时授权不能超过该快照。
- 网络检查统一调用 `domain_check`，命令检查统一调用 `command_check`；连接保存/打开仍使用 `require_all`。
- 用户关闭单个权限不会停用插件。插件停用、关闭窗口或进程退出会取消尚未回答的授权请求。
- `plugin_permissions` 只负责 Grant 存储、激活、撤销和域名/命令集合，不负责业务分支。

## 运行时授权弹窗

代码：`src-tauri/src/plugin_permission_prompt.rs`、`src/components/plugins/PluginPermissionHost.tsx`

- 弹窗只由 Core 创建和渲染；插件只能发起 `host.*` 调用并等待结果，不能伪造系统授权框。
- Core 先校验 Operation Scope，再检查权限状态。范围外请求直接失败，不进入弹窗。
- 同一个插件、同一权限、同一目标（网络主机或命令）的并发请求共享一个弹窗：
  十个读取并发只问一次；用户拒绝后，本批全部失败。
- 用户允许后，Core 先持久化授权，再唤醒等待中的操作；之后不再重复弹窗，直到用户从插件管理中关闭。
- 用户拒绝只影响本次操作，并进入短暂冷却期，避免插件立即重试造成弹窗骚扰。
- 授权弹窗超时、插件停止或应用退出都按拒绝处理。
- 已经取得的 read/write/network handle 在撤权后不再重新弹窗，后续调用直接失败；重新发起 open 才会重新申请。

## Scope

权限通过不等于资源通过。文件操作还受 `plugin_fs_access::Scope` 限制：

- `metadata`：只读属性；
- `files.read`：读取内容；
- `files.write/create/delete/rename`：分别对应真实对象和操作；
- `folders.read/create/delete/rename`：目录操作；
- 选中目录只授权该目录及其允许的子树；
- 符号链接、重命名、VFS revision 变化后重新校验。
- Scope 校验先于权限弹窗；未选中的路径不会出现在授权请求里。

网络与命令的 Operation Scope 分别是本次请求的精确主机和 Manifest `commands.allow` 中的精确命令名。

## 网络

- HTTP/TCP 都只能通过 SDK；
- Core 校验 host/domain、端口和 DNS 后的目标；
- `network.all` 允许任意主机，但激活界面会标红；
- HTTP 重定向逐跳检查；
- 插件在 SDK TCP 上自行做 TLS 时，Core 仍负责目标授权。

## 命令

`commands` 是独立高风险权限；只允许 Manifest 中 `commands.allow` 列出的程序名。命令在插件沙箱外以当前用户身份运行。

## 终端

SSH 插件通过 `host.terminal.*` 使用 Core 的 xterm 会话：

- session 由 Core 创建并绑定 plugin owner；
- 只有拥有该 Session 的插件能读写；
- 前端输入进入 Core 队列，插件轮询读取；
- 关闭标签会结束 Session。

## 激活与生命周期

- 安装/升级校验 Manifest、签名、路径和新权限；
- 安装批准会记录声明快照；声明变化时必须重新激活；
- 激活界面按文件与文件夹、网络与域名、命令、其他能力分组；用户可逐项取消勾选（默认全选），只保存所选权限和精确域名/命令，未选中的能力在后续操作需要时再次询问；
- `enabled` 表示插件可以运行，某个权限关闭不会自动停用插件；
- 激活成功会立即启动原生后端；主题等声明式插件不启动进程；启动失败写入插件错误中心，不回滚已成功的激活；
- 插件停用、卸载或重启后旧 Session/Handle 失效；单项撤权会立即影响后续操作和提交；
- 插件错误写入错误中心，不记录 secret 或完整文件内容；
- 当前 hooks 合同固定为 `hooks.invoke`，但文件生命周期接线尚未全部完成，不能把它当成已生效的拦截点。
