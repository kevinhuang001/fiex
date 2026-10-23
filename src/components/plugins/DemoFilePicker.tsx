import { useState } from 'react'
import { ChevronUp, FileText, Folder, X } from 'lucide-react'
export interface FakePicker {
  mode: string
  suggestedName?: string
  directory: string
  resolve: (value: string | string[] | null) => void
}

const FAKE_TREE: Record<string, { folders: string[]; files: string[] }> = {
  '/Users/kevin/Documents': {
    folders: ['Projects', 'Research', 'Archive'],
    files: ['Notes.md', 'Palette.json', 'Budget.xlsx', 'Archive.zip'],
  },
  '/Users/kevin/Documents/Projects': {
    folders: ['Design'],
    files: ['Fiex — 产品设计.pdf'],
  },
  '/Users/kevin/Documents/Projects/Design': {
    folders: [],
    files: ['Palette.json'],
  },
  '/Users/kevin/Documents/Research': {
    folders: [],
    files: ['Meeting.txt'],
  },
  '/Users/kevin/Documents/Archive': {
    folders: [],
    files: ['Archive.zip'],
  },
}

export function DemoFilePicker({
  fakePicker,
  onClose,
}: {
  fakePicker: FakePicker
  onClose: () => void
}) {
  const [fakeDirectory, setFakeDirectory] = useState(
    fakePicker.directory || '/Users/kevin/Documents',
  )
  const [fakeName, setFakeName] = useState(fakePicker.suggestedName || '')
  const fakeEntries = FAKE_TREE[fakeDirectory] || { folders: [], files: [] }
  const setFakePicker = (_value: null) => onClose()
  return (
    <div className="fake-picker-overlay" role="presentation">
      <div
        className="fake-picker"
        role="dialog"
        aria-label={fakePicker.mode === 'dialog.saveFile' ? '保存文件' : '选择文件'}
      >
        <header>
          <strong>{fakePicker.mode === 'dialog.saveFile' ? '保存文件' : '选择文件'}</strong>
          <button
            type="button"
            aria-label="关闭"
            onClick={() => {
              fakePicker.resolve(null)
              setFakePicker(null)
            }}
          >
            <X size={16} />
          </button>
        </header>
        <div className="fake-picker-path">
          <button
            type="button"
            aria-label="上一级"
            onClick={() =>
              setFakeDirectory((current) => {
                const parts = current.split('/').filter(Boolean)
                parts.pop()
                return `/${parts.join('/')}` || '/'
              })
            }
          >
            <ChevronUp size={14} />
          </button>
          <span title={fakeDirectory}>{fakeDirectory}</span>
        </div>
        <div className="fake-picker-list">
          {fakeEntries.folders.map((folder) => (
            <button
              type="button"
              className="fake-picker-item"
              key={folder}
              onClick={() => {
                setFakeDirectory((current) => `${current.replace(/\/$/, '')}/${folder}`)
                if (fakePicker.mode !== 'dialog.saveFile') setFakeName('')
              }}
            >
              <Folder size={15} />
              {folder}
            </button>
          ))}
          {fakeEntries.files.map((file) => (
            <button
              type="button"
              className={`fake-picker-item ${fakeName === file ? 'selected' : ''}`}
              key={file}
              onClick={() => setFakeName(file)}
              onDoubleClick={() => {
                const value = `${fakeDirectory.replace(/\/$/, '')}/${file}`
                fakePicker.resolve(fakePicker.mode === 'dialog.openFiles' ? [value] : value)
                setFakePicker(null)
              }}
            >
              <FileText size={15} />
              {file}
            </button>
          ))}
        </div>
        {fakePicker.mode === 'dialog.saveFile' && (
          <label className="fake-picker-name">
            文件名
            <input
              aria-label="文件名"
              value={fakeName}
              onChange={(event) => setFakeName(event.target.value)}
            />
          </label>
        )}
        <footer>
          <button
            type="button"
            onClick={() => {
              fakePicker.resolve(null)
              setFakePicker(null)
            }}
          >
            取消
          </button>
          <button
            type="button"
            className="primary"
            disabled={fakePicker.mode !== 'dialog.openDirectory' && !fakeName.trim()}
            onClick={() => {
              const value =
                fakePicker.mode === 'dialog.openDirectory'
                  ? fakeDirectory
                  : `${fakeDirectory.replace(/\/$/, '')}/${fakeName}`
              fakePicker.resolve(fakePicker.mode === 'dialog.openFiles' ? [value] : value)
              setFakePicker(null)
            }}
          >
            {fakePicker.mode === 'dialog.saveFile' ? '保存' : '选择'}
          </button>
        </footer>
      </div>
    </div>
  )
}
