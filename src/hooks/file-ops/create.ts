import { t } from '@/i18n'
import { call } from '@/lib/api'
import { uniqueName } from '@/lib/files'
import type { FileOpsContext } from './types'

/** Creating the next entry, then handing it straight to the inline rename. */
export function createActions({ x, writable, run }: FileOpsContext) {
  const create = async (folder: boolean) => {
    if (!writable()) return
    const preferred = folder ? t('未命名文件夹') : t('未命名.txt')
    const name = uniqueName(
      x.entries.map((e) => e.name),
      preferred,
    )
    const path = await run(folder ? t('正在新建文件夹…') : t('正在新建文件…'), () =>
      call<string>('create_entry', { path: x.path, name, folder }),
    )
    if (!path) return
    if (x.query) x.setQuery('')
    x.beginRename(path)
  }
  return { create }
}
