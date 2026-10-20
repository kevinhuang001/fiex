export interface GitState {
  letter: string
  label: string
  staged: boolean
  unstaged: boolean
}

const NAMES: Record<string, string> = {
  M: '修改',
  A: '新增',
  D: '删除',
  R: '重命名',
  C: '复制',
  T: '类型变更',
  U: '冲突',
}

export function gitState(code: string | undefined): GitState | null {
  if (!code) return null
  if (code === '??') {
    return { letter: 'U', label: '未跟踪', staged: false, unstaged: true }
  }
  const index = code[0] || ' '
  const work = code[1] || ' '
  const indexStatus = index !== ' ' && index !== '?'
  const workStatus = work !== ' ' && work !== '?'
  const letter = workStatus ? work : index
  if (letter === ' ') return null
  const name = NAMES[letter] || letter
  let label = name
  if (indexStatus && workStatus) label = `已暂存并${name}`
  else if (indexStatus) label = `已暂存${name}`
  else label = `未暂存${name}`
  return { letter, label, staged: indexStatus, unstaged: workStatus }
}
