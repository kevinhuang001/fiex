import type { PluginInfo } from './plugins'

export const commandRisk =
  '外部命令以当前用户身份运行，可能访问授权目录以外的文件、连接网络或启动其他程序。仅在信任插件及其申请的命令时授权。'

export const permissionLabels: Record<string, string> = {
  metadata: '读取基础属性',
  'folders.read': '读取文件夹',
  'folders.create': '创建目录项',
  'folders.delete': '删除目录项',
  'folders.rename': '重命名目录项',
  'files.read': '读取文件',
  'files.write': '修改文件',
  'files.create': '创建文件',
  'files.delete': '删除文件',
  'files.rename': '重命名文件',
  network: '网络访问',
  commands: '执行外部命令',
  hooks: '打开前钩子',
}
export const permissionHints: Record<string, string> = {
  metadata: '读取本次授权范围内文件和文件夹的名称、大小、类型等属性；不读取内容或枚举目录。',
  'folders.read': '列出本次授权范围内的文件夹。',
  'folders.create': '在当前目录及子目录创建文件或文件夹。',
  'folders.delete': '删除当前目录内的文件或文件夹，包括其内容。',
  'folders.rename': '重命名或移动当前目录内的目录项。',
  'files.read': '读取当前目录及子目录内的文件内容。',
  'files.write': '修改文件内容；通过暂存和提交保存，发生冲突时停止。',
  'files.create': '在本次授权范围内创建不存在的文件。',
  'files.delete': '删除本次授权范围内的文件。',
  'files.rename': '重命名本次授权范围内的文件。',
  network: '通过宿主访问单独授权的网络目标，插件自身不能直接联网。',
  commands: commandRisk,
  hooks:
    '在打开文件前接管操作（例如把压缩包直接解压到当前目录）。高优先级先执行，钩子可以选择放行或消费这次打开。',
}
export interface ActivationConsent {
  permissions: string[]
  networkAll: boolean
  domains: string[]
  commands: string[]
}

/** Everything the manifest declares and requests: the default selection of the
 * grouped consent sheet. Activation only ever narrows this set. */
export function activationConsent(plugin: PluginInfo): ActivationConsent {
  return {
    permissions: [...new Set(plugin.declaredPermissions || [])].sort(),
    networkAll: Boolean(plugin.requestedNetworkAll),
    domains: [...new Set(plugin.requestedNetworkDomains || [])].sort(),
    commands: [...new Set(plugin.requestedCommands || [])].sort(),
  }
}

const activationSections = [
  {
    id: 'files',
    title: '文件与文件夹',
    hint: '读取或修改授权目录中的文件与文件夹属性、目录项和内容。',
  },
  {
    id: 'network',
    title: '网络与域名',
    hint: '插件只能通过宿主访问下面单独勾选的目标，自身不能直接联网。',
  },
  {
    id: 'commands',
    title: '外部命令',
    hint: '命令由宿主代理执行，插件进程不能直接启动程序。',
  },
  {
    id: 'other',
    title: '其他能力',
    hint: '不属于以上分组的宿主能力。',
  },
] as const

export type ActivationSectionId = (typeof activationSections)[number]['id']

const permissionOrder = [
  'metadata',
  'folders.read',
  'folders.create',
  'folders.delete',
  'folders.rename',
  'files.read',
  'files.write',
  'files.create',
  'files.delete',
  'files.rename',
  'network',
  'commands',
  'hooks',
]
const rank = (value: string) => {
  const index = permissionOrder.indexOf(value)
  return index === -1 ? permissionOrder.length : index
}

/** Files and folders share one review section; network targets stay with the
 * network capability and commands with the command capability. */
export function permissionSection(permission: string): ActivationSectionId {
  if (permission === 'network') return 'network'
  if (permission === 'commands') return 'commands'
  if (
    permission === 'metadata' ||
    permission.startsWith('files.') ||
    permission.startsWith('folders.')
  )
    return 'files'
  return 'other'
}

export interface ActivationPermission {
  id: string
  label: string
  hint?: string
}

export interface ActivationCommand {
  name: string
  missing: boolean
}

export interface ActivationGroup {
  id: ActivationSectionId
  title: string
  hint: string
  permissions: ActivationPermission[]
  /** Exact hosts requested by the manifest; only network has these. */
  domains: string[]
  /** Exact executable names requested by the manifest. */
  commands: ActivationCommand[]
  /** The manifest asked for unrestricted network access. */
  networkAll: boolean
}

/**
 * Build the grouped review model for one plugin. Every declared permission,
 * requested domain and command becomes its own row, and the groups follow a
 * fixed order so the review is predictable.
 */
export function activationGroups(plugin: PluginInfo): ActivationGroup[] {
  const consent = activationConsent(plugin)
  const groups: ActivationGroup[] = activationSections.map((section) => ({
    ...section,
    permissions: [],
    domains: [],
    commands: [],
    networkAll: false,
  }))
  for (const permission of [...consent.permissions].sort((a, b) => rank(a) - rank(b))) {
    const group = groups.find((entry) => entry.id === permissionSection(permission))
    group?.permissions.push({
      id: permission,
      label: permissionLabels[permission] || permission,
      hint: permissionHints[permission],
    })
  }
  const network = groups.find((entry) => entry.id === 'network')
  if (network) {
    network.domains = consent.domains
    network.networkAll = consent.networkAll
  }
  const commands = groups.find((entry) => entry.id === 'commands')
  if (commands) {
    commands.commands = consent.commands.map((name) => ({
      name,
      missing: Boolean(plugin.missingCommands?.includes(name)),
    }))
  }
  return groups.filter(
    (group) =>
      group.permissions.length > 0 ||
      group.domains.length > 0 ||
      group.commands.length > 0 ||
      group.networkAll,
  )
}
