import { FileViews } from '@/components/browser/FileViews'
import { BatchRenameDialog } from '@/components/BatchRenameDialog'
import { ExplorerDialogs } from '@/components/explorer/ExplorerDialogs'
import { ExplorerPathBar } from '@/components/explorer/ExplorerPathBar'
import { ExplorerStatusBar } from '@/components/explorer/ExplorerStatusBar'
import { ActivityCenter } from '@/components/ActivityCenter'
import { SecondaryPane } from '@/components/SecondaryPane'
import { WorkspaceBar } from '@/components/WorkspaceBar'
import { ExplorerTabs } from '@/components/explorer/ExplorerTabs'
import { ExplorerToolbar } from '@/components/explorer/ExplorerToolbar'
import { FileContextMenu } from '@/components/explorer/FileContextMenu'
import { Sidebar } from '@/components/Sidebar'
import { PluginConnectionView } from '@/components/plugins/PluginConnectionView'
import { SshTerminalView } from '@/components/ssh/SshTerminalView'
import { useAppController } from '@/hooks/useAppController'
import { usePluginTasks } from '@/hooks/usePluginTasks'
import { asset, call, native } from '@/lib/api'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
import { Download, Info, LoaderCircle, X } from 'lucide-react'
import { useEffect, useRef } from 'react'
import type { CSSProperties } from 'react'
export default function App() {
  const app = useAppController()
  usePluginTasks()
  const pluginViewOf = (path: string) =>
    /^fiex-plugin-view:\/\/([A-Za-z0-9-]{1,128})$/.exec(path)?.[1]
  const terminalOf = (path: string) => /^fiex-terminal:\/\/([A-Za-z0-9-]{1,128})$/.exec(path)?.[1]
  const pluginViewId = pluginViewOf(app.left.path)
  const terminalId = terminalOf(app.left.path)
  const fullView = Boolean(pluginViewId || terminalId)
  const fullViews = app.left.tabs.flatMap((path, index) => {
    const terminal = terminalOf(path)
    const plugin = pluginViewOf(path)
    return terminal || plugin ? [{ path, index, terminal, plugin }] : []
  })
  const autostarted = useRef(false)
  const backgroundPath = app.prefs.backgroundImage
  const backgroundUrl = backgroundPath
    ? native && !backgroundPath.includes('://')
      ? asset(backgroundPath)
      : backgroundPath
    : ''
  const backgroundImage = backgroundUrl ? `url("${backgroundUrl.replace(/"/g, '%22')}")` : undefined

  useEffect(() => {
    const navigate = (event: Event) => {
      const detail = (event as CustomEvent<{ path?: string; pane?: string }>).detail
      if (!detail?.path) return
      app.left.navigate(detail.path)
    }
    window.addEventListener('fiex:navigate', navigate)
    return () => window.removeEventListener('fiex:navigate', navigate)
  }, [app.left])

  useEffect(() => {
    const openStore = () => app.setSettings(true)
    window.addEventListener('fiex:open-plugin-store', openStore)
    return () => window.removeEventListener('fiex:open-plugin-store', openStore)
  }, [app.setSettings])

  useEffect(() => {
    if (!native || autostarted.current || !app.left.hasListing) return
    autostarted.current = true
    const sessions = app.left.connections.filter(
      (connection) =>
        connection.kind === 'ssh' &&
        connection.autostart &&
        connection.saved !== false &&
        !app.left.tabs.includes(`fiex-terminal://${connection.id}`),
    )
    for (const session of sessions) {
      app.newWorkspaceTab()
      app.left.navigate(`fiex-terminal://${session.id}`)
    }
  }, [app.left.hasListing, app.left.connections, app.left.tabs])

  // The label size is the reference size: every window is zoomed by the same
  // ratio so the preference scales the sidebar, toolbar, dialogs and file list
  // together instead of only enlarging file names.
  useEffect(() => {
    if (!native) return
    void getCurrentWebviewWindow()
      .setZoom(app.prefs.fontSize / 11)
      .catch(() => {})
  }, [app.prefs.fontSize])
  return (
    <div
      className={`app ${app.sidebar ? '' : 'sidebar-hidden'} ${app.dragging ? 'dragging' : ''} ${backgroundUrl ? 'has-background' : ''}`}
      data-platform={app.platform}
      style={{ '--label-size': '11px' } as CSSProperties}
    >
      {backgroundUrl && (
        <div
          className="app-background"
          aria-hidden="true"
          style={{ backgroundImage, opacity: app.prefs.backgroundOpacity }}
        />
      )}
      <Sidebar
        x={{ ...app.x, locations: app.left.locations, connections: app.left.connections }}
        prefs={app.prefs}
        update={app.update}
        onTrash={app.openTrash}
        onEmptyTrash={app.emptyTrash}
        onApplications={app.openApplications}
        onAddConnection={() => app.setConnectionOpen(true)}
        onRemoveConnection={app.removeConnection}
        notify={app.notify}
        onEject={app.ejectDevice}
        onEjectDevice={app.ejectUsb}
        onMount={app.mountDevice}
        trashPath={app.trashPath}
      />
      <main
        className={`main ${app.split ? 'split' : ''}`}
        style={{ '--second-pane-width': `${app.paneWidth}%` } as CSSProperties}
      >
        <ExplorerTabs {...app} />
        {!fullView && (
          <ExplorerToolbar
            {...app}
            split={app.split}
            onToggleSplit={() => (app.split ? app.closePane() : app.openPane())}
          />
        )}
        {!fullView && <ExplorerPathBar {...app} x={app.left} />}
        {!fullView && app.x.error && (
          <div className="error-banner" role="alert">
            <Info size={16} />
            <span>{app.x.error}</span>
            {navigator.userAgent.includes('Windows') && /管理员/.test(app.x.error) && (
              <button
                className="error-action"
                onClick={() =>
                  void call('relaunch_elevated').catch((error) => app.x.setError(String(error)))
                }
              >
                以管理员身份重启
              </button>
            )}
            <button className="icon-button" title="关闭提示" onClick={() => app.x.setError('')}>
              <X size={15} />
            </button>
          </div>
        )}
        {!fullView && app.x.warnings.length > 0 && (
          <details className="warnings">
            <summary>{app.x.warnings.length} 个项目无法读取</summary>
            {app.x.warnings.join('\n')}
          </details>
        )}
        {fullViews.map((view) => (
          <div
            className="full-view-pane"
            key={`${view.path}:${view.index}`}
            hidden={view.index !== app.left.tab}
          >
            {view.terminal ? (
              <SshTerminalView
                connectionId={view.terminal}
                name={
                  app.left.connections.find((connection) => connection.id === view.terminal)
                    ?.name || 'SSH'
                }
              />
            ) : view.plugin ? (
              <PluginConnectionView connectionId={view.plugin} onNavigate={app.left.navigate} />
            ) : null}
          </div>
        ))}
        {!fullView &&
          (app.left.loading && !app.left.hasListing ? (
            <div className="loading">
              <LoaderCircle className="spin" />
              正在读取文件夹…
            </div>
          ) : (
            <div
              className={`pane-main ${app.split && app.active === 'left' ? 'pane-active' : ''}`}
              onPointerDownCapture={() => app.setActive('left')}
              data-drop-folder={
                /^[a-z][a-z0-9+.-]*:\/\//i.test(app.left.path) ? undefined : app.left.path
              }
            >
              <FileViews
                readOnly={app.left.directoryReadOnly}
                trashPath={app.trashPath}
                explorer={app.left}
                prefs={app.prefs}
                onOpen={app.openLeft}
                onPreview={(entry) => app.previewEntry(entry, app.left)}
                onMenu={(entry, mx, my) => app.setMenu({ entry, x: mx, y: my })}
                onDrop={app.transfer}
                onBookmark={app.bookmarkPaths}
                onWorkspace={app.addWorkspace}
                dragPaths={app.selectedAll}
                onExternalDrag={(result) =>
                  app.notify(result === 'Dropped' ? '已交给目标应用处理' : '已取消拖拽')
                }
              />
            </div>
          ))}
        {!fullView && app.split && (
          <SecondaryPane
            explorer={app.right}
            prefs={app.prefs}
            active={app.active === 'right'}
            onFocus={() => app.setActive('right')}
            onClose={() => app.closePane('right')}
            onTransfer={(paths, destination, moving) => app.transfer(paths, destination, moving)}
            onOpen={app.openRight}
            onPreview={(entry) => app.previewEntry(entry, app.right)}
            onMenu={(entry, mx, my) => app.setMenu({ entry, x: mx, y: my })}
            dragPaths={app.selectedAll}
            displayName={app.displayName}
            width={app.paneWidth}
            onWidthChange={app.setPaneWidth}
          />
        )}
        {!fullView && (
          <WorkspaceBar
            items={app.workspace.items}
            open={app.workspace.open}
            onOpen={app.open}
            onRemove={app.workspace.remove}
            onClear={app.workspace.clear}
            onClose={() => app.workspace.setOpen(false)}
          />
        )}
        {!fullView && <ExplorerStatusBar {...app} />}
        <ActivityCenter />
      </main>
      {app.dragging && (
        <div className="drop-overlay">
          <Download />
          <strong>复制到 {app.displayName(app.x.path)}</strong>
        </div>
      )}
      <FileContextMenu {...app} />
      <BatchRenameDialog
        entries={app.batchRename}
        onClose={() => app.setBatchRename(null)}
        onApply={app.applyBatchRename}
      />
      <ExplorerDialogs {...app} />
    </div>
  )
}
