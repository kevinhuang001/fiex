import { useSyncExternalStore } from 'react'
import { convertFileSrc } from '@tauri-apps/api/core'
import { call, demo, native } from '@/lib/api'
import type { Entry } from './types'

export interface PluginIconRule {
  pluginId: string
  pluginName: string
  resourceId: string
  path: string
  mediaType: string
  extensions: string[]
  names: string[]
  mime: string[]
  isDir: number | boolean | null
  priority: number
}

let rules: PluginIconRule[] | null = null
let pending: Promise<PluginIconRule[]> | null = null
let version = 0
const listeners = new Set<() => void>()

if (typeof window !== 'undefined') {
  window.addEventListener('fiex:plugins-changed', () => {
    rules = null
    pending = null
    version += 1
    for (const listener of listeners) listener()
  })
}

function subscribe(listener: () => void) {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

/** Re-render icon consumers after a plugin install, uninstall or toggle. */
export function usePluginIconVersion() {
  return useSyncExternalStore(
    subscribe,
    () => version,
    () => 0,
  )
}

const DEMO_RULES: PluginIconRule[] = [
  {
    pluginId: 'org.example.icon-theme',
    pluginName: '示例图标主题',
    resourceId: 'markdown',
    path: `${import.meta.env.BASE_URL || '/'}plugin-icons/plugin-markdown.svg`,
    mediaType: 'image/svg+xml',
    extensions: ['md'],
    names: [],
    mime: [],
    isDir: false,
    priority: 100,
  },
]

async function load(): Promise<PluginIconRule[]> {
  if (!native) {
    rules = demo ? DEMO_RULES : []
    return rules
  }
  if (rules) return rules
  pending ??= call<PluginIconRule[]>('plugin_file_icons')
    .then((value) => {
      rules = Array.isArray(value) ? value : []
      return rules
    })
    .catch(() => {
      rules = []
      return rules
    })
    .finally(() => {
      pending = null
    })
  return pending
}

function matches(rule: PluginIconRule, entry: Entry) {
  if (rule.isDir !== null && rule.isDir !== undefined && Boolean(rule.isDir) !== entry.isDir)
    return false
  const name = entry.name.toLowerCase()
  const extension = entry.extension.toLowerCase()
  const mime = entry.mime.toLowerCase()
  return (
    rule.names.some((value) => value === name) ||
    rule.extensions.some((value) => value === extension) ||
    rule.mime.some(
      (value) => value === mime || (value.endsWith('/*') && mime.startsWith(value.slice(0, -1))),
    )
  )
}

export async function pluginIconFor(entry: Entry): Promise<string | null> {
  const list = await load()
  const rule = list.find((item) => matches(item, entry))
  if (!rule) return null
  if (!native) return rule.path
  // Plugin icons are local files inside the installed package; the asset
  // protocol gives the renderer a URL without exposing a plugin origin.
  return convertFileSrc(rule.path)
}
