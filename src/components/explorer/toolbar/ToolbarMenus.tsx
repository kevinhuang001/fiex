import { Action, Label, Menu } from '@/components/Menus'
import * as D from '@/components/ui/dropdown-menu'
import { useMenuPosition } from '@/lib/menuPosition'
import type { AppController } from '@/hooks/useAppController'
import type { PluginContribution, PluginInfo } from '@/lib/plugins'
import type { Ide, Terminal } from '@/lib/types'
import { PuzzlePieceIcon as Puzzle } from '@phosphor-icons/react/dist/csr/PuzzlePiece'
import { Pin, Settings2 } from 'lucide-react'
import { useEffect, useRef, useState, type RefObject } from 'react'
import { pluginIcon, pluginKey } from './pluginTools'

export type ToolbarPickerState = { kind: 'terminal' | 'ide'; x: number; y: number } | null

export type PluginToolEntry = { plugin: PluginInfo; tool: PluginContribution }

/** One shared picker state for the IDE and terminal buttons: right-clicking
 *  either anchor opens the same context menu at the pointer. */
export function useToolbarPicker() {
  const [picker, setPicker] = useState<ToolbarPickerState>(null)
  const pickerRef = useRef<HTMLDivElement>(null)
  const pickerPosition = useMenuPosition(picker?.x ?? 0, picker?.y ?? 0, pickerRef, Boolean(picker))
  useEffect(() => {
    if (!picker) return
    const close = (event: Event) => {
      if ((event.target as Element | null)?.closest('.toolbar-picker')) return
      setPicker(null)
    }
    window.addEventListener('pointerdown', close)
    window.addEventListener('blur', close)
    return () => {
      window.removeEventListener('pointerdown', close)
      window.removeEventListener('blur', close)
    }
  }, [picker])
  return {
    picker,
    pickerRef,
    pickerPosition,
    openPicker: (kind: 'terminal' | 'ide', x: number, y: number) => setPicker({ kind, x, y }),
    closePicker: () => setPicker(null),
  }
}

type ToolbarPickerMenuProps = {
  picker: Exclude<ToolbarPickerState, null>
  pickerRef: RefObject<HTMLDivElement | null>
  pickerPosition: { left: number; top: number }
  closePicker: () => void
  prefs: Pick<AppController['prefs'], 'ide' | 'terminal'>
  update: AppController['update']
  ides: Ide[]
  terminals: Terminal[]
}

export function ToolbarPickerMenu({
  picker,
  pickerRef,
  pickerPosition,
  closePicker,
  prefs,
  update,
  ides,
  terminals,
}: ToolbarPickerMenuProps) {
  const items =
    picker.kind === 'ide' ? ides : [{ id: 'builtin', name: 'Fiex 内置终端' }, ...terminals]
  return (
    <div
      ref={pickerRef}
      className="context-menu toolbar-picker"
      role="menu"
      aria-label={picker.kind === 'ide' ? '选择 IDE' : '选择终端'}
      style={pickerPosition}
    >
      {items.map((item) => (
        <button
          role="menuitem"
          key={item.id}
          data-active={
            (picker.kind === 'ide' ? prefs.ide : prefs.terminal) === item.id || undefined
          }
          onClick={() => {
            update(picker.kind === 'ide' ? { ide: item.id } : { terminal: item.id })
            closePicker()
          }}
        >
          {item.name}
        </button>
      ))}
      {items.length === 0 && (
        <button role="menuitem" disabled>
          未检测到可用应用
        </button>
      )}
    </div>
  )
}

type PluginMenuProps = {
  availableTools: PluginToolEntry[]
  pinnedKeys: string[]
  runTool: (plugin: PluginInfo, tool: PluginContribution, anchor?: { x: number; y: number }) => void
  pinTool: (key: string) => void
  unpinTool: (key: string) => void
  onManage: () => void
}

export function PluginMenu({
  availableTools,
  pinnedKeys,
  runTool,
  pinTool,
  unpinTool,
  onManage,
}: PluginMenuProps) {
  return (
    <Menu label="插件" icon={<Puzzle weight="regular" size={20} />} align="center">
      <div className="plugin-menu-header">
        <Label>插件工具</Label>
        <D.DropdownMenuItem
          className="plugin-menu-settings"
          aria-label="插件设置"
          title="插件设置"
          onSelect={onManage}
        >
          <Settings2 size={14} />
        </D.DropdownMenuItem>
      </div>
      {availableTools.map(({ plugin, tool }) => {
        const key = pluginKey(plugin.id)
        const pinned = pinnedKeys.includes(key)
        return (
          <Action
            key={key}
            onClick={() => runTool(plugin, tool)}
            trailing={
              <button
                type="button"
                className={`plugin-pin ${pinned ? 'active' : ''}`}
                aria-label={`${pinned ? '取消固定' : '固定'} ${plugin.name} ${tool.title}`}
                title={pinned ? '取消固定' : '固定到工具栏'}
                onClick={() => (pinned ? unpinTool(key) : pinTool(key))}
              >
                <Pin size={13} fill={pinned ? 'currentColor' : 'none'} />
              </button>
            }
          >
            <span className="plugin-menu-entry">
              <span className="plugin-menu-icon">{pluginIcon(tool.icon)}</span>
              <span className="plugin-menu-label">
                {plugin.name} · {tool.title}
              </span>
            </span>
          </Action>
        )
      })}
      {!availableTools.length && <Label>暂无可用的插件工具</Label>}
    </Menu>
  )
}
