# 目录缓存、搜索与传输预检

本页说明目录打开、搜索和文件传输如何共享 Core 的资源身份和 provider 能力。缓存提升首屏速度，但不代替权限检查或 provider 当前状态。

## 打开目录

1. UI 确定当前 pane、目录 URI 和请求 revision。
2. UI 可先读取已授权目录的展示快照，让列表尽快绘制。
3. Core 通过统一 listing 边界请求对应 provider。分页 VFS 只有完整成功后才形成完整快照。
4. Core 汇总条目、能力和 presentation 数据，再以 revision 发送增量更新。
5. UI 按资源身份比较新旧项目；相同项目保留选择和滚动位置，过期请求或旧 pane 响应被丢弃。

本地文件系统和 USB provider 的目录操作仍会访问实时 adapter。文件选择器、列表、列视图、Recent、Applications、Trash 和连接挂载复用统一的授权/展示边界，但各自的数据源和支持操作不同。

## 搜索

本地名称和内容搜索实时遍历当前可访问范围，不以目录展示快照作为内容索引。VFS provider 可以提供原生搜索；Core 也可对已访问目录图使用列表回退。结果按路径合并，旧查询不会覆盖新查询，完成批次可以清除已失效结果。

名称搜索不应读取文件字节。内容搜索遵从相应的文件读取能力、文本/二进制识别、大小限制和显示忽略规则。插件搜索要经过 SearchProvider 路由和结果验证。对外说明的结果上限和不搜索应用包内部等行为见[搜索与预览文件](/user-guide/search-preview)。

## 传输预检与能力

传输计划由源 provider 的导出能力、目标 provider 的导入能力和当前资源 scope 共同约束。跨 mount 操作携带可移植来源元数据，不能把一个 provider 的 node ID 交给另一个 provider 使用。目标目录尚未创建时，Core 先在最近的已存在目标祖先上评估相对目标名。

Preflight 可检查额外、动态的 provider 策略，例如系统应用能否导出、文件名或远端状态是否可接受。它只会收窄操作，不代替授权。执行前重新检查，并在移动清理源文件前再次确认源操作仍可用。

菜单“可用性”结果允许短暂复用；真正执行请求不能沿用旧的缓存授权结论。操作成功或 provider 注册变化后，Core 清理相关预检结果。

## 内置位置的策略

- Recent 只提供映射入口，不继承源文件删除能力。
- Applications 只接受 Core 能确认的应用/快捷方式，并限制内部组织；系统应用不允许当作普通文件导出或覆盖。
- Trash 使用自己的还原与永久删除动作，不把 Trash 根目录当作通用文件夹。
- 普通本地文件按 provider 与节点的 operation capabilities 决定动作。

## 对应验证

- Frontend 结果合并与快照稳定：src/features/explorer/model/searchBatch.test.ts、src/lib/listingDiff.test.ts、src/features/explorer/model/paintedListingCache.test.ts。
- Core 目录图、取消和失效：src-tauri/src/core/fs/directory_cache/tests.rs。
- 搜索流、忽略和内容边界：src-tauri/src/core/search/tests.rs。
- 传输与能力负面路径：src-tauri/src/core/fs/preflight、src-tauri/src/core/fs/tests.rs、src-tauri/src/plugins/resource/file_capabilities/tests.rs。
- Applications / Recent / Trash：builtins/fiex-applications、builtins/fiex-trash、core/volumes/native_vfs 和 context_menu tests。

完整的需求编号、测试证据和系统级缺口见[软件需求与测试追溯](/developer/requirements)。
