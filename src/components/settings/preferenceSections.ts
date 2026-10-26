import {
  Filter,
  Info,
  Keyboard,
  Palette,
  PanelLeft,
  Rocket,
  Tags,
  TerminalSquare,
  type LucideIcon,
} from 'lucide-react'
import { t } from '@/i18n'
export type SectionId =
  'appearance' | 'behavior' | 'shortcuts' | 'ignore' | 'sidebar' | 'tags' | 'terminal' | 'about'

interface Section {
  id: SectionId
  label: string
  icon: LucideIcon
  color: string
  keywords: string[]
}

export const sections: Section[] = [
  {
    id: 'appearance',
    get label() {
      return t('外观')
    },
    icon: Palette,
    color: '#3b82f6',
    keywords: ['theme', 'dark', 'light', '主题', '深色', '浅色', '图标'],
  },
  {
    id: 'behavior',
    get label() {
      return t('启动与显示')
    },
    icon: Rocket,
    color: '#14b8a6',
    keywords: ['behavior', 'startup', '启动', '隐藏文件', '标签页', '行为'],
  },
  {
    id: 'shortcuts',
    get label() {
      return t('快捷键')
    },
    icon: Keyboard,
    color: '#8b5cf6',
    keywords: ['keyboard', 'shortcut', '快捷键', '按键'],
  },
  {
    id: 'sidebar',
    get label() {
      return t('侧边栏')
    },
    icon: PanelLeft,
    color: '#5b9cf8',
    keywords: ['sidebar', '侧边栏', '常用位置', '收藏', '设备', '连接', '标签', '顺序'],
  },
  {
    id: 'tags',
    get label() {
      return t('标签')
    },
    icon: Tags,
    color: '#ec4899',
    keywords: ['tag', 'tags', '标签', '颜色', '自定义'],
  },
  {
    id: 'ignore',
    get label() {
      return t('忽略')
    },
    icon: Filter,
    color: '#f59e0b',
    keywords: ['ignore', '忽略', 'node_modules', '.ds_store', '压缩', '复制'],
  },
  {
    id: 'terminal',
    get label() {
      return t('启动方式')
    },
    icon: TerminalSquare,
    color: '#10b981',
    keywords: ['terminal', 'shell', 'ide', '终端', '命令行', '启动', '编辑器'],
  },
  {
    id: 'about',
    get label() {
      return t('关于')
    },
    icon: Info,
    color: '#64748b',
    keywords: ['about', 'version', 'license', '关于', '版本', '许可'],
  },
]

type SearchEntry = { section: SectionId; label: string; keywords: string[] }

export const searchEntries: SearchEntry[] = [
  {
    section: 'appearance',
    get label() {
      return t('主题')
    },
    keywords: ['theme', '主题', '皮肤'],
  },
  {
    section: 'appearance',
    get label() {
      return t('外观模式')
    },
    keywords: ['dark', 'light', '深色', '浅色', '跟随系统'],
  },
  {
    section: 'behavior',
    get label() {
      return t('显示隐藏文件')
    },
    keywords: ['hidden', '隐藏文件', '.ds_store'],
  },
  {
    section: 'behavior',
    get label() {
      return t('启动页')
    },
    keywords: ['start', '启动', '首页'],
  },
  {
    section: 'behavior',
    get label() {
      return t('启动时恢复标签页')
    },
    keywords: ['tab', 'restore', '恢复', '标签页', '会话'],
  },
  {
    section: 'appearance',
    get label() {
      return t('字体大小')
    },
    keywords: ['font', '字号', '缩放', '大小'],
  },
  {
    section: 'sidebar',
    get label() {
      return t('区域顺序')
    },
    keywords: ['sidebar', '顺序', '排序', '区域'],
  },
  {
    section: 'sidebar',
    get label() {
      return t('侧边栏分区')
    },
    keywords: ['显示', '隐藏', '标签', '设备', '连接'],
  },
  {
    section: 'sidebar',
    get label() {
      return t('常用位置')
    },
    keywords: ['places', '位置', '显示'],
  },
  {
    section: 'shortcuts',
    get label() {
      return t('快捷键方案')
    },
    keywords: ['shortcut', 'preset', '方案', '按键'],
  },
  {
    section: 'shortcuts',
    get label() {
      return t('自定义快捷键')
    },
    keywords: ['shortcut', '自定义', '按键'],
  },
  {
    section: 'tags',
    get label() {
      return t('标签列表')
    },
    keywords: ['tag', '标签', '颜色'],
  },
  {
    section: 'tags',
    get label() {
      return t('添加标签')
    },
    keywords: ['tag', '新建', '颜色'],
  },
  {
    section: 'ignore',
    get label() {
      return t('忽略规则')
    },
    keywords: ['ignore', '忽略', 'node_modules'],
  },
  {
    section: 'terminal',
    get label() {
      return t('终端')
    },
    keywords: ['terminal', 'shell', '命令行'],
  },
  { section: 'terminal', label: 'IDE', keywords: ['editor', '编辑器', '代码'] },
  {
    section: 'about',
    get label() {
      return t('版本信息')
    },
    keywords: ['version', '版本', '许可', 'license'],
  },
]
