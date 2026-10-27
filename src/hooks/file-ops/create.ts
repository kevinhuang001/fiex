import { t } from '@/i18n'
import { call } from '@/lib/api'
import { uniqueName } from '@/lib/files'
import type { FileOpsContext } from './types'

/** Creating the next entry, then handing it straight to the inline rename. */
export function createActions({ x, writable, run }: FileOpsContext) {
  /** New entries are folders: the file-system protocol has no "create an
   * empty file by name" operation, so Core only offers folder creation. */
  const create = async () => {
    if (!writable()) return
    const name = uniqueName(
      x.entries.map((e) => e.name),
      t('未命名文件夹'),
    )
    const path = await run(t('正在新建文件夹…'), () =>
      call<string>('create_entry', { path: x.path, name }),
    )
    if (!path) return
    if (x.query) x.setQuery('')
    x.beginRename(path)
  }
  return { create }
}
