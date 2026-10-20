import {
  Sun,
  Eye,
  SlidersHorizontal,
  TerminalSquare,
  ChevronDown,
  ExternalLink,
} from 'lucide-react'
import { Modal } from './ui/Modal'
import { Button } from './ui/button'
import { Menu, Choice } from './Menus'
import { call } from '@/lib/api'
import type { Preferences, Terminal } from '@/lib/types'
interface Props {
  open: boolean
  onClose: () => void
  prefs: Preferences
  update: (p: Partial<Preferences>) => void
  terminals: Terminal[]
  onError: (s: string) => void
}
export function PreferencesDialog({ open, onClose, prefs, update, terminals, onError }: Props) {
  return (
    <Modal open={open} onClose={onClose} title="设置">
      <div className="settings">
        <div className="setting-row">
          <span>
            <Sun size={17} />
            外观
          </span>
          <div className="segmented">
            {(['system', 'light', 'dark'] as const).map((t, i) => (
              <button key={t} aria-pressed={prefs.theme === t} onClick={() => update({ theme: t })}>
                {['跟随系统', '浅色', '深色'][i]}
              </button>
            ))}
          </div>
        </div>
        <div className="setting-row">
          <span>
            <Eye size={17} />
            显示隐藏文件
          </span>
          <button
            role="switch"
            aria-checked={prefs.hidden}
            className="switch"
            onClick={() => update({ hidden: !prefs.hidden })}
          >
            <i />
          </button>
        </div>
        <div className="setting-row">
          <span>
            <SlidersHorizontal size={17} />
            图标大小
          </span>
          <input
            aria-label="图标大小"
            type="range"
            min="48"
            max="136"
            step="8"
            value={prefs.iconSize}
            onChange={(e) => update({ iconSize: +e.target.value })}
          />
        </div>
        <div className="setting-row">
          <span>
            <TerminalSquare size={17} />
            默认终端
          </span>
          <Menu label="默认终端" icon={<ChevronDown />}>
            {terminals.map((t) => (
              <Choice
                key={t.id}
                checked={prefs.terminal === t.id}
                onClick={() => update({ terminal: t.id })}
              >
                {t.name}
              </Choice>
            ))}
          </Menu>
        </div>
        <p className="settings-note">
          {terminals.find((t) => t.id === prefs.terminal)?.name || '系统默认终端'}
        </p>
        <div className="about">
          <img src="/fiex.svg" alt="" />
          <strong>
            Fiex <small>0.1.0</small>
          </strong>
          <span>File explorer, with room to breathe.</span>
          <Button
            variant="secondary"
            onClick={() =>
              void call('open_path', { path: 'https://github.com/kevinhuang001/fiex' }).catch((e) =>
                onError(String(e)),
              )
            }
          >
            GitHub
            <ExternalLink />
          </Button>
        </div>
        <details className="shortcuts">
          <summary>键盘快捷键</summary>
          <p>
            空格：快速查看 · 方向键：选择文件
            <br />⌘ / Ctrl + F：查找 · ⌘ / Ctrl + L：前往文件夹
            <br />⌘ / Ctrl + C / X / V：复制 / 剪切 / 粘贴
            <br />⌘ / Ctrl + Z：撤销 · ⌘ / Ctrl + D：创建副本
            <br />⌘ / Ctrl + T / W：新建 / 关闭标签页
            <br />
            ⇧⌘ / Ctrl + Shift + N：新建文件夹
            <br />
            F2：重新命名 · Delete / ⌘⌫：移到废纸篓
            <br />
            Alt + ← / →：前进后退 · F5：刷新
          </p>
        </details>
      </div>
    </Modal>
  )
}
