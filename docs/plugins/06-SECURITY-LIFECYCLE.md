# 06 · 权限、安全与生命周期

## 信任边界

可信：

- Core；
- `plugin_permission_policy`；
- Host Broker；
- PluginRuntime；
- ContentHost / MountRegistry；
- 系统 Keyring 代理。

不可信：

- native 插件进程；
- 插件 iframe；
- 插件返回的路径、HTML、导航目标和网络响应。

native 插件是独立进程，但当前不是完整的 OS 沙箱：插件必须遵守“不直接使用 `std::fs` / `std::net` / 子进程”的合同，平台级强制隔离仍不是所有系统都已实现。

## PermissionPolicy

代码：`src-tauri/src/plugin_permission_policy.rs`

```text
Manifest permissions
  → 用户 Grant（plugin_permissions 存储）
  → PermissionPolicy::allowed / require / require_all
  → Host RPC / Broker
  → Operation Scope
```

`PermissionPolicy` 是权限决定的统一入口：

- 未知能力 → deny；
- 未声明 → deny；
- 未批准、插件停用、策略不可读 → deny；
- 网络域名检查统一调用 `domain_allowed`；
- 命令检查统一调用 `command_allowed`；
- 连接保存/打开使用 `require_all`。

`plugin_permissions` 继续负责 Grant 存储、激活、撤销和域名/命令集合，不负责业务分支。

## Scope

权限通过不等于资源通过。文件操作还受 `plugin_fs_access::Scope` 限制：

- `metadata`：只读属性；
- `files.read`：读取内容；
- `files.write/create/delete/rename`：分别对应真实对象和操作；
- `folders.read/create/delete/rename`：目录操作；
- 选中目录只授权该目录及其允许的子树；
- 符号链接、重命名、VFS revision 变化后重新校验。

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
- 用户逐项批准权限；`enabled` 只是申请；
- 插件停用、撤权、卸载或重启后旧 Session/Handle 失效；
- 插件错误写入错误中心，不记录 secret 或完整文件内容；
- 当前 hooks 合同固定为 `hooks.invoke`，但文件生命周期接线尚未全部完成，不能把它当成已生效的拦截点。
