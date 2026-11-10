# 缓存、诊断与错误合同

本页集中说明几个跨 UI、Core 和 provider 的运行时约束。它补充[系统架构](/developer/architecture)，不是另一套独立协议；实现和测试位置以当前[代码基线](/reference/source-baseline)为准。

## 列表缓存不是文件事实

目录快照用于让再次进入的位置更快呈现。Core/provider 的新列表是最终状态来源。目录枚举成功后才提交完整 VFS 快照；分页失败不能把不完整结果当作完整目录。缓存命中可以先画面展示，但后台刷新必须继续运行。

UI 和 Core 对更新使用目录身份、资源身份及 revision 比较。旧导航或旧 mutation 的响应不能覆盖新状态；目录变化后失效受影响的父子快照和相应 presentation。用户的偏好、连接和 pin 意图属于持久状态，不随缓存淘汰一起删除。

目录结构、图标/装饰、预览/Info 元数据和远程文件字节分别缓存。它们失效条件不同，不应把其中一类缓存当作另一类权限或最终结果。

## 变更通知与预检

成功的创建、重命名、删除、Trash 移入和提交写入会通知变更所有者并失效相关目录；失败或 abort 的写入不得广播为已提交变更。跨 provider 传输在写入前检查来源导出、目标导入或同 mount 移动能力，并在执行阶段重新确认。

Provider 的 preflight 是附加策略检查，不是授权来源。它可以因服务器、目标类型或应用导入规则拒绝操作，但不能授予缺失的能力或 scope。Malformed reply、过期资源和预检错误都应失败关闭。

## 诊断与隐私

Rust 日志、插件日志、错误详情和导出前先清理凭据头、OAuth 查询值及已识别的 secret 模式。日志记录用于定位操作和错误，不应存放完整请求体、文件内容、密码、token 或私钥。前端错误文本也需在入队和导出路径处理。

日志是有界诊断记录，不是审计承诺或业务状态数据库。某条日志缺失不应被解释为操作从未发生；诊断信息也不能代替文件/provider 的返回状态。

## 面向用户的错误

用户可见的运行时错误通过统一 Error Center 报告。必填校验、无效重命名模板、权限说明、加载/成功状态和可恢复提示留在原操作位置；普通取消和预期 fallback 不应弹出重复错误。不要从插件可控的 message 文本推断 Core 的取消、授权或控制流语义。

若错误同时有 Core diagnostic ID，UI 可以用它链接到日志详情。日志级别为 error 本身不代表应打开模态对话框。命令调用方仍负责保持自己的工作流状态，避免后端和前端重复呈现同一个失败。

## 对应验证

- 列表快照、父子失效与 presentation 一致性：Rust 的 core/fs/directory_cache/tests.rs；前端的 listingDiff.test.ts、paintedListingCache.test.ts、entryPresentation.test.ts。
- 成功写入与失败事务：Rust 的 core/fs/tests.rs、core/transfers/copy/tests.rs，以及 provider/fs/streams tests。
- 错误去重和操作系统权限错误路由：src/lib/errorCenter.test.ts。
- 日志 secret 清理和写入顺序：src/lib/redactDiagnostic.test.ts、src-tauri/src/infrastructure/logging/tests.rs。
- 用户流程中状态中心的显示：e2e/explorer/operations.spec.ts。

CI 结果和未覆盖的磁盘耗尽、多进程日志旋转、强制结束恢复等限制见[软件需求与测试追溯](/developer/requirements)和[格式、构建与 CI](/developer/quality-ci)。
