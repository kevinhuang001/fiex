# 插件架构实现进度（开发分支）

> **此文档对应 `feat/plugin-sdk-extraction` 开发分支，非正式发布说明。** Draft 0.1 协议在实装中允许修改；SDK 不保证 1.0 兼容性。

## 本阶段已写入代码

- `sdk/rust/`：独立可发布的 Rust Native SDK 初版，Content-Length JSON-RPC framing、错误和服务循环、单元测试。
- `sdk/ui/`：浏览器端 `@fiex/ui-sdk` 原型及宿主 MessageChannel 桥接口，支持 iframe 握手、请求/响应、事件、取消和类型。
- `src-tauri/src/plugin_host.rs`：长驻子进程客户端，按进程序列化请求、初始化身份校验、断开后清理。
- 三个独立二进制入口 `fiex-plugin-webdav`、`fiex-plugin-smb`、`fiex-plugin-git`。**当前进程仍复用旧 `fiex` library 里的网络/Git 逻辑，是过渡适配，不属于最终插件解耦**。
- `remote::provider_list`：由远程 provider 直接列出资源元数据，不创建本地空占位文件；Core 可以据此接入 VFS。

## 规格审查发现的实际问题

1. **独立执行程序与逻辑独立不是一回事**：初期把旧 Core 业务调用移到另一进程可以验证 IPC，却依然有库依赖。正式发布前必须把三种后端移动到 `plugins/official/*` 独立 crate，使 Core 不链接任何实现。
2. **目录列表必须明确快照/游标**：当前过渡 `fs.list` 只返回节点数组，并不满足 [v1 VFS 规范](02-VFS.md) 的 `{entries,nextCursor,snapshotToken}`；正式能力声明前需实现完整接口。
3. **数据流不可塞 JSON**：当前过渡下载命令仍通过宿主本地路径/旧镜像工作；真正的 `fs.openRead/beginWrite/commitWrite` 必须使用受控二进制流和版本前置条件。
4. **SDK 与宿主对外语义尚未冻结**：HostBridge 示例只提供握手、请求白名单，不代表安装/权限/独立 origin 和资源加载链已完成。不得让不可信 iframe 在主应用同源页面中运行。
5. **缓存和同步仍在遗留 `remote.rs`**：直到 CacheManager/SyncEngine 落地并完成有效缓存迁移，不能声称 WebDAV/SMB 已经从内核完整拆分。
6. **可靠取消和任务机制仍未迁移**：Native 进程同步调用无法提供完善的每任务取消，正在跟进独立 TaskManager。

## 发布阻断项

- [ ] SDK、Native 宿主、三个二进制跨平台 `cargo fmt/clippy/test` 和完整构建通过。
- [ ] 旧 `remote.rs` / `git*.rs` 不再被 Core 链接，插件可独立构建和打包。
- [ ] 官方插件不在 Host 实现中硬编码列表；第三方 manifest 动态安装/能力授权可用。
- [ ] WebDAV/SMB 所有目录与文件访问都使用 Core VFS，无零字节占位，连接配置/凭据安全迁移完成。
- [ ] Git 工具栏和历史虚拟目录通过同一插件 API 接入，无专用 Tauri Git 命令。
- [ ] 内核级缓存、选择性同步、冲突解决、任务进度、取消、恢复真正落地。
- [ ] iframe UI 独立 origin、CSP/权限、构建产物加载及真实活动中心集成。
- [ ] 原有文件浏览、Git、WebDAV、SMB 行为与三平台打包测试不回退。

## 稳定性策略

在上述阻断项完成之前，所有 protocol 标注为 `experimental-0.1`，未来可以破坏性修改；**不要向第三方承诺 v1.0 的向后兼容性**。首个正式版必须附完整 JSON Schema、行为测试、错误码和安全规则。
