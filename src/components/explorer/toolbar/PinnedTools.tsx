import { Button } from '@/components/ui/button'
import type { AppController } from '@/hooks/useAppController'
import { usePluginCatalog, type PluginContribution, type PluginInfo } from '@/lib/plugins'
import { call } from '@/lib/api'
import { OPEN_PLUGIN_PANEL } from '@/components/plugins/PluginPanelHost'
import { useEffect, useRef } from 'react'
import { createPortal } from 'react-dom'
import { PluginMenu, type PluginToolEntry } from './ToolbarMenus'
import { pluginIcon, pluginKey } from './pluginTools'
import { usePinnedDrag } from './usePinnedDrag'

type PinnedToolsProps = {
  prefs: Pick<AppController['prefs'], 'pinnedPluginTools'>
  update: AppController['update']
  x: Pick<AppController['x'], 'selected' | 'path' | 'setError'>
}

/** Pinned plugin tool strip plus the plugin dropdown menu. The strip owns its
 *  drag/reorder state so the toolbar root only has to place it. */
export function PinnedTools({ prefs, update, x }: PinnedToolsProps) {
  const plugins = usePluginCatalog()
  const lastPointer = useRef({ x: window.innerWidth - 18, y: 96 })
  const pinnedKeys = prefs.pinnedPluginTools || []
  const availableTools: PluginToolEntry[] = plugins.flatMap((plugin) =>
    plugin.enabled && plugin.extensions?.toolbar
      ? [{ plugin, tool: plugin.extensions.toolbar }]
      : [],
  )
  const pinnedTools = pinnedKeys.flatMap((key) => {
    const found = availableTools.find(({ plugin }) => pluginKey(plugin.id) === key)
    return found ? [found] : []
  })
  const pinTool = (key: string) => {
    if (pinnedKeys.includes(key)) return
    update({ pinnedPluginTools: [...pinnedKeys, key] })
  }
  const unpinTool = (key: string) => {
    update({ pinnedPluginTools: pinnedKeys.filter((item) => item !== key) })
  }
  const autoPinned = useRef(false)
  useEffect(() => {
    if (!import.meta.env.DEV || autoPinned.current) return
    if (new URLSearchParams(location.search).has('pluginFixture')) return
    const defaults = availableTools
      .filter(({ plugin, tool }) => plugin.id.startsWith('org.fiex.') && tool.panel)
      .map(({ plugin }) => pluginKey(plugin.id))
    if (!defaults.length) return
    autoPinned.current = true
    const missing = defaults.filter((key) => !pinnedKeys.includes(key))
    if (missing.length) update({ pinnedPluginTools: [...pinnedKeys, ...missing] })
  }, [availableTools, pinnedKeys, update])
  useEffect(() => {
    const track = (event: globalThis.PointerEvent) => {
      lastPointer.current = { x: event.clientX, y: event.clientY }
    }
    window.addEventListener('pointermove', track, true)
    window.addEventListener('pointerdown', track, true)
    return () => {
      window.removeEventListener('pointermove', track, true)
      window.removeEventListener('pointerdown', track, true)
    }
  }, [])
  const runTool = (
    plugin: PluginInfo,
    tool: PluginContribution,
    anchor?: { x: number; y: number },
  ) => {
    if (tool.panel) {
      window.dispatchEvent(
        new CustomEvent(OPEN_PLUGIN_PANEL, {
          detail: {
            pluginId: plugin.id,
            panelId: plugin.id,
            title: tool.title,
            icon: tool.icon,
            selection: x.selected,
            directory: x.path,
            anchor: anchor ?? lastPointer.current,
            source: 'toolbar',
          },
        }),
      )
      return
    }
    if (plugin.backendAvailable) {
      void call('plugin_invoke', {
        pluginId: plugin.id,
        method: 'toolbar.invoke',
        params: { root: x.path, selection: x.selected },
        directory: x.path,
        selectedPaths: x.selected.length ? x.selected : [x.path],
      }).catch((error) => x.setError(String(error)))
      return
    }
    x.setError('插件后端不可用')
  }
  const { pinStrip, pinDrag, pinDragged, beginPinDrag, pinShift } = usePinnedDrag({
    pinnedKeys: prefs.pinnedPluginTools,
    update,
  })
  const liftedPin = pinDrag && pinnedTools[pinDrag.from] ? pinDrag : null
  if (!availableTools.length) return null
  return (
    <>
      <div className="toolbar-divider" />
      <div className="plugin-toolbar-group" aria-label="插件工具">
        <div className="plugin-pinned-strip" ref={pinStrip}>
          {pinnedTools.map(({ plugin, tool }, index) => {
            const key = pluginKey(plugin.id)
            return (
              <Button
                key={key}
                variant="ghost"
                size="icon"
                className={`plugin-tool-button ${pinDrag?.from === index ? 'dragging' : ''}`}
                style={{ transform: pinShift(index) }}
                title={`${plugin.name} · ${tool.title}（拖动排序，右键取消固定）`}
                aria-label={`${plugin.name} ${tool.title}`}
                onPointerDown={(event) => beginPinDrag(event, index)}
                onContextMenu={(event) => {
                  event.preventDefault()
                  unpinTool(key)
                }}
                onClick={(event) => {
                  if (!pinDragged.current)
                    runTool(plugin, tool, { x: event.clientX, y: event.clientY })
                }}
              >
                {pluginIcon(tool.icon)}
              </Button>
            )
          })}
        </div>
        <PluginMenu
          availableTools={availableTools}
          pinnedKeys={pinnedKeys}
          runTool={runTool}
          pinTool={pinTool}
          unpinTool={unpinTool}
        />
      </div>
      <div className="toolbar-divider" />
      {liftedPin &&
        createPortal(
          <div
            className={`plugin-tool-button plugin-tool-ghost ${liftedPin.settling ? 'settling' : ''}`}
            style={{
              width: liftedPin.width,
              height: liftedPin.height,
              transform: `translate3d(${liftedPin.left}px, ${liftedPin.top}px, 0) scale(1.08)`,
            }}
          >
            {pluginIcon(pinnedTools[liftedPin.from]?.tool.icon)}
          </div>,
          document.body,
        )}
    </>
  )
}
