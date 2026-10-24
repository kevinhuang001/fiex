import { ArchiveIcon as Archive } from '@phosphor-icons/react/dist/csr/Archive'
import { CloudIcon as Cloud } from '@phosphor-icons/react/dist/csr/Cloud'
import { GitBranchIcon as GitBranch } from '@phosphor-icons/react/dist/csr/GitBranch'
import { HardDriveIcon as HardDrive } from '@phosphor-icons/react/dist/csr/HardDrive'
import { HashIcon as Hash } from '@phosphor-icons/react/dist/csr/Hash'
import { KeyIcon as Key } from '@phosphor-icons/react/dist/csr/Key'
import { MagnifyingGlassIcon as MagnifyingGlass } from '@phosphor-icons/react/dist/csr/MagnifyingGlass'
import { PlugIcon as Plug } from '@phosphor-icons/react/dist/csr/Plug'

const icons: Record<string, typeof Plug> = {
  'git-branch': GitBranch,
  archive: Archive,
  hash: Hash,
  cloud: Cloud,
  'hard-drive': HardDrive,
  'key-round': Key,
  'magnifying-glass': MagnifyingGlass,
}

export function PluginGlyph({ icon, size = 16 }: { icon?: string; size?: number }) {
  const Icon = icons[icon || ''] || Plug
  return <Icon weight="regular" size={size} />
}
