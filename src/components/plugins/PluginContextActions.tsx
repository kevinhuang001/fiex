import { t } from '@/i18n'
import { useContext, useEffect, useLayoutEffect, useRef, useState } from 'react'
import { ChevronRight, Puzzle } from 'lucide-react'
import { call } from '@/lib/api'
import { measureSubmenu } from '@/lib/menuPosition'
import { usePluginCatalog } from '@/lib/plugins'
import {
  beginActivity,
  finishActivity,
  quietProgressChannel,
  updateActivity,
} from '@/hooks/useActivity'
import { OPEN_PLUGIN_PANEL, PLUGIN_RESULT_EVENT } from '@/components/plugins/PluginPanelHost'
import {
  PluginMenuNode,
  SubmenuGroupContext,
  validPluginMenuItems,
  type PluginMenu,
  type PluginMenuItem,
} from './PluginMenuNode'

interface Context {
  paths: string[]
  path: string
  menus: PluginMenu[]
  loading: boolean
  error: string
  onRetry: () => void
}

const PLUGIN_MENU_ID = 'plugin-actions'

export function PluginContextActions({
  paths,
  path,
  menus: resolvedMenus,
  loading,
  error,
  onRetry,
  onClose,
  onError,
}: Context & { onClose: () => void; onError: (error: string) => void }) {
  const plugins = usePluginCatalog()
  const submenuGroup = useContext(SubmenuGroupContext)
  const setGroupOpenId = submenuGroup?.setOpenId
  const [menus, setMenus] = useState<PluginMenu[]>([])
  const [localOpen, setLocalOpen] = useState(false)
  const open = submenuGroup ? submenuGroup.openId === PLUGIN_MENU_ID : localOpen
  const setExpanded = (value: boolean) =>
    setGroupOpenId
      ? setGroupOpenId((current) =>
          value ? PLUGIN_MENU_ID : current === PLUGIN_MENU_ID ? null : current,
        )
      : setLocalOpen(value)
  const [openChild, setOpenChild] = useState<string | null>(null)
  const [flip, setFlip] = useState({ left: false, up: false, maxWidth: 420 })
  const requestedSubmenus = useRef(new Set<string>())
  const trigger = useRef<HTMLButtonElement>(null)
  const submenu = useRef<HTMLDivElement>(null)
  const closeTimer = useRef<number | null>(null)
  const focusFirst = useRef(false)
  const selectionKey = JSON.stringify([path, paths])
  const cancelClose = () => {
    if (closeTimer.current !== null) window.clearTimeout(closeTimer.current)
    closeTimer.current = null
  }
  const scheduleClose = () => {
    cancelClose()
    // Let the pointer cross the gap before closing the submenu.
    closeTimer.current = window.setTimeout(() => setExpanded(false), 500)
  }
  useEffect(() => cancelClose, [])
  useEffect(() => {
    if (!open) setOpenChild(null)
  }, [open])
  useEffect(() => {
    requestedSubmenus.current.clear()
    setGroupOpenId?.((current) => (current === PLUGIN_MENU_ID ? null : current))
    setLocalOpen(false)
    setOpenChild(null)
    setFlip({ left: false, up: false, maxWidth: 420 })
  }, [selectionKey, setGroupOpenId])
  useEffect(() => {
    setMenus((previous) => (previous === resolvedMenus ? previous : resolvedMenus))
  }, [resolvedMenus])
  useLayoutEffect(() => {
    if (!open || !submenu.current) return
    if (focusFirst.current) {
      focusFirst.current = false
      submenu.current.querySelector<HTMLElement>('[role="menuitem"]:not([disabled])')?.focus()
    }
    const anchor = trigger.current
    if (!anchor) return
    const preferLeft = anchor.getBoundingClientRect().left < window.innerWidth / 2
    const placement = measureSubmenu(anchor, submenu.current, preferLeft ? 'left' : 'right')
    setFlip((current) =>
      current.left === placement.left &&
      current.up === placement.up &&
      current.maxWidth === placement.maxWidth
        ? current
        : placement,
    )
  }, [open, menus, loading, flip.maxWidth])

  const resolveSubmenu = async (menu: PluginMenu, submenuId: string) => {
    const key = menu.menuSessionId + ':' + submenuId
    if (requestedSubmenus.current.has(key)) return
    requestedSubmenus.current.add(key)
    try {
      const response = await call('plugin_context_menu_children', {
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
      const response = await call('plugin_context_menu_execute', {
        menuSessionId: menu.menuSessionId,
        itemId: item.id,
        onProgress: quietProgressChannel(),
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
        plugin?.id,
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
  return (
    <div
      className="context-submenu-wrap plugin-context-actions"
      onMouseEnter={() => {
        cancelClose()
        setExpanded(true)
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
          setExpanded(true)
        }}
        onKeyDown={(event) => {
          if (event.key === 'ArrowRight' || event.key === 'ArrowDown') {
            event.preventDefault()
            event.stopPropagation()
            focusFirst.current = true
            setExpanded(true)
          } else if (event.key === 'Escape' && open) {
            event.stopPropagation()
            setExpanded(false)
          }
        }}
      >
        <Puzzle size={15} />
        <span className="context-submenu-label">{t('插件')}</span>
        <ChevronRight size={13} className="submenu-arrow" />
      </button>
      {open && (
        <div
          ref={submenu}
          role="menu"
          aria-label={t('插件操作')}
          className={`context-submenu plugin-context-menu${flip.left ? ' open-left' : ''}${flip.up ? ' plugin-context-menu-up' : ''}`}
          style={{ maxWidth: flip.maxWidth }}
          onMouseEnter={cancelClose}
          onMouseLeave={scheduleClose}
          onKeyDown={(event) => {
            if (event.key === 'ArrowLeft' || event.key === 'Escape') {
              event.preventDefault()
              event.stopPropagation()
              setExpanded(false)
              trigger.current?.focus()
            }
          }}
        >
          {loading ? (
            <p className="context-submenu-empty">{t('正在加载插件操作…')}</p>
          ) : error ? (
            <div className="context-submenu-empty">
              <p>
                {t('加载插件操作失败')}: {error}
              </p>
              <button role="menuitem" onClick={onRetry}>
                {t('重试')}
              </button>
            </div>
          ) : hasItems ? (
            <SubmenuGroupContext.Provider value={{ openId: openChild, setOpenId: setOpenChild }}>
              {menus.flatMap((menu) =>
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
              )}
            </SubmenuGroupContext.Provider>
          ) : (
            <p className="context-submenu-empty">{t('无可用插件操作')}</p>
          )}
        </div>
      )}
    </div>
  )
}
