import { Download, Upload } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { PluginInstaller } from '../PluginInstaller'
import type { PluginInfo } from '@/lib/plugins'
import type { ReactNode } from 'react'

/** Entry points above the installed list: local import and the official store. */
export function PluginStoreToolbar({
  onImport,
  onOpenStore,
  children,
}: {
  onImport: () => void
  onOpenStore: () => void
  children?: ReactNode
}) {
  return (
    <div className="plugin-manager-toolbar">
      {children}
      <div className="plugin-toolbar-actions">
        <Button type="button" size="sm" variant="outline" onClick={onImport}>
          <Upload size={15} />
          导入插件
        </Button>
        <Button type="button" size="sm" onClick={onOpenStore}>
          <Download size={15} />
          获取插件
        </Button>
      </div>
    </div>
  )
}

/**
 * The marketplace sheet. PluginInstaller owns the catalog fetch plus the
 * search, card, install-button and busy states; the page only owns whether the
 * sheet is open and what happens after an install.
 */
export function PluginStore({
  open,
  installed,
  onClose,
  onInstalled,
  initialCategory,
}: {
  open: boolean
  installed: PluginInfo[]
  onClose: () => void
  onInstalled: () => void
  initialCategory?: string
}) {
  return (
    <PluginInstaller
      open={open}
      installed={installed}
      onClose={onClose}
      onInstalled={onInstalled}
      initialCategory={initialCategory}
    />
  )
}
