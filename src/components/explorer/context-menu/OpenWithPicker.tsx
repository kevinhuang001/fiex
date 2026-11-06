import { t } from '@/i18n'
import { useEffect, useMemo, useRef, useState } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import { AppWindow, Search } from 'lucide-react'
import { call } from '@/lib/api'
import { Modal } from '@/components/ui/Modal'

interface PickerItem {
  id: string
  name: string
  iconSource?: string
}
interface PickerResponse {
  sessionId: string
  items: PickerItem[]
}

function PickerApplication({ item, onChoose }: { item: PickerItem; onChoose: () => void }) {
  const [icon, setIcon] = useState('')
  useEffect(() => {
    if (!item.iconSource) return
    let active = true
    void call<string>('native_icon', { path: item.iconSource, size: 32 })
      .then((source) => {
        if (active) setIcon(source)
      })
      .catch(() => {})
    return () => {
      active = false
    }
  }, [item.iconSource])

  return (
    <button role="option" onClick={onChoose} title={item.name}>
      {icon ? <img src={icon} alt="" /> : <AppWindow size={20} />}
      <span>{item.name}</span>
    </button>
  )
}

export function OpenWithPicker({
  path,
  onClose,
  onError,
}: {
  path: string
  onClose: () => void
  onError: (message: string) => void
}) {
  const [session, setSession] = useState<PickerResponse>()
  const [query, setQuery] = useState('')
  const [error, setError] = useState('')
  const listRef = useRef<HTMLDivElement>(null)
  const visibleItems = useMemo(
    () =>
      (session?.items || []).filter((item) =>
        item.name.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase()),
      ),
    [session, query],
  )
  const rowCount = Math.ceil(visibleItems.length / 2)
  const rows = useVirtualizer({
    count: rowCount,
    getScrollElement: () => listRef.current,
    estimateSize: () => 46,
    overscan: 8,
  })

  useEffect(() => {
    let active = true
    void call<PickerResponse>('open_with_picker_candidates', { path })
      .then((result) => {
        if (active) setSession(result)
      })
      .catch((reason) => {
        if (active) setError(String(reason))
      })
    return () => {
      active = false
    }
  }, [path])

  const choose = async (item: PickerItem) => {
    if (!session) return
    try {
      await call('open_with_picker_execute', {
        path,
        sessionId: session.sessionId,
        itemId: item.id,
      })
      onClose()
    } catch (reason) {
      onError(String(reason))
    }
  }

  return (
    <Modal open onClose={onClose} title={t('选择应用打开文件')} className="open-with-picker">
      <label className="open-with-search">
        <Search size={16} />
        <input
          autoFocus
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder={t('搜索应用')}
          aria-label={t('搜索应用')}
        />
      </label>
      {error ? (
        <div className="open-with-empty">{error}</div>
      ) : !session ? (
        <div className="open-with-empty">{t('正在加载应用…')}</div>
      ) : visibleItems.length ? (
        <div ref={listRef} className="open-with-list" role="listbox" aria-label={t('可用应用')}>
          <div style={{ height: rows.getTotalSize(), position: 'relative' }}>
            {rows.getVirtualItems().map((row) => {
              const items = visibleItems.slice(row.index * 2, row.index * 2 + 2)
              return (
                <div
                  key={row.key}
                  className="open-with-row"
                  style={{ height: row.size, transform: `translateY(${row.start}px)` }}
                >
                  {items.map((item) => (
                    <PickerApplication
                      key={item.id}
                      item={item}
                      onChoose={() => void choose(item)}
                    />
                  ))}
                </div>
              )
            })}
          </div>
        </div>
      ) : (
        <div className="open-with-empty">{t('没有匹配的应用')}</div>
      )}
    </Modal>
  )
}
