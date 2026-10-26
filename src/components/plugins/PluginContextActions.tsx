import { t } from '@/i18n'
import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import { ChevronRight, Puzzle } from 'lucide-react'
import { call } from '@/lib/api'
import { usePluginCatalog } from '@/lib/plugins'
import { beginActivity, finishActivity, updateActivity } from '@/hooks/useActivity'
import { OPEN_PLUGIN_PANEL, PLUGIN_RESULT_EVENT } from '@/components/plugins/PluginPanelHost'
import {
  PluginMenuNode,
  validPluginMenuItems,
  type PluginMenu,
  type PluginMenuItem,
} from './PluginMenuNode'

interface Context {
  paths: string[]
  path: string
  isDirectory: boolean
  readOnly: boolean
  isRemote: boolean
}

export function PluginContextActions({
  paths,
  path,
  isDirectory,
  readOnly,
  isRemote,
  onClose,
  onError,
}: Context & { onClose: () => void; onError: (error: string) => void }) {
  const plugins = usePluginCatalog()
  const [menus, setMenus] = useState<PluginMenu[]>([])
  const [open, setOpen] = useState(false)
  const [flip, setFlip] = useState({ left: false, up: false })
  const requestedSubmenus = useRef(new Set<string>())
  const trigger = useRef<HTMLButtonElement>(null)
  const submenu = useRef<HTMLDivElement>(null)
  const closeTimer = useRef<number | null>(null)
  const focusFirst = useRef(false)
  const selectionKey = JSON.stringify(paths)
  const cancelClose = () => {
    if (closeTimer.current !== null) window.clearTimeout(closeTimer.current)
    closeTimer.current = null
  }
  const scheduleClose = () => {
    cancelClose()
    // Leaving the plugin trigger closes the group at once; nested panels manage
    // their own hover state and must not overlap a sibling plugin.
    setOpen(false)
  }
  useEffect(() => cancelClose, [])
  // Resolve every plugin's menu as soon as the root menu opens, so the
  // submenu is already populated when the user hovers it. The backend resolves
  // active providers itself; the catalog below only supplies names and icons,
  // so loading it must not trigger a second resolve that could close the menu.
  useEffect(() => {
    let active = true
    requestedSubmenus.current.clear()
    setMenus([])
    setOpen(false)
    setFlip({ left: false, up: false })
    void call<PluginMenu[]>('plugin_context_menu_resolve', {
      directory: path,
      paths: JSON.parse(selectionKey) as string[],
      isDirectory,
      readOnly,
      isRemote,
    })
      .then((menus) => {
        if (active) setMenus(Array.isArray(menus) ? menus : [])
      })
      .catch(() => {
        if (active) setMenus([])
      })
    return () => {
      active = false
    }
  }, [path, selectionKey, isDirectory, readOnly, isRemote])
  useLayoutEffect(() => {
    if (!open || !submenu.current) return
    if (focusFirst.current) {
      focusFirst.current = false
      submenu.current.querySelector<HTMLElement>('[role="menuitem"]:not([disabled])')?.focus()
    }
    // Measure against the trigger, not the already-flipped submenu, so a later
    // lazy submenu update cannot flip the panel back off screen.
    const anchor = trigger.current?.getBoundingClientRect()
    if (!anchor) return
    const width = submenu.current.offsetWidth
    const height = submenu.current.offsetHeight
    // Submenu levels are flush; measure their real edge for flipping.
    const side = 0
    const preferLeft = anchor.left < window.innerWidth / 2
    const roomLeft = anchor.left - side - width >= 8
    const left = roomLeft && (preferLeft || anchor.right + side + width > window.innerWidth - 8)
    const up = anchor.top - 6 + height > window.innerHeight - 8 && anchor.bottom + 6 - height >= 8
    setFlip((current) => (current.left === left && current.up === up ? current : { left, up }))
  }, [open, menus])

  const resolveSubmenu = async (menu: PluginMenu, submenuId: string) => {
    const key = menu.menuSessionId + ':' + submenuId
    if (requestedSubmenus.current.has(key)) return
    requestedSubmenus.current.add(key)
    try {
      const response = await call<{ items: unknown }>('plugin_context_menu_children', {
        menuSessionId: menu.menuSessionId,
        parentId: submenuId,
      })
      const children = response.items
      if (!Array.isArray(children)) throw new Error('Invalid submenu response')
      setMenus((previous) =>
        previous.map((candidate) => {
          if (candidate.menuSessionId !== menu.menuSessionId) return candidate
          const updated = [...candidate.items, ...children]
          return validPluginMenuItems(updated) ? { ...candidate, items: updated } : candidate
        }),
      )
    } catch (error) {
      requestedSubmenus.current.delete(key)
      onError(String(error))
    }
  }

  const invoke = async (
    menu: PluginMenu,
    item: PluginMenuItem,
    anchor?: { x: number; y: number },
  ) => {
    onClose()
    const pluginId = menu.pluginId
    const plugin = plugins.find((entry) => entry.id === pluginId)
    let activityId = ''
    try {
      // No content permission is handed out during resolve; executing a real
      // click enters the operation with its exact selected paths.
      const response = await call<unknown>('plugin_context_menu_execute', {
        menuSessionId: menu.menuSessionId,
        itemId: item.id,
      })
      if (
        response &&
        typeof response === 'object' &&
        'kind' in response &&
        response.kind === 'panel' &&
        'assetPath' in response &&
        typeof response.assetPath === 'string'
      ) {
        const entry = response as {
          assetPath: string
          panelId?: string
          title?: string
          size?: { width: number; height: number }
        }
        window.dispatchEvent(
          new CustomEvent(OPEN_PLUGIN_PANEL, {
            detail: {
              pluginId,
              panelId: entry.panelId || item.id,
              title: entry.title || item.title,
              icon: plugin?.icon,
              selection: paths,
              directory: path,
              anchor,
              source: 'contextMenu',
              size: entry.size,
              assetPath: entry.assetPath,
              timeoutMs: plugin?.uiTimeoutMs,
              allowCreate: (plugin?.declaredPermissions || []).some(
                (permission) => permission === 'files.create' || permission === 'folders.create',
              ),
            },
          }),
        )
        return
      }
      activityId = beginActivity(
        (plugin?.name || t('插件')) + ' · ' + item.title,
        paths.length + t(' 个项目'),
        undefined,
        'plugin',
      )
      updateActivity(activityId, { indeterminate: true })
      finishActivity(activityId, undefined, t('已提交'))
      // Background tasks return a taskId; the Status Center owns their
      // progress, so never open a raw JSON result dialog for them.
      const silent =
        response && typeof response === 'object' && ('taskId' in response || 'handled' in response)
      if (!silent && response && typeof response === 'object') {
        window.dispatchEvent(
          new CustomEvent(PLUGIN_RESULT_EVENT, { detail: { title: item.title, result: response } }),
        )
      }
    } catch (error) {
      if (activityId) finishActivity(activityId, String(error))
      onError(String(error))
    }
  }

  const hasItems = menus.some((menu) => menu.items.some((item) => !item.parentId))
  // Keep the entry visible for installed-but-inactive plugins too; the panel
  // explains why no actions resolved instead of making the whole menu vanish.
  const hasContextPlugins = plugins.length > 0
  if (!hasItems && !hasContextPlugins) return null
  return (
    <div
      className="context-submenu-wrap plugin-context-actions"
      onMouseEnter={() => {
        cancelClose()
        setOpen(true)
      }}
      onMouseLeave={scheduleClose}
    >
      <button
        ref={trigger}
        role="menuitem"
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={(event) => {
          event.stopPropagation()
          cancelClose()
          // Hover already opens the menu; a click must not immediately close it.
          setOpen(true)
        }}
        onKeyDown={(event) => {
          if (event.key === 'ArrowRight' || event.key === 'ArrowDown') {
            event.preventDefault()
            event.stopPropagation()
            focusFirst.current = true
            setOpen(true)
          } else if (event.key === 'Escape' && open) {
            event.stopPropagation()
            setOpen(false)
          }
        }}
      >
        <Puzzle size={15} />
        {t('插件')}
        <ChevronRight size={13} className="submenu-arrow" />
      </button>
      {open && (
        <div
          ref={submenu}
          role="menu"
          aria-label={t('插件操作')}
          className={`context-submenu plugin-context-menu${flip.left ? ' open-left' : ''}${flip.up ? ' plugin-context-menu-up' : ''}`}
          onMouseEnter={cancelClose}
          onMouseLeave={scheduleClose}
          onKeyDown={(event) => {
            if (event.key === 'ArrowLeft' || event.key === 'Escape') {
              event.preventDefault()
              event.stopPropagation()
              setOpen(false)
              trigger.current?.focus()
            }
          }}
        >
          {hasItems ? (
            menus.flatMap((menu) =>
              menu.items
                .filter((item) => !item.parentId)
                .map((item) => (
                  <PluginMenuNode
                    key={menu.pluginId + ':' + item.id}
                    item={item}
                    items={menu.items}
                    parentLeft={flip.left}
                    preferLeft={
                      flip.left ||
                      (trigger.current?.getBoundingClientRect().left ?? 0) < window.innerWidth / 2
                    }
                    onAction={(selected, anchor) => void invoke(menu, selected, anchor)}
                    onExpand={(submenuId) => void resolveSubmenu(menu, submenuId)}
                  />
                )),
            )
          ) : (
            <p className="context-submenu-empty">
              {t('没有可用的插件操作，请确认插件已激活且是新版安装包。')}
            </p>
          )}
        </div>
      )}
    </div>
  )
}
