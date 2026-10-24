import { Archive, Cloud, GitBranch, HardDrive, Hash, Puzzle, Search, Server } from 'lucide-react'
import type { ReactNode } from 'react'

export function pluginIcon(icon?: string) {
  const icons: Record<string, ReactNode> = {
    'git-branch': <GitBranch size={21} />,
    cloud: <Cloud size={21} />,
    server: <Server size={21} />,
    drive: <HardDrive size={21} />,
    archive: <Archive size={21} />,
    hash: <Hash size={21} />,
    search: <Search size={21} />,
    puzzle: <Puzzle size={21} />,
  }
  return icons[icon || 'puzzle'] || <Puzzle size={21} />
}
