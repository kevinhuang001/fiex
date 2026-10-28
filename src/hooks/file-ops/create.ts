import { t } from '@/i18n'
import { call } from '@/lib/api'
import { uniqueName } from '@/lib/files'
import type { FileOpsContext } from './types'

/** Creating the next entry, then handing it straight to the inline rename. */
export function createActions({ x, writable, run }: FileOpsContext) {
  const create = async () => {
    if (!writable()) return
    // The toolbar disables its new-folder button from the same flag; a
    // keyboard shortcut must not bypass the provider's `mkdir` capability.
    if (!x.directoryCanCreate) {
      x.setError(t('此位置不支持新建文件夹'))
      return
    }
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
  /** An empty file goes through the provider's write stream with a name; the
   * menu only offers it when the directory declares `write`. */
  const createFile = async () => {
    if (!writable()) return
    const name = uniqueName(
      x.entries.map((e) => e.name),
      t('未命名文件'),
    )
    const path = await run(t('正在新建文件…'), () =>
      call<string>('create_file_entry', { path: x.path, name }),
    )
    if (!path) return
    if (x.query) x.setQuery('')
    x.beginRename(path)
  }
  return { create, createFile }
}
