import { t } from '@/i18n'
import { call } from '@/lib/api'
import { reportFilesystemPermissionError } from '@/lib/errorCenter'
import { uniqueName } from '@/lib/files'
import type { Entry } from '@/lib/types'
import { canRenameEntry } from './rename'
import type { FileOpsContext } from './types'

export type OfficeFileTemplate = 'docx' | 'xlsx' | 'pptx'

/** Creating the next entry, then handing it straight to the inline rename. */
export function createActions({ x, run }: FileOpsContext) {
  const renameCreatedEntry = async (path: string) => {
    let entry: Entry
    try {
      entry = await call('file_info', { path })
    } catch (cause) {
      const message = String(cause)
      if (!reportFilesystemPermissionError(message, `stat\n${path}`)) x.setError(message)
      return
    }
    if (canRenameEntry(entry)) x.beginRename(path, entry)
  }
  const create = async () => {
    if (!x.directoryAllowedOps.includes('mkdir')) {
      x.setError(t('此位置不支持新建文件夹'))
      return
    }
    const name = uniqueName(
      x.entries.map((e) => e.name),
      t('未命名文件夹'),
    )
    const path = await run(
      t('正在新建文件夹…'),
      () => call('create_entry', { path: x.path, name }),
      undefined,
      false,
      undefined,
      undefined,
      false,
      undefined,
      undefined,
      (created) => (created ? [x.path] : []),
    )
    if (!path) return
    if (x.query) x.setQuery('')
    await renameCreatedEntry(path)
  }
  /** An empty file goes through the provider's write stream with a name; the
   * menu only offers it when the directory declares `write`. */
  const createFile = async (template?: OfficeFileTemplate) => {
    if (!x.directoryAllowedOps.includes('write')) {
      x.setError(t('此位置不支持新建文件'))
      return
    }
    const templateNames: Record<OfficeFileTemplate, string> = {
      docx: t('未命名文档.docx'),
      xlsx: t('未命名工作簿.xlsx'),
      pptx: t('未命名演示文稿.pptx'),
    }
    const name = uniqueName(
      x.entries.map((e) => e.name),
      template ? templateNames[template] : `${t('未命名文件')}.txt`,
    )
    const path = await run(
      t('正在新建文件…'),
      () => call('create_file_entry', { path: x.path, name, template }),
      undefined,
      false,
      undefined,
      undefined,
      false,
      undefined,
      undefined,
      (created) => (created ? [x.path] : []),
    )
    if (!path) return
    if (x.query) x.setQuery('')
    x.setSelected([path])
    await renameCreatedEntry(path)
  }
  return { create, createFile }
}
