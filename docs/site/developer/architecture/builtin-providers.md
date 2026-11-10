# 内置菜单与文件装饰

右键菜单和文件行装饰由 Core provider 解析，通用的列表、图标、列和画廊 renderer 只绘制已验证结果，不重新实现位置权限或连接状态策略。

## 右键菜单

Core 和插件菜单共用 event router。请求开始时 Core 捕获有效选择：在当前选择内右键时使用整组选择，在选择外右键则只使用被点击项目。资源引用绑定到这次菜单 session。

菜单项解析与执行分离。执行请求必须携带当前 session 中真实存在且可用的 action ID，并回到生成该 action 的 provider。禁用 action、未知父项、过期 session 或伪造的 Core action 都会被拒绝。插件子菜单按需加载并检查深度、数量和重复 ID。

内置策略包括打开/预览/Info、Finder 标签、连接缓存和同步、重命名、剪贴板、副本、终端、复制路径与 Trash。前端收到 Core action 后执行相应 UI 流程；是否提供某个菜单项由 Core/provider 的能力结果决定。

## 装饰与状态

内置标签、只读状态、连接缓存/同步状态和插件装饰通过 presentation 流合并。结果按稳定优先顺序处理；只接受当前目录内项目的装饰。插件 badge、颜色和描述经过字段验证，不能改变文件身份或原始名称。

所有视图共享相同的 name/badge renderer。呈现组件不维护连接同步、标签、只读规则或操作能力的第二份状态。

## 对应验证

- 菜单可见性：src/components/explorer/context-menu/menuVisibility.test.ts。
- 菜单分组与 workspace 行为：src/components/explorer/context-menu/moreMenu.test.ts。
- action/session/provider 归属：src-tauri/src/plugins/providers/context_menu/tests/contracts.rs、routing.rs 和 native_views/tests.rs。
- 标签与连接装饰：src-tauri/src/plugins/providers/decorations/tests.rs、dynamic/tests.rs；UI 名称转义见 src/components/browser/DecorationBadges.test.tsx。
- 用户点击流程：e2e/explorer/operations.spec.ts、plugins-providers.spec.ts。

各位置的可用菜单与当前测试缺口列在[软件需求与测试追溯](/developer/requirements)。
