import {
  demoBranch,
  demoChanges,
  demoHistory,
  demoRoots,
  firstCommit,
  repoFor,
  secondCommit,
} from '../data'
import { demoView, demoViews, notHandled, state } from './shared'

const remoteConfigs = new Map<string, { name: string; url: string }[]>()

export function handleGit(command: string, a: Record<string, unknown>): unknown {
  switch (command) {
    case 'git_refs':
      return { head: 'aaaaaaaa', current: demoBranch, branches: ['main', 'dev'], tags: ['v0.2.4'] }
    case 'git_remote_status':
      return {
        remotes: remoteConfigs.get(String(a.root)) || [],
        branch: demoBranch,
        upstream: '',
        ahead: 0,
        behind: 0,
      }
    case 'git_remote_save': {
      const previous = remoteConfigs.get(String(a.root)) || []
      remoteConfigs.set(String(a.root), [
        ...previous.filter((r) => r.name !== a.name),
        { name: String(a.name), url: String(a.url) },
      ])
      return null
    }
    case 'git_sync':
      return a.push ? 'Pushed' : 'Already up to date'
    case 'git_identity':
      return { name: 'Kevin Huang', email: 'kevin@example.com' }
    case 'git_init': {
      const path = String(a.path)
      demoRoots.add(path)
      for (const entry of state.entries)
        if (!entry.isDir && entry.path.startsWith(path + '/')) demoChanges[entry.path] = '??'
      return {
        root: path,
        branch: 'main',
        entries: Object.fromEntries(
          Object.entries(demoChanges).filter(([p]) => p.startsWith(path + '/')),
        ),
      }
    }
    case 'git_stage':
    case 'git_unstage': {
      for (const path of Object.keys(demoChanges)) {
        if (
          !path.startsWith(String(a.root) + '/') ||
          (!a.all && !(a.paths as string[]).includes(path))
        )
          continue
        const code = demoChanges[path]
        demoChanges[path] =
          command === 'git_stage'
            ? code === '??'
              ? 'A '
              : `${code[1] === ' ' ? code[0] : code[1]} `
            : code[0] === 'A'
              ? '??'
              : ` ${code[0]}`
      }
      return null
    }
    case 'git_commit': {
      if (!String(a.message).trim()) throw new Error('请填写提交说明')
      const staged = Object.keys(demoChanges).filter(
        (path) =>
          path.startsWith(String(a.root) + '/') && ![' ', '?'].includes(demoChanges[path][0]),
      )
      if (!staged.length) throw new Error('没有暂存的更改')
      for (const path of staged) delete demoChanges[path]
      return '8e4a9c2'
    }
    case 'git_history':
      return demoHistory.slice(Number(a.skip) || 0, (Number(a.skip) || 0) + 50)
    case 'git_browse':
      return demoView(
        String(a.root),
        String(a.reference) === 'dev'
          ? secondCommit
          : String(a.reference).length === 40
            ? String(a.reference)
            : firstCommit,
      )
    case 'git_status': {
      const view = [...demoViews].find(([uri]) => String(a.path).startsWith(uri))?.[1]
      if (view) return { root: view.root, branch: view.commit.slice(0, 8), entries: {} }
      const repo = repoFor(String(a.path))
      return repo
        ? {
            root: repo,
            branch: demoBranch,
            entries: Object.fromEntries(
              Object.entries(demoChanges).filter(([p]) => p.startsWith(repo + '/')),
            ),
          }
        : null
    }

    default:
      return notHandled
  }
}
