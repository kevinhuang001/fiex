import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { CoreLogsSheet } from './plugins/CoreLogsSheet'
import { requestPluginActivation } from '@/components/plugins/PluginActivationHost'
import { t } from '@/i18n'
import { CommandDialog, DomainDialog } from './plugins/PermissionDialogs'
import { PluginImportDialog, PluginMessage, PluginUninstallDialog } from './plugins/PluginAlerts'
import { PluginList } from './plugins/PluginList'
import { PluginPermissionsSheet } from './plugins/PluginPermissionsSheet'
import { PluginLogsSheet } from './plugins/PluginLogsSheet'
import { PluginSettingsSheet } from './plugins/PluginSettingsSheet'
import { PluginStore, PluginStoreToolbar } from './plugins/PluginStore'
import { PluginCategoryFilter } from './plugins/PluginCategoryFilter'
import { refreshPluginCatalog } from '@/lib/plugins'
import { usePluginSettingsController } from '@/hooks/usePluginSettingsController'
import { PluginPermissionPrompt, promptForPermission } from './PluginPermissionPrompt'

export function PluginSettings() {
  const [coreLogs, setCoreLogs] = useState(false)
  const {
    plugins,
    category,
    setCategory,
    categories,
    shown,
    storeOpen,
    setStoreOpen,
    storeCategory,
    localPreview,
    setLocalPreview,
    permissionsFor,
    setPermissionsFor,
    logsFor,
    setLogsFor,
    settingsFor,
    setSettingsFor,
    busy,
    message,
    setMessage,
    draft,
    permissionToApprove,
    setPermissionToApprove,
    domainDraft,
    setDomainDraft,
    domainSettingsOpen,
    setDomainSettingsOpen,
    domainToApprove,
    setDomainToApprove,
    commandDraft,
    setCommandDraft,
    commandSettingsOpen,
    setCommandSettingsOpen,
    commandToApprove,
    setCommandToApprove,
    confirmRemove,
    setConfirmRemove,
    chooseLocalPlugin,
    installLocal,
    cancelLocalImport,
    fields,
    updateActivation,
    updatePermission,
    grantPermission,
    updateDomain,
    updateNetworkAll,
    updateCommand,
    uninstall,
    saveSettings,
    updateDraft,
  } = usePluginSettingsController()

  return (
    <>
      <div className="plugin-page">
        <PluginStoreToolbar
          onImport={() => void chooseLocalPlugin()}
          onOpenStore={() => setStoreOpen(true)}
        >
          <PluginCategoryFilter categories={categories} value={category} onChange={setCategory} />
          <Button type="button" size="sm" variant="outline" onClick={() => setCoreLogs(true)}>{t('Core 运行日志')}</Button>
        </PluginStoreToolbar>
        <PluginList
          plugins={shown}
          busy={busy}
          onOpenSettings={(plugin) => {
            setMessage('')
            setSettingsFor(plugin)
          }}
          onOpenDetails={(plugin) => {
            setMessage('')
            setDomainSettingsOpen(false)
            setCommandSettingsOpen(false)
            setLogsFor(null)
            setPermissionsFor(plugin)
          }}
          onOpenLogs={(plugin) => {
            setMessage('')
            setPermissionsFor(null)
            setLogsFor(plugin)
          }}
          onRemove={(plugin) => {
            setMessage('')
            setConfirmRemove(plugin)
          }}
          onToggleActivation={(plugin, enabled) => void updateActivation(plugin, enabled)}
          onBrowseStore={() => setStoreOpen(true)}
        />
        <PluginMessage message={message} />
      </div>

      <PluginStore
        open={storeOpen}
        installed={plugins}
        initialCategory={storeCategory}
        onClose={() => setStoreOpen(false)}
        onInstalled={() => {
          refreshPluginCatalog()
          setStoreOpen(false)
        }}
      />

      <PluginPermissionsSheet
        plugin={permissionsFor}
        busy={busy}
        message={message}
        onClose={() => {
          setPermissionsFor(null)
          setDomainSettingsOpen(false)
          setCommandSettingsOpen(false)
          setMessage('')
        }}
        onActivate={(plugin) => requestPluginActivation(plugin.id)}
        onTogglePermission={(plugin, permission, granted) =>
          void updatePermission(plugin, permission, granted)
        }
        onConfigure={(permission) => {
          setMessage('')
          if (permission === 'network') setDomainSettingsOpen(true)
          else setCommandSettingsOpen(true)
        }}
      />

      <PluginLogsSheet plugin={logsFor} onClose={() => setLogsFor(null)} />
      <CoreLogsSheet open={coreLogs} onClose={() => setCoreLogs(false)} />

      {permissionsFor &&
        domainSettingsOpen &&
        permissionsFor.declaredPermissions.includes('network') && (
          <DomainDialog
            plugin={permissionsFor}
            busy={busy}
            draft={domainDraft}
            onDraftChange={setDomainDraft}
            onToggle={(domain, granted) => void updateDomain(permissionsFor, domain, granted)}
            onAdd={(domain) => void updateDomain(permissionsFor, domain, true)}
            onToggleAll={(all) => void updateNetworkAll(permissionsFor, all)}
            onClose={() => setDomainSettingsOpen(false)}
          />
        )}
      {permissionsFor &&
        commandSettingsOpen &&
        permissionsFor.declaredPermissions.includes('commands') && (
          <CommandDialog
            plugin={permissionsFor}
            busy={busy}
            draft={commandDraft}
            onDraftChange={setCommandDraft}
            onToggle={(command, granted) => void updateCommand(permissionsFor, command, granted)}
            onAdd={(command) => void updateCommand(permissionsFor, command, true)}
            onClose={() => setCommandSettingsOpen(false)}
          />
        )}

      <PluginSettingsSheet
        plugin={settingsFor}
        fields={fields}
        draft={draft}
        busy={busy}
        message={message}
        onDraftChange={updateDraft}
        onSave={() => void saveSettings()}
        onClose={() => setSettingsFor(null)}
        onDismiss={() => {
          setSettingsFor(null)
          setMessage('')
        }}
      />

      {permissionsFor && domainToApprove && (
        <PluginPermissionPrompt
          open
          pluginName={permissionsFor.name}
          icon={permissionsFor.icon}
          item={{
            id: `domain:${domainToApprove}`,
            kind: 'network',
            title: t('网络域名'),
            value: domainToApprove,
          }}
          step={1}
          total={1}
          busy={busy}
          onAllow={() => void updateDomain(permissionsFor, domainToApprove, true)}
          onDeny={() => setDomainToApprove(null)}
        />
      )}
      {permissionsFor && permissionToApprove && (
        <PluginPermissionPrompt
          open
          pluginName={permissionsFor.name}
          icon={permissionsFor.icon}
          item={promptForPermission(permissionToApprove)}
          step={1}
          total={1}
          busy={busy}
          onAllow={() => void grantPermission(permissionsFor)}
          onDeny={() => setPermissionToApprove(null)}
        />
      )}

      <PluginUninstallDialog
        plugin={confirmRemove}
        busy={busy}
        onClose={() => setConfirmRemove(null)}
        onConfirm={() => void uninstall()}
      />
      <PluginImportDialog
        preview={localPreview}
        busy={busy}
        onClose={() => setLocalPreview(null)}
        onCancel={cancelLocalImport}
        onConfirm={() => void installLocal()}
      />
      {permissionsFor && commandToApprove && (
        <PluginPermissionPrompt
          open
          pluginName={permissionsFor.name}
          icon={permissionsFor.icon}
          item={{
            id: `command:${commandToApprove}`,
            kind: 'command',
            title: t('外部命令'),
            value: commandToApprove,
          }}
          step={1}
          total={1}
          busy={busy}
          onAllow={() => void updateCommand(permissionsFor, commandToApprove, true)}
          onDeny={() => setCommandToApprove(null)}
        />
      )}
    </>
  )
}
