import { t } from '@/i18n'
import { call } from '@/lib/api'
import { reportError } from '@/lib/errorCenter'
import { Button } from '../ui/button'
export function ConnectionSetupHelp({
  factory,
  busy,
}: {
  factory: { description?: string; setupUrl?: string }
  busy: boolean
}) {
  const setupUrl = factory.setupUrl
  return (
    <>
      {factory.description && <p className="sidebar-hint">{factory.description}</p>}
      {setupUrl && (
        <Button
          type="button"
          variant="outline"
          disabled={busy}
          onClick={() =>
            void call('open_external_url', { url: setupUrl }).catch((cause) =>
              reportError(cause, 'connections:setup'),
            )
          }
        >
          {t('打开应用控制台')}
        </Button>
      )}
    </>
  )
}
