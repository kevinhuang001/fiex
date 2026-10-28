# 05 · Connection、VFS 与同步

## Connection

一个插件最多声明一个连接入口：

```json
"connections": {
  "title": "WebDAV",
  "fields": [
    { "id": "url", "type": "url", "required": true },
    { "id": "password", "type": "secret" }
  ]
}
```

保存连接时：

1. Core 校验 Manifest 字段；
2. 检查插件已启用、已批准、权限完整；
3. 把目标 host 加入该插件的网络白名单；
4. 调用 `connection.validate`；
5. secret 字段写入系统钥匙串，普通字段写入连接记录。

连接 `kind` 就是插件 id，不再有单独的 factory id。

## 导航

`connection.open` / `connection.create` 只能返回：

```json
{ "kind": "vfs", "mountId": "m1", "nodeRef": { "mountId": "m1", "nodeId": "root" } }
```

```json
{ "kind": "pluginView", "view": { "source": "package", "entry": "ui/home.html" }, "state": {} }
```

- `vfs` 进入 Core Explorer/VFS 视图；
- `pluginView` 完全替换右侧主内容区，保留左侧导航和全局标签；
- 不能返回弹窗、任意外部 URL 或任意宿主路径；
- `pluginView` 可带 `tabTitle` 作为标签标题。

## VFS

```json
"vfs": { "title": "WebDAV", "readOnly": false }
```

Core 分配 `mountId`，插件只使用 Core 传入的节点/资源身份，不自己构造 URI。

当前 VFS Provider 已实现的固定方法：

- `vfs.list`
- `vfs.stat`
- `vfs.openRead` / `vfs.readStream` / `vfs.closeRead`
- `vfs.search`

独立视图与连接共用 `vfs.openRead/readStream/closeRead` 和 `vfs.beginWrite/writeStream/commit/abort`；旧的小文件读取和快照上传接口已删除。

## 同步与缓存

- 远端文件由 Core 下载缓存后再交给本地打开；
- 缓存、离线固定和冲突状态由 Core 管理；
- Metadata 缓存不携带授权；权限仍按插件和当前 Scope 判断；
- 远端 revision 变化、连接关闭或插件撤权后，旧引用失效。

## SSH 特例

`org.fiex.ssh` 是连接插件，但导航目标是终端：

```text
connection.open
  → Core 识别 org.fiex.ssh
  → kind = terminal
  → xterm.js full-view tab
```

TCP 由 `host.net.tcp.open` 建立，SSH 握手和 PTY 由官方 SSH 插件完成，终端字节通过 `host.terminal.*` 与 Core 的 xterm 会话交换。
