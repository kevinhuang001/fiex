import manifest from 'material-icon-theme/dist/material-icons.json'
import type { Entry } from './types'

export interface FileVisual {
  url: string
  folder: boolean
  generic: boolean
}

const applications = new Set(['app', 'exe', 'desktop', 'appimage'])
const base = import.meta.env.BASE_URL || '/'
const cache = new Map<string, FileVisual>()

function definition(id: string | undefined): string {
  const path =
    (id && manifest.iconDefinitions[id]?.iconPath) ||
    manifest.iconDefinitions.document?.iconPath ||
    manifest.iconDefinitions.file.iconPath
  return path.slice(path.lastIndexOf('/') + 1)
}

function visual(id: string | undefined, folder: boolean, generic: boolean): FileVisual {
  return { url: `${base}file-icons/${definition(id)}`, folder, generic }
}

/**
 * Resolves an entry through the Material Icon Theme manifest, which contains
 * 1,300+ extensions and 2,100+ file names. File names win over compound
 * extensions, so `foo.d.ts`, `schema.json`, `Dockerfile` and `.gitignore` all
 * get their dedicated icon.
 */
export function fileVisual(entry: Pick<Entry, 'name' | 'isDir' | 'extension'>): FileVisual {
  const key = `${entry.isDir ? 'd' : 'f'}:${entry.name}`
  const hit = cache.get(key)
  if (hit) return hit

  const lower = entry.name.toLowerCase()
  let result: FileVisual
  if (entry.isDir && !applications.has(entry.extension)) {
    const id = manifest.folderNames[lower] || manifest.folderNames[entry.name]
    result = visual(id || manifest.folder, true, !id)
  } else {
    const nameId = manifest.fileNames[lower] || manifest.fileNames[entry.name]
    let extensionId: string | undefined
    if (!nameId) {
      const parts = lower.split('.')
      for (let i = 1; i < parts.length && !extensionId; i++) {
        extensionId = manifest.fileExtensions[parts.slice(i).join('.')]
      }
      // .app/.appimage/.desktop have no dedicated Material icon.
      if (!extensionId && applications.has(entry.extension))
        extensionId = manifest.fileExtensions.exe
    }
    // `file.svg` is deliberately neutral grey. Unknown files use the blue
    // `document` icon instead so the browser never reverts to a grey page.
    const id = nameId || extensionId || manifest.fileExtensions.txt || 'document'
    result = visual(id, false, !nameId && !extensionId)
  }
  if (cache.size >= 4000) cache.delete(cache.keys().next().value!)
  cache.set(key, result)
  return result
}
