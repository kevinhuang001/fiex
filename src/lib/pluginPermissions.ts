import type { PluginInfo } from './plugins'

export const commandRisk =
  '外部命令以当前用户身份运行，可能访问授权目录以外的文件、连接网络或启动其他程序。仅在信任插件及其申请的命令时授权。'

export const permissionLabels: Record<string, string> = {
  'folders.read': '读取文件夹',
  'folders.create': '创建目录项',
  'folders.delete': '删除目录项',
  'folders.rename': '重命名目录项',
  'folders.write': '管理目录项（旧版声明）',
  'files.read': '读取文件',
  'files.write': '修改文件',
  network: '网络访问',
  commands: '执行外部命令',
}
export const permissionHints: Record<string, string> = {
  'folders.read': '列出当前目录及子目录，读取文件名、大小等元数据。',
  'folders.create': '在当前目录及子目录创建文件或文件夹。',
  'folders.delete': '删除当前目录内的文件或文件夹，包括其内容。',
  'folders.rename': '重命名或移动当前目录内的目录项。',
  'files.read': '读取当前目录及子目录内的文件内容。',
  'files.write': '修改文件内容；通过暂存和提交保存，发生冲突时停止。',
  network: '通过宿主访问单独授权的网络目标，插件自身不能直接联网。',
  commands: commandRisk,
}
export interface ActivationConsent {
  permissions: string[]
  domains: string[]
  commands: string[]
}
export function activationConsent(plugin: PluginInfo): ActivationConsent {
  return {
    permissions: [...new Set(plugin.declaredPermissions || [])].sort(),
    domains: [...new Set(plugin.requestedNetworkDomains || [])].sort(),
    commands: [...new Set(plugin.requestedCommands || [])].sort(),
  }
}
