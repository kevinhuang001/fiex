# 04 · Native 插件 IPC 协议 2.0

> 状态：随代码实现，实验性接口。协议版本为 `2.0`；可以破坏性修改。

## 1. 传输

- 编码：UTF-8 JSON。
- 帧：`Content-Length: <bytes>\r\n\r\n<json>`。
- 单帧上限 4 MiB；stdout 只用于协议，诊断写 stderr。
- 进程生命周期由 Core 管理；stdin/stdout 都是控制通道。

## 2. 握手

Core 在启动插件后发送：

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "plugin.initialize",
  "params": { "protocolVersion": "2.0", "hostVersion": "0.3.0" }
}
```

插件返回：

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": { "pluginId": "org.fiex.git", "protocolVersion": "2.0", "ready": true }
}
```

版本不匹配、身份不匹配或握手失败时 Core 终止进程。

## 3. 方法与命名空间

### 宿主 → 插件

| 方法                                 | 说明                                              |
| ------------------------------------ | ------------------------------------------------- |
| `plugin.initialize`                  | 握手                                              |
| `plugin.ping`                        | 存活检查                                          |
| `plugin.shutdown`                    | 正常退出                                          |
| `vfs.list`                           | 插件自有虚拟文件系统分页目录                      |
| `vfs.stat`                           | 虚拟节点元数据                                    |
| `vfs.readSmall`                      | 小文件预览，返回 Base64                           |
| `vfs.search`                         | 虚拟文件系统内搜索                                |
| `connection.validate`                | 验证远程连接配置                                  |
| `fs.list`                            | 远程目录分页                                      |
| `fs.readSmall`                       | 远程小文件预览                                    |
| `fs.openRead`                        | 打开远程读取流，返回 `streamId`                   |
| `fs.readStream`                      | 按 offset 读取流块，返回 `{base64,bytesRead,eof}` |
| `fs.closeRead`                       | 关闭读取流                                        |
| `git.*` / `archive.*` / `checksum.*` | 插件自定义命令                                    |

`vfs.*` 用于显式 `contributions.vfs` 的 Core View；`connections` 通过现有远端适配器隐式具备内部 VFS。Core 用 `fiex://mount/<id>/` 查找已绑定 owner。`host.vfs.openView` 校验当前 IPC 身份、contribution 和选中目录，返回 Core URI；插件不得声明 scheme。详情见 [VFS](02-VFS.md)。

`fs.*` 只用于远程只读连接。Core 负责暂存、目标文件和缓存；插件不接收宿主绝对路径。

### 插件 → 宿主

| 方法                                   | 说明                          |
| -------------------------------------- | ----------------------------- |
| `host.storage.get/set/delete/keys`     | 插件私有 KV，按 pluginId 隔离 |
| `host.secret.get/set/delete/available` | 系统凭据代理                  |

这些方法绑定到已认证的进程身份，请求体中的 pluginId 不被信任。插件私有存储和凭据不需要额外权限；凭据不会回显到 iframe。

## 4. 分页与流

- `vfs.list` / `fs.list` 返回 `{entries,nextCursor,snapshotToken?,warnings?}`。
- 条目字段：`nodeId,name,kind,size,modified`。
- Core 每次请求最多 500 条，拒绝游标不前进或条目过多。
- `fs.openRead` 返回 `{streamId,size?}`。`fs.readStream` 每次最多 256 KiB，必须返回 `eof`。
- 传输上限 16 GiB；Core 在取消或失败时删除暂存文件。
- 取消：Core 在块之间检查取消标志并关闭流；长任务级 `taskId` 协议尚未实现。

## 5. 错误

JSON-RPC 标准错误码：

- `-32601` 方法不存在
- `-32602` 参数无效
- `-32603` 插件内部错误

Core 把插件错误包装为稳定文本，不把原始 stdout/stderr 注入 UI。协议版本不匹配、身份不匹配、帧过大、响应 ID 不匹配都会终止进程。
