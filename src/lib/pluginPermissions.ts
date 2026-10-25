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
export function activationConsent(plugin: PluginInfo): ActivationConsent {
  return {
    permissions: [...new Set(plugin.declaredPermissions || [])].sort(),
    networkAll: Boolean(plugin.requestedNetworkAll),
    domains: [...new Set(plugin.requestedNetworkDomains || [])].sort(),
    commands: [...new Set(plugin.requestedCommands || [])].sort(),
  }
}
