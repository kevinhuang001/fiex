# 22 · 主题包开发指南

[索引](README.md) · 相关：[20 主题与面板生命周期](20-THEMES-AND-PERFORMANCE.md) · [21 面板 UI 开发](21-PANEL-UI-GUIDE.md)

主题是**纯声明式插件**：没有可执行文件、没有 iframe、没有权限。它只提供一组语义 token，Core 把它们挂到窗口根节点上，插件面板通过 Bridge 收到同一组值。这篇文档覆盖包的形态、可用的 token、校验规则、本地预览和验收。

## 1. 主题包的形态

```text
org.fiex.theme.vista.fiexplugin    (zip)
  manifest.json
  theme.json
```

- `manifest.json` 里**不能有** `runtime` / `entrypoints`；只有 `contributions.themes`。
- 安装后出现在 **设置 → 外观 → 主题**，选中的主题立即应用到主窗口和所有已打开的面板。
- 禁用/卸载当前主题后回到内置调色板。

> 判定规则在 `src-tauri/src/plugins/themes.rs`：`entrypoints` 为空且 `contributions.themes` 非空，才按主题包处理；否则按普通插件校验。

## 2. 最小示例

`manifest.json`：

```json
{
  "schemaVersion": 1,
  "id": "org.example.theme",
  "name": "Example Theme",
  "version": "1.0.0",
  "description": "示例主题",
  "permissions": {},
  "contributions": {
    "themes": [{ "path": "theme.json" }]
  }
}
```

`theme.json`：

```json
{
  "id": "light",
  "name": "Example Light",
  "mode": "light",
  "tokens": {
    "accent": "#0067c0",
    "surface": "#f3f3f3",
    "canvas": "#ffffff",
    "text": "#202020",
    "muted": "#626262",
    "line": "#dedede",
    "hover": "#e9e9e9",
    "selected": "#e5f1fb",
    "radius-control": "6px",
    "radius-panel": "12px"
  }
}
```

设置里显示的标识是 `<plugin-id>:<theme-id>`，例如 `org.example.theme:light`。主题文件不提供任何 token 时，对应属性回退到**该模式的内置值**，所以可以只覆盖想改的部分。

## 3. 校验规则（违反即安装失败）

| 对象                   | 规则                                                                           |
| ---------------------- | ------------------------------------------------------------------------------ |
| `contributions.themes` | 数组，≤ 16 项；每项 `{ "path": "..." }`，路径必须是包内相对路径                |
| 主题文件               | ≤ 32 KiB                                                                       |
| `id`                   | 非空，≤ 64，仅 `A-Z a-z 0-9 - _`                                               |
| `name`                 | ≤ 128 字符                                                                     |
| `mode`                 | 必须是 `light` 或 `dark`                                                       |
| `tokens`               | ≤ 256 项                                                                       |
| token 名               | 非空，≤ 64，仅小写字母、数字、`-`（`^[a-z][a-z0-9-]*$`）                       |
| token 值               | ≤ 512 字符；**不能**包含 `;` `{` `}` `<` `>` `\`，不能出现 `url`，不能包含 `@` |
| 重复 id                | 同一插件内主题 id 必须唯一                                                     |

值允许渐变与多段阴影（只要不含上面那些字符），例如：

```json
{ "toolbar-background": "linear-gradient(180deg, #f4fbff, #c3e0f4 54%, #d8ecfa)" }
```

> 插件面板侧还有一层同样的校验（`sdk/ui/src/index.ts` 的 `applyTheme`），所以恶意或损坏的值不会变成任意 CSS。

## 4. token 参考

分三类：**基础色**决定整体观感，**表面 token** 决定组件长什么样，**圆角/字体** 决定质感。所有 token 都可以是颜色、渐变或阴影（除非组件把它当纯色参与 `color-mix`）。

### 4.1 基础色

| token                            | 作用                                               |
| -------------------------------- | -------------------------------------------------- |
| `text` / `muted`                 | 主文字 / 次要文字与列头                            |
| `canvas` / `surface`             | 内容底 / 面板底、卡片底、工具栏默认底              |
| `hover` / `selected`             | 通用悬停底 / 通用选中底（未单独 token 化的组件用） |
| `line`                           | 分隔线、边框                                       |
| `accent`                         | 强调色：焦点框、链接、主按钮、进度                 |
| `on-accent`                      | 强调底上的文字                                     |
| `danger` / `success` / `warning` | 语义状态色                                         |
| `glass` / `overlay` / `shadow`   | 玻璃层、模态遮罩、浮层阴影                         |

### 4.2 表面 token

| token                                                                                   | 控制的组件                          |
| --------------------------------------------------------------------------------------- | ----------------------------------- |
| `sidebar-background` / `sidebar-line`                                                   | 左侧栏与它的右边界                  |
| `toolbar-background` / `toolbar-line`                                                   | 工具栏、底部发丝线                  |
| `tabbar-background`                                                                     | 标签条                              |
| `tab-background` / `tab-hover-background` / `tab-active-background` / `tab-active-line` | 标签：默认 / 悬停 / 激活 / 激活描边 |
| `row-alt-background` / `row-hover-background`                                           | 列表行斑马纹 / 悬停                 |
| `row-selected-background` / `row-selected-line` / `row-selected-text`                   | 选中行底、描边、文字                |
| `nav-active-background` / `nav-active-line`                                             | 侧栏当前项                          |
| `statusbar-background`                                                                  | 底部状态栏                          |
| `menu-background` / `context-menu-background` / `menu-line`                             | 下拉菜单 / 右键菜单 / 菜单边框      |
| `input-background` / `input-line`                                                       | 输入框、搜索框、路径编辑框          |
| `control-hover-background`                                                              | 按钮/菜单项的悬停底                 |
| `segmented-background` / `segmented-active-background`                                  | 分段控件（视图切换）与其激活项      |

### 4.3 圆角与字体

| token                            | 默认        |
| -------------------------------- | ----------- |
| `radius-control`                 | 9px         |
| `radius-panel`                   | 12px        |
| `radius-row`                     | 7px         |
| `radius-tab`                     | 10px        |
| `radius-menu` / `radius-submenu` | 12px / 10px |
| `font-family`                    | 系统字体栈  |

## 5. 主题如何到达窗口与面板

```text
theme.json ──安装──▶ Core plugin_themes
                       │ 选中主题
                       ▼
             applyTheme() 把 token 写成 <html> 内联变量
                       │
        ┌──────────────┴───────────────┐
        ▼                              ▼
  主窗口 CSS 直接 var(--x)      themeSnapshot() → 面板 MessagePort
                                       │
                                       ▼
                              SDK 写成 iframe 的 <html> 内联变量
                                       │
                                       ▼
                              插件面板 CSS var(--x) 立即生效
```

这意味着**主题作者不需要管插件面板**：面板只要用了语义 token，就会跟着变。反过来，面板如果硬编码颜色，主题也救不了它（见 21 第 7 节）。

## 6. 深浅色

- 一个 `theme.json` 只有一个 `mode`。要提供明暗两套，就在同一个包里放两个主题条目：

```json
"contributions": {
  "themes": [
    { "path": "light.json" },
    { "path": "dark.json" }
  ]
}
```

- `mode: "dark"` 会让 Core 给根节点加 `dark` 类并设置 `color-scheme: dark`；不提供主题时使用内置深色板。
- 内置深色板对 `sidebar-background`、`sidebar-line`、`nav-active-*` 有单独取值（`src/styles/tokens.css` 的 `:root.dark`）。自定义深色主题时记得把这些也一起覆盖，否则会拿到浅色推导值。
- 面板里也可以写 `html.dark .selector`，但优先用 token，避免每个主题都要维护分支。

## 7. 官方两套主题的做法

`fiex-official-plugins` 里的 `official/vista` 和 `official/win11` 是两个可参考的完整例子：

| 特征   | Vista Aero                                            | Windows 11                           |
| ------ | ----------------------------------------------------- | ------------------------------------ |
| 圆角   | `radius-control: 3px`、`radius-panel: 5px`            | `4px` / `8px`                        |
| 工具栏 | 双段高光渐变（`#f4fbff → #dceefb 46% → #c3e0f4 54%`） | 近似单色 `#f7f7f7` + `toolbar-line`  |
| 侧栏   | 竖直蓝色玻璃渐变                                      | 中性 `#f3f3f3`                       |
| 选中行 | 渐变 + `#a7d8f5` 描边                                 | 扁平 `#cfe8fc`                       |
| 菜单   | 五段渐变 + `#7d8ea0` 边框，5px 圆角                   | `#f9f9f9` + `#e5e5e5` 边框，8px 圆角 |
| 强调色 | `#1e6fb8`                                             | `#005fb8`                            |
| 字体   | Segoe UI                                              | Segoe UI Variable                    |

取值参考了开源的 [7.css](https://github.com/khang-nd/7.css)（Aero 按钮/列表/菜单变量）和 [Fluent 设计色板](https://github.com/calvinhxx/Fluent-Qt/blob/main/docs/design-languages/fluent.md)（Windows 11 的 `#005FB8`、`#F3F3F3`、`#C42B1C` 等）。

想“像”某个系统，重点不在改 `accent`，而在：

1. 圆角家族（控制 Vista 的硬朗 / Win11 的圆润）；
2. 表面渐变（Aero 的高光段、Win11 的纯色层）；
3. 行选中与菜单的边框（Aero 的蓝边 vs Fluent 的灰边）；
4. 中性色的冷/暖倾向。

## 8. 本地预览

```bash
npm run dev   # Core dev server: http://127.0.0.1:1420/
```

开发模式下 Vite 会扫描**同级仓库** `../fiex-official-plugins/official/*/manifest.json`，把其中的主题（含 `theme.json`）按 Core 的命名空间规则汇总到 `/__fiex_theme/index.json`。因此：

- 改完 `theme.json` 刷新页面，**设置 → 外观 → 主题** 里就能选到它，无需安装插件。
- 这条通道**只在 DEV 存在**；生产/浏览器构建没有插件宿主，主题必须通过安装获得。
- 主题生效后，顺手打开任意插件面板确认颜色一致——这是验证“面板也在吃同一套 token”的最快方式。

## 9. 发布

主题包与普通插件走同一套 Release 流程（见插件仓库 README）：

- `scripts/generate-catalog.py` 的 `SLUGS` 需要包含该主题的 slug；无 `runtime.entrypoints` 但有 `contributions.themes` 的包会为所有平台生成同一条 `assets` 记录。
- `scripts/package-fiex-plugin.py` 会把 `theme.json` 一起打进 `.fiexplugin`，无需编译产物。
- 主题包不需要签名二进制，但仍需用户安装并启用。

## 10. 验收清单

- [ ] 明/暗两种模式下文字与背景对比度足够（尤其 `muted`、`line`）。
- [ ] 逐个检查：侧栏、工具栏、标签条、列表选中/悬停、右键菜单、下拉菜单、状态栏、输入框、分段控件。
- [ ] 打开一个插件面板，确认面板颜色随之变化（说明 token 链路通了）。
- [ ] 切回内置主题，确认没有残留（Core 会移除旧主题写入的变量）。
- [ ] 只覆盖必要 token，未覆盖的保持内置值。
- [ ] 没有在值里使用 `;`、`url(...)`、`@`——否则安装会被拒绝。

## 11. 常见问题

| 现象                       | 原因                                                                                          |
| -------------------------- | --------------------------------------------------------------------------------------------- |
| 安装后“外观”里没有这个主题 | manifest 带了 `runtime`/`entrypoints`（被当成普通插件）；或 `themes[].path` 越界/不存在       |
| 提示 invalid theme token   | 值里出现 `;` `{` `}` `<` `>` `\`、`url` 或 `@`；或 token 名含大写/下划线                      |
| 设了 token 但界面没变化    | 该组件消费的是另一个 token（例如行悬停用 `row-hover-background` 而不是 `hover`）              |
| 菜单/标签改不动            | 检查是否用了对应表面 token；旧版本可能被 `settings.css` 的覆盖规则吃掉（21 与本次重构已消除） |
| 面板不跟随主题             | 面板硬编码了颜色，或用了主题未声明的自定义 token                                              |
| 深色主题侧栏发灰           | 没覆盖 `:root.dark` 里的默认值，显式设置 `sidebar-background`                                 |
