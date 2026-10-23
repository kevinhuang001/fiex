import { ArchiveIcon as Archive } from '@phosphor-icons/react/dist/csr/Archive'
import { CloudIcon as Cloud } from '@phosphor-icons/react/dist/csr/Cloud'
import { GitBranchIcon as GitBranch } from '@phosphor-icons/react/dist/csr/GitBranch'
import { HardDriveIcon as HardDrive } from '@phosphor-icons/react/dist/csr/HardDrive'
import { HashIcon as Hash } from '@phosphor-icons/react/dist/csr/Hash'
import { MagnifyingGlassIcon as MagnifyingGlass } from '@phosphor-icons/react/dist/csr/MagnifyingGlass'
import { PuzzlePieceIcon as PuzzlePiece } from '@phosphor-icons/react/dist/csr/PuzzlePiece'

const icons: Record<string, typeof PuzzlePiece> = {
  'git-branch': GitBranch,
  archive: Archive,
  hash: Hash,
  cloud: Cloud,
  'hard-drive': HardDrive,
  'magnifying-glass': MagnifyingGlass,
}

export function PluginGlyph({ icon, size = 16 }: { icon?: string; size?: number }) {
  const Icon = icons[icon || ''] || PuzzlePiece
  return <Icon weight="duotone" size={size} />
}
