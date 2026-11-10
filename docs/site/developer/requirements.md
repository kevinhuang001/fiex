# 软件需求与测试追溯

本文把当前 Core 主分支中已经实现的行为整理成可检查的软件需求，并列出对应的自动化验证位置。需求是从代码、界面和 CI 提炼的现状基线，目的是让新改动能说明“改了什么、由什么测试保护”；测试追溯不是对全功能无缺陷的承诺。

## 测试类型与解释方式

| 标记 | 验证层 | 可以证明什么 | 不能单独证明什么 |
| --- | --- | --- | --- |
| **UT** | TypeScript/Rust 单元测试 | 纯逻辑、转换和策略在给定输入下正确 | 整个桌面应用的系统行为 |
| **UI** | React 组件测试 | 某个组件在受控 props/state 下展示或交互 | OS 文件对话框、原生窗口和真实磁盘 |
| **E2E** | Playwright 浏览器测试 | 用户流程在自动化页面与隔离 fixture 下连贯工作 | 所有平台的 native API 和真实账户服务 |
| **Rust** | Core/SDK Rust 测试 | provider、授权、资源和 Core 业务逻辑正确 | 桌面包在用户机器上的全部运行环境 |
| **IPC** | CI 外部 mock 插件进程 | 真实子进程启动、握手、请求、并发/取消和关闭边界 | 第三方生产服务的全部异常行为 |
| **FM** | Kani / 覆盖率 / mutation | Scope policy 在有限输入模型下满足属性，并检验测试敏感性 | 未建模的 OS、网络与 UI 行为 |
| **PLAT** | macOS/Windows/Linux CI 或实机检查 | 指定 runner/OS 的平台相关路径 | 未覆盖的系统版本、厂商驱动或用户配置 |

需求的“追溯测试”是维护入口；逐 PR 仍需看当次 CI 结果。没有列出自动化测试的位置时，写明需要的人工平台检查，不用推测已经覆盖。

## Part A：导航与浏览需求

| ID | 软件需求 | 验收要点 | 当前追溯 |
| --- | --- | --- | --- |
| `UI-NAV-01` | 用户应能从边栏或地址栏进入可访问的位置，并查看对应目录内容。 | 文件夹切换后路径与列表指向同一位置；无效位置给出错误。 | E2E `e2e/explorer/browsing.spec.ts`；UT `src/hooks/explorer/useNavigation.test.ts`、`useListingLocations.test.ts` |
| `UI-NAV-02` | 用户应能用标签页保留多个浏览上下文，并可在标签中启用一个或两个面板。 | 切换标签恢复其位置；最后标签关闭时使用配置的起始页。 | E2E `browsing.spec.ts` 中 tabs/split pane 用例；UT `useNavigation.test.ts` |
| `UI-VIEW-01` | 用户应能在列表、图标、列和画廊视图间切换，并设置排序/分组。 | 排序只改变活动面板；目录可恢复自己的展示偏好。 | E2E `browsing.spec.ts`；UT `src/lib/folderView.test.ts`、`listColumns.test.ts`、`entryPresentation.test.ts` |
| `UI-LIST-01` | 大目录浏览不能要求一次把所有可见行都渲染到 DOM。 | 10,000 项 fixture 能维持虚拟化并支持末端选择与重命名。 | E2E `browsing.spec.ts` 的 10,000 icons 用例；UT `src/hooks/explorer/paintedListingCache.test.ts` |
| `UI-SEARCH-01` | 用户应能在当前搜索能力范围内查找名称/内容，并查看增量结果。 | 搜索批次合并、取消/过期响应不覆盖新查询；结果受界面上限约束。 | E2E `browsing.spec.ts` 搜索流程；UT `src/hooks/explorer/searchBatch.test.ts`；Rust `src-tauri/src/core/search/tests.rs` |
| `UI-PREVIEW-01` | 用户应能预览受支持文件，并将其他文件交给系统应用打开。 | 图片、文本/代码、PDF 等按实现渲染；无法预览时可选择系统应用。 | E2E `e2e/explorer/browsing.spec.ts` 代码/PDF 用例、`plugins-providers.spec.ts` Open With 用例；UT `src/lib/preview.test.ts` |
| `UI-PLUGIN-01` | 插件 UI 应在 Fiex 提供的隔离容器中显示，并只通过受控 bridge 交互。 | 插件面板可关闭/重开；插件页面可读取当前上下文，不获得任意文件路径 API。 | E2E `e2e/explorer/plugins-providers.spec.ts` panel/iframe cases；UI `src/components/plugins/panel/resources.test.ts` |
| `UI-INFO-01` | 信息面板应区分文件和文件夹，并避免自动计算声明为按需的插件字段。 | 只请求用户点击的字段；打开面板不触发所有 onDemand 计算。 | E2E `browsing.spec.ts` 的 “Info computes only the on-demand field…”；Rust `src-tauri/src/plugins/providers/info/tests.rs` |

## Part B：文件、位置和偏好需求

| ID | 软件需求 | 验收要点 | 当前追溯 |
| --- | --- | --- | --- |
| `OPS-SEL-01` | 用户应能单选、多选和全选当前目录项目。 | 选择状态与键鼠操作一致，选择操作不会改变源文件。 | E2E `e2e/explorer/operations.spec.ts` multi-select；`browsing.spec.ts` 大列表末端选择 |
| `OPS-FILE-01` | 用户应能创建/重命名文件夹和文件，并在名称冲突时作出明确选择。 | 名称预览/确认正确；冲突支持实现中的比较、跳过、替换或保留两者流程。 | E2E `e2e/explorer/terminal.spec.ts` create/rename；`operations.spec.ts` inline rename/conflict/batch rename；UT `src/lib/rename.test.ts` |
| `OPS-XFER-01` | 用户应能在 Fiex 中复制、剪切、粘贴和拖动项目。 | 同卷/跨卷行为满足文件系统能力；跨卷错误不把旧结果展示为成功。 | E2E `operations.spec.ts` transfer/move tests；UT `src/hooks/file-ops/transfer.test.ts`；Rust `src-tauri/src/core/transfers/copy/tests.rs` |
| `OPS-TRASH-01` | 用户应能把支持的位置的项目移入废纸篓，并在系统提供原路径记录时尝试放回。 | Recent 删除只移除最近映射；Trash 操作遵守来源和恢复路径差异。 | Rust `src-tauri/crates/builtins/fiex-trash/src/tests.rs`；E2E `e2e/explorer/terminal.spec.ts` 覆盖回收流程、`browsing.spec.ts` 覆盖 Trash 打开；macOS CI 专用 Finder/Fiex Put Back regression。其它来源的真实系统 Trash restore 仍需 PLAT 实测 |
| `LOC-CORE-01` | Recent、Applications、Trash 等内置位置应按其虚拟语义显示，并限制不支持的操作。 | URI/显示项身份保持一致；Recent 删除不删除原文件；位置能力控制菜单。 | E2E `browsing.spec.ts` 覆盖 Applications/Trash 打开；Rust `src-tauri/src/core/volumes/native_vfs/tests.rs`、`src-tauri/crates/builtins/fiex-applications/src/tests.rs`；Recent 菜单映射见 `src-tauri/crates/builtins/fiex-recent/src/module/actions.rs`，当前没有专门的 Recent 删除 E2E |
| `PREF-01` | 用户应能调整外观、标签、忽略规则、启动位置及快捷键等设置。 | 偏好跨重载持久化；快捷键冲突可处理；标签变更有确认流程。 | E2E `e2e/explorer/settings.spec.ts`；E2E `e2e/i18n.spec.ts`；UT `src/lib/shortcuts.test.ts`、`tags.test.ts`、`places.test.ts` |

## Part C：插件与连接需求

| ID | 软件需求 | 验收要点 | 当前追溯 |
| --- | --- | --- | --- |
| `PLUG-INST-01` | 安装/导入插件前，Core 应校验 manifest、包身份、文件路径和协议兼容。 | 不兼容协议不启动；包内路径不能越界；导入界面展示身份/签名信息。 | E2E `e2e/explorer/plugins-providers.spec.ts` import/signature cases；Rust `src-tauri/src/plugins/installer/marketplace/tests.rs`、`signing/tests.rs`、`host/manifest/contribution_tests.rs` |
| `PLUG-CONSENT-01` | 启用插件必须由用户审查其声明权限；取消不得授予。 | 可取消个别能力；撤销后能力调用失败或重新提示。 | E2E `e2e/explorer/plugins-lifecycle.spec.ts`；Rust `src-tauri/src/plugins/security/permissions/tests.rs`、`policy/tests.rs`、`prompt/tests.rs`；UT `src/lib/pluginPermissions.test.ts` |
| `PLUG-IPC-01` | Native 插件请求与 Host API 调用必须使用版本化 JSON-RPC，并匹配 request/response。 | 正确握手、ID、帧边界、并发与关闭；stdout 不含协议外日志。 | Rust SDK framing tests `sdk/rust/src/lib.rs`；SDK runtime/provider tests；Core CI external mock process `src-tauri/examples/plugin_smoke.rs` + `.github/workflows/ci.yml` |
| `PLUG-SCOPE-01` | 每次插件 Host API 调用必须受 Manifest、用户 grant 和当前资源/操作范围共同约束。 | 资源身份不能跨请求/任务复用；越界、过期或无效授权 fail closed。 | Rust `src-tauri/src/plugins/resource/fs_access/tests.rs`、`resource_context/tests.rs`、`file_capabilities/tests.rs`；**FM** Kani job `scope-policy-verification`，coverage job ≥85% lines，mutation job |
| `PLUG-VFS-01` | VFS 插件应声明静态实现上限，并针对当前 mount/资源发现动态可用操作。 | discovery 结果不增加上限；流按 mount 隔离；未 commit 写入被丢弃。 | Rust `src-tauri/src/plugins/vfs/tests.rs`、SDK `sdk/rust/src/filesystem/tests.rs` 与 `filesystem/tests/streams.rs`；IPC Core-host 流 smoke |
| `PLUG-PROV-01` | Provider 扩展应按 manifest 注册、按资源匹配，并在执行时验证输入和权限。 | Info、Preview、菜单、Search、装饰和连接调用只交给合适的 provider。 | E2E `e2e/explorer/plugins-providers.spec.ts` 覆盖搜索、连接、菜单/UI bridge；Rust `src-tauri/src/plugins/providers/info/tests.rs`、`preview/tests.rs`、`context_menu/tests/routing.rs`、`decorations/tests.rs`、`fs/tests.rs`、`connection_menu/tests.rs` |
| `PLUG-PROV-02` | Native SDK 应只分发插件注册的标准事件，并让未注册业务方法进入 fallback。 | provider ID 稳定且唯一；插件可组合多个 provider；未知标准事件按 method-not-found 处理。 | SDK `sdk/rust/src/providers/tests.rs` 覆盖注册/选择/组合；fallback 和 Hooks/Toolbar 尚无专用 E2E，新增这类行为时应补相应集成用例 |
| `CONN-01` | 用户应能保存、校验、编辑并打开插件连接。 | 修改已有连接不重复创建；连接视图与账号/mount 身份关联。 | E2E `e2e/connections.spec.ts`、`plugins-providers.spec.ts`；Rust `src-tauri/src/remote/files/tests.rs`、`reconcile/tests.rs` |

## Part D：质量属性与平台要求

| ID | 软件需求 | 验收要点 | 当前追溯 |
| --- | --- | --- | --- |
| `SEC-NET-01` | 插件网络访问须经 Core 网络代理，并按批准的主机限制。 | 未批准目标、已撤销 grant 和不允许重定向不能访问。 | Rust `src-tauri/src/plugins/net/tests.rs`、`security/policy/tests.rs`、`permissions/tests.rs`；IPC mock tests；按平台沙箱检查 |
| `SEC-CMD-01` | 外部命令只能按 manifest 声明与用户批准执行，并按 argv 调用。 | 未声明命令拒绝；命令 grant 必须同时符合声明和当前用户选择。 | Rust `src-tauri/src/plugins/security/permissions/tests.rs`、`policy/tests.rs` 覆盖声明/精确授权；进程启动沙箱还需 PLAT 检查和代码审阅 |
| `REL-01` | 旧的异步目录请求、搜索批次或装饰响应不能覆盖较新的导航状态。 | 更新序号/取消状态正确；刷新后 UI 与 provider 新结果一致。 | UT `src/lib/listingDiff.test.ts`、`listing.test.ts`、`entryPresentation.test.ts`；E2E `e2e/explorer/browsing.spec.ts`；Rust `src-tauri/src/core/fs/directory_cache/tests.rs` |
| `REL-02` | 支持取消的长任务应上报状态并释放资源。 | cancel 后任务最终结束；文件、网络与 VFS 流在关闭/错误时回收。 | Rust SDK `sdk/rust/src/sdk_tests/tasks.rs`、`filesystem/tests/streams.rs`、`service_tests.rs`；Core CI `plugin_smoke` / `plugin_concurrency` |
| `PORT-01` | Core 应在 macOS、Windows、Linux 构建并运行其支持的桌面功能。 | 各平台 CI 安装/编译成功；针对系统差异的能力有平台专测或清楚限制。 | `.github/workflows/ci.yml` desktop matrix；macOS Trash regression；PLAT 实机检查仍覆盖 OS 版本、驱动和隐私配置差异 |
| `I18N-01` | 前端和插件请求应按当前语言展示界面文本并处理语言切换。 | 受支持语言的消息完整；运行时语言切换应用于当前界面。 | E2E `e2e/i18n.spec.ts`；UT `src/i18n/index.test.ts`；CI `npm run check:i18n`；Core IPC locale example |
| `PRIV-01` | 诊断信息不应把令牌、凭据或敏感参数明文暴露。 | 已知敏感模式被清理；错误提示可定位而不包含文件内容。 | UT `src/lib/redactDiagnostic.test.ts`；Rust logging tests `src-tauri/src/infrastructure/logging/tests.rs`；人工检查新日志字段 |

## 测试目录与需求维护规则

前端逻辑测试按源码附近放置 `*.test.ts(x)`；Playwright 用户流程集中在 `e2e/`；Core Rust 测试按 crate/module 的行为边界放入 `tests.rs` 或子目录；SDK/provider contract 测试放在 SDK 子模块。跨进程和多平台集成由 Actions workflow 明确编排，不能仅看一个目录名推断执行状态。

每次新增或改变用户可见需求时：

1. 给需求分配稳定 ID，写出可观察的验收条件。
2. 加入最接近该行为的单元/组件测试；完整用户流程再加 E2E。
3. 改动 Core/SDK 进程边界时，补充 Rust 合约或外部进程集成检查。
4. 修改权限/scope 决策时，更新属性测试、Kani harness 或对应安全用例。
5. 记录平台依赖及尚未自动验证的路径，不把构建成功写成运行行为测试通过。
6. 在 PR 中更新本页的需求与测试追溯。

本页不是固定测试清单。CI 命令会改变；当前执行计划以 [Core CI workflow](https://github.com/kevinhuang001/fiex/blob/main/.github/workflows/ci.yml) 为准。
