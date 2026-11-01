import { useAppEnvironment } from '@/hooks/useAppEnvironment'
import { t, useLocale } from '@/i18n'
import { FileViews } from '@/components/browser/FileViews'
import { BatchRenameDialog } from '@/components/BatchRenameDialog'
import { ExplorerDialogs } from '@/components/explorer/ExplorerDialogs'
import { ErrorDialogHost } from '@/components/ErrorDialogHost'
import { reportError } from '@/lib/errorCenter'
import { ExplorerPathBar } from '@/components/explorer/ExplorerPathBar'
import { ExplorerStatusBar } from '@/components/explorer/ExplorerStatusBar'
import { LogCenter } from '@/components/activity/LogCenter'
import { ActivityCenter } from '@/components/ActivityCenter'
import { SecondaryPane } from '@/components/SecondaryPane'
import { WorkspaceBar } from '@/components/WorkspaceBar'
import { ExplorerTabs } from '@/components/explorer/ExplorerTabs'
import { ExplorerToolbar } from '@/components/explorer/ExplorerToolbar'
import { FileContextMenu } from '@/components/explorer/FileContextMenu'
import { Sidebar } from '@/components/Sidebar'
import { PluginConnectionView } from '@/components/plugins/PluginConnectionView'
import { PluginServices } from '@/components/plugins/PluginServices'
import { TerminalPanel } from '@/components/terminal/TerminalPanel'
import { useAppController } from '@/hooks/useAppController'
import { warmFilePickerData } from '@/lib/filePickerData'
import { usePluginTasks } from '@/hooks/usePluginTasks'
import { asset, native } from '@/lib/api'
import { Download, LoaderCircle } from 'lucide-react'
import { useEffect, useState } from 'react'
import type { CSSProperties } from 'react'
export default function App() {
  useLocale()

  const app = useAppController()
  usePluginTasks()
  useEffect(() => {
    if (!app.x.error) return
    reportError(app.x.error, 'explorer')
    app.x.setError('')
  }, [app.x.error])

  useEffect(() => {
    warmFilePickerData()
  }, [])
  const pluginViewOf = (path: string) =>
    /^fiex-plugin-view:\/\/([A-Za-z0-9-]{1,128})$/.exec(path)?.[1]
  const pluginViewId = pluginViewOf(app.left.path)
  const fullView = Boolean(pluginViewId)
  const fullViews = app.left.tabs.flatMap((path, index) => {
    const plugin = pluginViewOf(path)
    return plugin ? [{ path, index, plugin }] : []
  })
  const [viewTitles, setViewTitles] = useState<Record<string, string>>({})
  const tabName = (path: string) => viewTitles[path] || app.displayName(path)
  const uiScale = useAppEnvironment(app)
  const backgroundPath = app.prefs.backgroundImage
  const backgroundUrl = backgroundPath
    ? native && !backgroundPath.includes('://')
      ? asset(backgroundPath)
      : backgroundPath
    : ''
  const backgroundImage = backgroundUrl ? `url("${backgroundUrl.replace(/"/g, '%22')}")` : undefined

  return (
    <div
      className={`app ${app.sidebar ? '' : 'sidebar-hidden'} ${app.dragging ? 'dragging' : ''} ${backgroundUrl ? 'has-background' : ''}`}
      data-platform={app.platform}
      style={
        {
          '--label-size': '11px',
          ...(native
            ? {}
            : {
                zoom: uiScale,
                width: `calc(100vw / ${uiScale})`,
                height: `calc(100vh / ${uiScale})`,
              }),
        } as CSSProperties
      }
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
        onApplications={app.openApplications}
        onAddConnection={() => app.openConnectionDialog()}
        onEditConnection={(connection) => app.openConnectionDialog(connection)}
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
        <ExplorerTabs {...app} displayName={tabName} />
        {!fullView && (
          <ExplorerToolbar
            {...app}
            split={app.split}
            onToggleSplit={() => (app.split ? app.closePane() : app.openPane())}
          />
        )}
        {!fullView && <ExplorerPathBar {...app} x={app.left} />}
        {fullViews.map((view) => (
          <div
            className="full-view-pane"
            key={`${view.path}:${view.index}`}
            hidden={view.index !== app.left.tab}
          >
            {view.plugin ? (
              <PluginConnectionView
                connectionId={view.plugin}
                onNavigate={app.left.navigate}
                onTitle={(title) => setViewTitles((values) => ({ ...values, [view.path]: title }))}
              />
            ) : null}
          </div>
        ))}
        {!fullView &&
          (app.left.loading && !app.left.hasListing ? (
            <div className="loading">
              <LoaderCircle className="spin" />
              {t('正在读取文件夹…')}
            </div>
          ) : (
            <div
              className={`pane-main ${app.split && app.active === 'left' ? 'pane-active' : ''}`}
              onPointerDownCapture={() => app.setActive('left')}
              data-drop-folder={
                /^[a-z][a-z0-9+.-]*:\/\//i.test(app.left.path) &&
                app.left.path !== 'applications://' &&
                !/^applications:\/\/(?:user\/|~)/.test(app.left.path)
                  ? undefined
                  : app.left.path
              }
            >
              <FileViews
                readOnly={app.left.directoryReadOnly}
                trashPath={app.trashPath}
                explorer={app.left}
                prefs={app.prefs}
                updatePrefs={app.update}
                onOpen={app.openLeft}
                onPreview={(entry) => app.previewEntry(entry, app.left)}
                onMenu={(entry, mx, my) => app.setMenu({ entry, x: mx, y: my })}
                onBlankMenu={(mx, my) => app.setMenu({ entry: null, x: mx, y: my })}
                onDrop={app.transfer}
                onBookmark={app.bookmarkPaths}
                onWorkspace={app.addWorkspace}
                dragPaths={app.selectedAll}
                onExternalDrag={(result) =>
                  app.notify(result === 'Dropped' ? t('已交给目标应用处理') : t('已取消拖拽'))
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
            onBlankMenu={(mx, my) => app.setMenu({ entry: null, x: mx, y: my })}
            dragPaths={app.selectedAll}
            displayName={app.displayName}
            updatePrefs={app.update}
            width={app.paneWidth}
            onWidthChange={app.setPaneWidth}
          />
        )}
        <TerminalPanel
          terminalFont={app.prefs.terminalFont}
          terminalFontSize={app.prefs.terminalFontSize}
          onSettingsChange={app.update}
        />
        {!fullView && (
          <WorkspaceBar
            items={app.workspace.items}
            open={app.workspace.open}
            onOpen={app.open}
            onRemove={app.workspace.remove}
            onClear={app.workspace.clear}
            onClose={() => app.workspace.setOpen(false)}
            onTerminal={() =>
              window.dispatchEvent(
                new CustomEvent('fiex:show-terminal-panel', {
                  detail: { cwd: app.left.path },
                }),
              )
            }
          />
        )}
        {!fullView && <ExplorerStatusBar {...app} />}
        <ActivityCenter />
        <LogCenter />
      </main>
      <div id="fiex-overlay-root" className="fiex-overlay-root" />
      {app.dragging && (
        <div className="drop-overlay">
          <Download />
          <strong>
            {t('复制到')}
            {app.displayName(app.x.path)}
          </strong>
        </div>
      )}
      <FileContextMenu {...app} />
      <BatchRenameDialog
        entries={app.batchRename}
        onClose={() => app.setBatchRename(null)}
        onApply={app.applyBatchRename}
      />
      <ExplorerDialogs {...app} />
      <ErrorDialogHost />
      <PluginServices />
    </div>
  )
}
