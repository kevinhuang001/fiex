# 03 · Native IPC 全量方法目录与目标协议

> 本文列举插件系统内所有 `plugin.*`、`vfs.*`、`connection.*`、`fs.*`、`host.*` 方法及动态注册命令。API 列表是新设计的 **目标实现清单**；代码落地时须在 schema 中逐项定义 params/result/error/授权和契约测试。**不将 Tauri 内部 command 当作插件可调用的外部 IPC。**

## 1. Envelope、Session 与错误

```json
{"jsonrpc":"2.0","id":42,"method":"host.fs.openRead","params":{"path":"notes.txt","authorization":{"operationId":13}}}
```
```json
{"jsonrpc":"2.0","id":42,"result":{"handle":"opaque","size":12345}}
```
```json
{"jsonrpc":"2.0","id":42,"error":{"code":-32003,"message":"PermissionDenied","data":{"reason":"path_not_selected"}}}
```

目标 Native 版本 `3.0`，标准传输仍为 `Content-Length` framed JSON-RPC 2.0，单帧 4 MiB。Core 维护关联 operation/task 的密钥和进程身份；SDK 自动附加 `authorization`。文档不要求开发者手工构造授权对象。推荐稳定错误码：`-32601` method_not_found、`-32602` invalid_params、`-32603` internal、`-32800` cancelled、`-32001` not_found、`-32002` conflict、`-32003` permission_denied、`-32004` unavailable、`-32005` deadline_exceeded。详细数据只能包含非敏感结构化字段。

```mermaid
sequenceDiagram
  participant C as Core RPC dispatcher
  participant P as Native SDK
  C->>P: plugin.initialize(protocolVersion,hostVersion)
  P-->>C: {pluginId,protocolVersion,ready:true}
  par 并发请求
    C->>P: command A (id:100, scope A)
    C->>P: command B (id:101, scope B)
    P->>C: host.fs.openRead(scope B, id:90)
    C-->>P: result(handle B)
    P-->>C: response B (id:101)
    P-->>C: response A (id:100)
  end
  C->>P: plugin.shutdown
  P-->>C: result:null
```

## 2. Core → Plugin：系统/Provider 完整目录

| Method | Params（关键字段） | Result | 用途/约束 |
|---|---|---|---|
| `plugin.initialize` | `protocolVersion,hostVersion` | `pluginId,protocolVersion,ready` | 启动协商，必须匹配安装包身份 |
| `plugin.ping` | 空 | `pong/ok` | 存活检测 |
| `plugin.shutdown` | 空 | `null` | 优雅退出、在最后期限强杀 |
| `plugin.cancel` | `taskId` | notification | 协作取消任务 |
| `connection.validate` | `connection` | `{valid:true,...}` | 验证连接配置；不得泄露 Secret |
| `fs.list` | `connection,directoryId?,cursor?,limit?` | `entries,nextCursor,snapshotToken?,identityStability?` | Connection 远端目录分页 |
| `fs.stat` | `connection,nodeId` | `revision,size,kind?,modified?` | 获取一致性基线 |
| `fs.readSmall` | `connection,nodeId,offset,maxBytes` | `base64,eof` | 小数据预览，严格限长 |
| `fs.openRead` | `connection,nodeId,ifRevision?` | `streamId,size?,revision?` | 建立读取会话并绑定文件版本 |
| `fs.readStream` | `streamId,offset,maxBytes` | `base64,bytesRead?,eof` | 有界分块，必须防跨版本拼接 |
| `fs.closeRead` | `streamId` | `null` | 无论成功、取消、错误均清理 |
| `fs.writeFile` | `connection,nodeId,source,size,ifRevision` | `revision,size?` | 单一远端写回入口，Core 仅授权指定快照 |
| `vfs.list` | `contributionId,viewContext,nodeId?,cursor?,limit?` | `entries,nextCursor,warnings?` | 显式 VFS 目录列表 |
| `vfs.stat` | `contributionId,viewContext,nodeId` | `Entry` | 虚拟节点信息 |
| `vfs.readSmall` | `contributionId,viewContext,nodeId,maxBytes` | `base64` | 小文件读取/临时导出 |
| `vfs.search` | `contributionId,viewContext,nodeId?,query,limit?,hidden?,ignores?` | `entries` | 在只读 VFS 上检索 |
| `<custom command>` | 由 contribution 的 `command/args` 和操作上下文决定 | JSON | toolbar、context、search、preview、info、hooks、decorations 的自定义处理 |

Provider 清单中的 `nodeId` 与 `revision` 均不应当被 Core 当作磁盘路径；所有 `Entry.path` 必须由 Core 根据 mountId 生成。扩展点动态命令原则上不能冒充 `plugin.`、`host.`、`vfs.`、`fs.`、`connection.` 保留前缀。

### Connection 文件契约

```json
{
  "entries":[
    {"nodeId":"opaque-81","name":"a.txt","kind":"file","size":12,"modified":1800000000,"revision":"r42"}
  ],
  "nextCursor":null,
  "identityStability":"stable"
}
```

`identityStability: "stable"|"path"` 表示改名后的 ID 稳定性，**不表示内容一致性**。目录分页必须有单调前进规则；若远端分页顺序变化、缺少快照功能，Core 不得把“分页结束”当成整目录强一致快照。

## 3. Plugin → Core：文件 Broker 全部方法

| IPC method | Params | Result | 必要权限 |
|---|---|---|---|
| `host.fs.list` | `path` | `Entry[]` | `folders.read` + scope |
| `host.fs.stat` | `path` | `Entry` | `folders.read` 或 `files.read` + scope |
| `host.fs.readChunk` | `path,offset,length` | `base64,size,eof` | `files.read` + scope |
| `host.fs.read` | `path,offset,length` | `buffer,size,eof` | `files.read` + scope；内部低层 |
| `host.fs.openRead` | `path` | `handle,size` | `files.read` + scope |
| `host.fs.readOpen` | `handle,offset,length,buffer?` | `size,eof,base64?` | 属主 Handle + 文件权限 |
| `host.fs.closeRead` | `handle` | `null` | Handle 所属 operation/task |
| `host.fs.create` | `path` | `Entry` | `folders.create` + scope |
| `host.fs.mkdir` | `path` | `Entry` | `folders.create` + scope |
| `host.fs.beginWrite` | `path,create` | `handle` | `files.write`，create 时还需 `folders.create` |
| `host.fs.writeChunk` | `handle,base64` | `null` | `files.write` + Handle owner |
| `host.fs.writeBuffer` | `handle,buffer,size` | `null` | `files.write` + Handle owner |
| `host.fs.seekWrite` | `handle,kind,offset` | `u64 offset` | `files.write` + Handle owner |
| `host.fs.commit` | `handle` | `Entry/confirmation` | `files.write` + 冲突复检 |
| `host.fs.abort` | `handle` | `null` | Handle owner |
| `host.fs.write` | `path,buffer,mode` | 状态 | `files.write`；低层旧风格接口计划内收敛 |
| `host.fs.remove` | `path` | `null` | `folders.delete` + scope |
| `host.fs.rename` | `from,to` | `null` | `folders.rename` + 双路径 scope |

统一处理文件目录对象的 race/symlink、原子提交、临时权限与文件大小。`create` 权限判定不能通过 `files.write` 单独绕过，`rename` 要分别授权两个操作数。

## 4. Plugin → Core：网络、共享缓冲、进程

| IPC method | Params | Result | 权限/说明 |
|---|---|---|---|
| `host.net.http.open` | `method,url,headers?,bodyBase64?,bodyReadHandle?,timeoutMs?,allowInvalidCerts?` | `stream,status,headers` | network + 每跳域名/IP 校验 |
| `host.net.http.read` | `stream,size` | `base64,size,eof` | 当前响应属主 |
| `host.net.http.close` | `stream` | `null` | 释放响应 |
| `host.net.http` | `method,url,headers?,bodyBuffer?` | `status,headers,bodyBase64?/bodyBuffer?` | 有界旧式单次 HTTP；目标收敛到 open/read/close |
| `host.net.tcp.open` | `host,port` | `stream,peerAddress` | network + 审批目标 |
| `host.net.tcp.readBytes` | `stream,size` | `base64,size,eof` | 标准 TCP SDK 使用 |
| `host.net.tcp.writeBytes` | `stream,base64` | `size` | 标准 TCP SDK 使用 |
| `host.net.tcp.read` | `stream,size` | `buffer,size,eof` | 共享窗口低层 |
| `host.net.tcp.write` | `stream,buffer` | `size` | 共享窗口低层 |
| `host.net.tcp.close` | `stream` | `null` | 清理 |
| `host.buffer.create` | `size,writable` | `descriptor` | owner、配额、上限 |
| `host.buffer.map` | `id` | `descriptor` | 相同 owner |
| `host.buffer.release` | `id` | `null` | 映射释放 |
| `host.process.run` | `command,args,cwd?,stdin?` | `code,stdout/stderr,truncated` | commands allowlist，高风险 |
| `host.exec` | `command,args...` | 结构化执行结果 | 低层执行入口；建议最终统一至 process.run |

`allowInvalidCerts` 不应仅靠插件自行设置；设计要求连接实例保存经用户确认的证书例外，并且限定具体主机。避免 DNS 重绑定与内网 SSRF 的建议：域名匹配 + 解析 IP 校验 + 连接实际地址验证；HTTP 禁止跨目标泄露 Authorization/Cookie，重定向逐跳验证。

## 5. Plugin → Core：视图、任务、数据、诊断

| IPC method | Params | Result | 说明 |
|---|---|---|---|
| `host.vfs.openView` | `contributionId,resourceRoot,viewContext` | `fiex://mount/.../` | 只允许显式 vfs owner + 已选资源 |
| `host.preview.create` | `name,base64` | `path` | 限额临时预览；目标升级为 opaque resource ref |
| `host.task.start` | `taskId,label?,internal?` | `null` | 从本次操作衍生 TaskScope |
| `host.task.progress` | `taskId,done,total?` | `null` | 更新活动中心 |
| `host.task.finish` | `taskId,status,result?,error?` | `null` | 回收 TaskScope 和句柄 |
| `host.task.cancel` | `taskId` | `null` | 请求取消，应双向闭环 |
| `host.storage.get` | `scope,key` | JSON 或 null | 隔离插件 KV |
| `host.storage.set` | `scope,key,value` | `null` | 宿主原子持久化 |
| `host.storage.delete` | `scope,key` | `null` | 删除 KV |
| `host.storage.keys` | `scope` | `string[]` | 只列自己命名空间 |
| `host.secret.get` | `scope,key` | `string` | OS 凭据代理 |
| `host.secret.set` | `scope,key,value` | `null` | secret 不进普通日志 |
| `host.secret.delete` | `scope,key` | `null` | 删除 Secret |
| `host.secret.available` | 空 | `bool` | 是否有可用 Keyring |
| `host.error.report` | `code?,message,detail?` | `null` | 限流记录、去标识化 |

`host.*` 方法的权限不能由用户提供的同名字段提升；Core 必须验证 operation owner、handle owner、已安装并已授权的插件进程。低层方法可以在最终 ABI 继续存在，**但仅作为 SDK 内部机制，不应承诺它们是推荐的第三方开发入口**。

## 6. 动态命令合同

| 来源 | 调用方法名称 | 推荐参数/结果 |
|---|---|---|
| Toolbar/ContextMenu | `<plugin>.<action>` | `{selection,directory,args}` → JSON |
| Search | 声明 `command`，默认 `search.query` | `{query,root,hidden,ignores,limit}` → `{entries}` |
| Info | 声明 `command` | `{path}` → `{rows:[{label,value}]}` |
| Hooks | 声明 `command` | `{event,path}` → `{action:"pass"|"consume"}` |
| Dynamic Decorations | 声明 `method` | `{directory,entries}` → 仅装饰，无文件写权限 |
| Preview | UI `plugin.invoke` | SDK 方法名与参数自定义；遵循 UI Bridge 限额 |

动态命令不属于 `host.*` 白名单，仅能在其贡献点明确注册的触发流程下执行；Core 对所有动态参数进行数据 schema 验证，并绑定最小资源授权。

## 7. 目标规范落地清单

- 为每个上述方法生成 JSON Schema，固定 required/default/bounds、result shape、超时和各平台错误映射；
- 进程握手、并发请求乱序、多级请求回入、错 ID、超时、取消、关闭进程的恢复都需契约测试；
- `fs.writeFile` 在 WebDAV 强 ETag 下断言原子前置条件，其他 Provider 必须准确报告保障级别，由 Core 按最保守策略执行；
- 所有 `host.fs.*` 都测试未选择、其他文件、其他 task、重复提交、符号链接/重命名绕出目录与大于 5 GiB 稀疏文件；
- 有界 buffer 创建、释放与错误降级不应导致数据静默损坏。
