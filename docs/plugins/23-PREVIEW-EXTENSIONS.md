# 23 · 预览扩展点（preview）与 office-preview 插件设计

[索引](README.md) · 相关：[05 UI 扩展点](05-UI-EXTENSIONS.md) · [11 UI SDK](11-UI-SDK.md) · [21 面板开发](21-PANEL-UI-GUIDE.md)

> **状态：v1 已实现。** 插件声明 `contributions.preview`，按扩展名与 `priority` 参与解析；office-preview 是首个使用者。
>
> v1 的行为：命中插件预览器时，在预览弹窗内部挂载该插件的面板（与其他插件面板同一套沙箱、Bridge 和错误记录），内置文本/图片/媒体/PDF 不变。用户侧的拖动排序与禁用开关尚未接入设置页；当前由 manifest 的 `priority` 决定，并在 `src/lib/preview.ts` 预留了 `previewOrder` / `previewDisabled` 的存储与解析。

## 1. 目标与非目标

**目标**

- 插件声明自己能预览哪些扩展名，以及优先级。
- 插件可以覆盖内置预览器（例如用更强的 PDF 渲染器替换 `core.pdf`）。
- 用户可以在设置里禁用/调整预览器顺序，覆盖插件声明的优先级。
- 预览失败或超时时自动降级到下一个预览器，最终回到内置文本/十六进制预览。
- 预览 UI 与插件面板共用同一套沙箱、Bridge 与主题 token，不引入第二套协议。

**非目标**

- 不追求 Office 文档的像素级还原（图表、SmartArt、动画、嵌入 OLE 不做）。
- 不支持旧版二进制格式 `.doc/.xls/.ppt`（见 §8.4）。
- 不允许预览器访问任意文件：文件读取仍走插件权限与 Host 代理。
- 不替换 Quick Look 的外壳（标题栏、翻页、关闭）由 Core 提供。

## 2. 贡献模型

新增可选的 `contributions.preview` 数组：

```json
{
  "contributions": {
    "preview": [
      {
        "id": "office-preview.render",
        "title": "Office 预览",
        "extensions": ["docx", "xlsx", "pptx"],
        "priority": 100,
        "maxBytes": 26214400,
        "panel": {
          "entry": "ui/dist/index.html",
          "title": "Office 预览",
          "defaultSize": { "width": 900, "height": 640 }
        }
      }
    ]
  }
}
```

| 字段         | 必填 | 说明                                                                   |
| ------------ | ---- | ---------------------------------------------------------------------- |
| `id`         | 是   | 插件内唯一，解析结果里用 `<pluginId>:<id>` 标识                        |
| `title`      | 是   | 设置页与降级提示里显示的名字                                           |
| `extensions` | 是   | 小写扩展名数组（不带点），1–64 个，每个 ≤ 16 字符；不接受 `*`          |
| `priority`   | 否   | 整数 0–1000，默认 100；越大越优先                                      |
| `maxBytes`   | 否   | 超过此大小直接跳过该预览器；默认 64 MiB                                |
| `panel`      | 是   | 复用现有面板结构（`entry` / `title` / `defaultSize`），指向自包含 HTML |

约束与 `toolbar`/`contextMenus` 一致：`id` 插件内唯一、`entry` 必须包内相对路径、单文件 ≤ 1 MiB、面板尺寸受 320×220 … 720×680 夹取（Quick Look 会按窗口放宽到 92%×76%）。

## 3. 内置预览器

内置预览器实现为 Core 内部的固定条目，不通过 manifest 声明，但**参与同一套优先级解析**：

| id              | 扩展名                           | 默认优先级 | 说明                               |
| --------------- | -------------------------------- | ---------- | ---------------------------------- |
| `core.text`     | 所有文本类（现有 `isText` 判定） | 10         | 现有高亮预览                       |
| `core.image`    | png/jpg/gif/webp/svg…            | 20         | 现有图片预览                       |
| `core.media`    | mp3/mp4/wav/mov…                 | 30         | 现有媒体预览                       |
| `core.pdf`      | pdf                              | 40         | 现有 `PdfPreview`（pdfjs）         |
| `core.fallback` | 其余                             | 0          | 十六进制/元信息 + “用系统应用打开” |

插件声明同名扩展名且优先级更高即完成覆盖；内置条目的优先级固定，不随用户调整，只有“禁用插件预览器”会影响结果。

## 4. 解析与优先级

```
for extension in file.extensions:
    candidates = builtins(ext) + plugins(ext)
    candidates = [c for c in candidates if not user.disabled(c.id)]
    sort by: user.order index (若存在) → priority desc → id asc
    pick candidates[0]
```

- **用户顺序优先**：设置里拖动过的预览器按用户顺序排在前面，未拖动过的排在后面再按 `priority` 排序。
- **平局确定**：`priority` 相同则按 `<pluginId>:<id>` 字典序，保证跨机器可复现。
- **禁用**：`previewDisabled` 里的 id 直接剔除；全部被禁用时回落到 `core.fallback`。
- **不按扩展名单独配置**（v1）：一个预览器一个顺序，避免设置页出现 N×M 矩阵。若后续需要，可加 `priorityByExtension` 而不破坏现有字段。

解析结果缓存在内存里，`fiex:plugins-changed` / 设置变更时失效。

## 5. 预览会话与 Bridge

### 5.1 生命周期

```
Space / 双击预览
  → Core 解析预览器
  → 内置：现有组件（不变）
  → 插件：在预览外壳内挂载 sandbox iframe（opaque origin）
        → bridge 握手（复用 panel 协议）
        → Core 发送 preview.context
        → 插件后端解析文件，UI 渲染
  → 关闭 / 切换文件 / 超时 → 释放会话
```

- 插件预览与面板共用 `PluginPanelHost` 的握手、方法白名单与主题下发逻辑，只是容器从浮层换成预览外壳。
- 同一时间只有一个预览会话；切换文件时复用同一 iframe，发送 `preview.context` 而不是重建。

### 5.2 传给插件的上下文

```ts
// 新增 bridge 方法：preview.context
type PreviewContext = {
  path: string
  name: string
  extension: string
  size: number
  directory: string
  // 只读预览：不提供写入能力
}
```

- 文件内容由插件的 **Native 后端**通过现有 SDK 文件 API 读取（受 `permissions.files.read` 约束），不经过 iframe，避免把用户文件塞进消息通道。
- 纯 UI 插件（无后端）不能注册 `preview`：解析大文件需要进程隔离，也便于限时/限内存。manifest 校验时拒绝无 `runtime` 的 `preview` 条目。

### 5.3 降级与错误

- 插件返回错误、超时（默认 **4 秒**握手 + **10 秒**首屏）、或文件超过 `maxBytes` → 标记该预览器本次失败，自动尝试下一个候选；连续失败 3 次后本次会话不再尝试它。
- 最终回落到 `core.fallback`，并显示“无法预览，使用系统应用打开”。
- 加载失败不会写日志弹窗打断浏览；错误详情进状态中心。

## 6. 用户设置

设置 → **预览**（新分区）：

- 列出所有可用预览器：名称、来源（内置/插件名）、支持的扩展名、优先级。
- 每行：启用开关 + 上下拖动排序（复用已固定插件的拖动交互）。
- 顶部一行说明：“同一文件类型按列表从上到下尝试，失败会继续下一个。”
- 存储：`preferences.previewOrder: string[]`、`preferences.previewDisabled: string[]`；不写入插件目录。

默认顺序：内置条目按 §3 的优先级插入，插件按其声明 `priority` 插入，用户不动手也能得到合理结果。

## 7. Core 改动清单

| 模块                                                  | 改动                                                                               |
| ----------------------------------------------------- | ---------------------------------------------------------------------------------- |
| `src-tauri/src/plugins/preview.rs`（新）              | manifest 校验、内置+插件候选合并、优先级解析、`preview_registry()` 命令            |
| `src-tauri/src/plugins/host.rs`                       | `discover()` 接受 `preview` 贡献；无 runtime 的插件被拒；面板解析复用 `find_panel` |
| `src/lib/preview.ts`（新）                            | 前端注册表、排序、禁用、失败计数                                                   |
| `src/components/Preview.tsx`                          | 根据解析结果分流：内置组件 or `PreviewHost`                                        |
| `src/components/plugin-preview/PreviewHost.tsx`（新） | 预览外壳内的 iframe + bridge，复用 `attachFiexPanel`                               |
| `src/hooks/usePreferences.ts`                         | 新增两个字段与迁移默认值                                                           |
| `src/components/settings/PreviewSettings.tsx`（新）   | §6 的设置页                                                                        |
| SDK `ui/src/index.ts`                                 | `client.preview.context()` 类型；无新方法则复用 `preview.context`                  |

## 8. office-preview 插件

### 8.1 形态

- 独立 crate（`fiex-plugin-office-preview`）+ React 单文件面板；不做 VFS、不做连接。
- 声明：`permissions.files.read = true`、`runtime.protocolVersion = "2.0"`、`contributions.preview`（docx/xlsx/pptx）。
- 不申请网络、命令、写入权限。
- 目录：`official/office-preview/`，与其它官方插件同构；在 catalog 中新增 slug。

### 8.2 解析位置

解析放在 **Rust 后端**，UI 只负责排版与交互：

| 格式 | 解析方式                                       | 输出                                       |
| ---- | ---------------------------------------------- | ------------------------------------------ |
| docx | `zip` + `quick-xml` 读 `word/document.xml`     | 结构化 JSON：段落/样式/表格/图片（base64） |
| xlsx | `calamine`                                     | 工作表 + 单元格值/合并区域                 |
| pptx | `zip` + `quick-xml` 读 `ppt/slides/slideN.xml` | 每页元素（文本框/图片）与顺序              |

UI 侧不做 Office 解析，避免在 iframe 里引入 MB 级 JS 包，也便于统一做大小/时间上限。

### 8.3 交互

- 顶部工具条：文件名、格式徽标、缩放（docx）、工作表切换（xlsx）、上一页/下一页（pptx）。
- 所有颜色走宿主主题 token（见 [22 主题包开发](22-THEME-AUTHORING.md)）。
- 不支持的内容显示占位提示，而不是整篇失败（例如“图表未渲染”）。

### 8.4 明确不支持

- `.doc` / `.xls` / `.ppt`（OLE 复合二进制），只提示“请另存为新格式后预览”。
- 加密/带密码的 Office 文件：提示“受密码保护，无法预览”。
- 宏、外部链接、嵌入对象、动画与切换效果。

## 9. 测试计划

**Core 单元测试**

- 解析：同扩展名多个候选按 priority 排序；平局按 id；用户顺序覆盖；禁用回落到 fallback；无 runtime 的 preview 条目被拒；扩展名大小写/非法字符校验。
- 内置条目优先级固定、插件可覆盖 `pdf`。
- `maxBytes` 超限被跳过。

**Core e2e**

- 用 `?pluginFixture` 增加一个预览夹具插件（docx 扩展名），断言：空格预览打开插件 iframe、收到 `preview.context`、切换文件不重建 iframe。
- 禁用该预览器后再次预览，回落到文本/fallback。
- 失败注入：插件抛错 → 自动降级，不阻塞浏览。

**office-preview 插件测试**

- Rust：固定 fixture（docx/xlsx/pptx 各一个）解析出的段落数/工作表数/页数断言；超大文件拒绝；损坏文件返回明确错误；密码文件报错。
- UI：构建产物单文件、无外部请求。
- Core integration：打包后通过真实 Host 打开三个 fixture 预览。

## 10. 实施顺序

1. **P0**：manifest 校验 + 解析器 + 设置页（只支持禁用/排序）+ e2e 夹具预览器。
2. **P1**：`office-preview` 插件（docx → xlsx → pptx 顺序做），先只读渲染。
3. **P2**：降级体验（超时/失败计数）、缩放与工作表切换、主题细节。
4. **P3**（可选）：`priorityByExtension`、预览器能力声明（是否支持多页/缩放）。

## 11. 待确认

- 插件预览是否允许**只在桌面端**启用（浏览器 demo 无插件宿主，回落到内置）——建议是。
- 是否需要“默认用系统应用打开”的开关；当前设计把它作为 fallback 的最后一步。
- 预览器是否需要声明 `mime`（而不只是扩展名）；建议 v1 只用扩展名，避免 manifest 冗余。
