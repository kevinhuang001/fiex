import type { ReactNode } from 'react'
import { ArchiveIcon as Archive } from '@phosphor-icons/react/dist/csr/Archive'
import { CloudIcon as Cloud } from '@phosphor-icons/react/dist/csr/Cloud'
import { GitBranchIcon as GitBranch } from '@phosphor-icons/react/dist/csr/GitBranch'
import { HardDriveIcon as HardDrive } from '@phosphor-icons/react/dist/csr/HardDrive'
import { HashIcon as Hash } from '@phosphor-icons/react/dist/csr/Hash'
import { PuzzlePieceIcon as Puzzle } from '@phosphor-icons/react/dist/csr/PuzzlePiece'

const pluginIconMap: Record<string, ReactNode> = {
  'git-branch': <GitBranch weight="regular" size={20} />,
  archive: <Archive weight="regular" size={20} />,
  hash: <Hash weight="regular" size={20} />,
  cloud: <Cloud weight="regular" size={20} />,
  'hard-drive': <HardDrive weight="regular" size={20} />,
}

export function pluginIcon(name?: string) {
  return pluginIconMap[name || ''] || <Puzzle weight="regular" size={20} />
}

export const pluginKey = (pluginId: string, toolId: string) => pluginId + '::' + toolId
