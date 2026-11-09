#!/usr/bin/env node
import { readFileSync } from 'node:fs'

const handlerSource = readFileSync('src-tauri/src/desktop/mod.rs', 'utf8')
const handlerBody = handlerSource.match(/generate_handler!\[([\s\S]*?)\]/)?.[1]
if (!handlerBody) throw new Error('Could not find the Tauri command handler list')

const registered = handlerBody
  .split(',')
  .map((entry) => entry.trim().split('::').at(-1))
  .filter(Boolean)
const uniqueRegistered = new Set(registered)
if (uniqueRegistered.size !== registered.length) {
  throw new Error('Tauri command handler contains duplicate command names')
}

function keysFromInterface(file, interfaceName) {
  const source = readFileSync(file, 'utf8')
  const body = source.match(new RegExp(`interface ${interfaceName}\\s*\\{([\\s\\S]*?)\\n\\}`))?.[1]
  if (!body) throw new Error(`Could not find ${interfaceName} in ${file}`)
  return [...body.matchAll(/^ {2}([a-z][\w]*):/gm)].map((match) => match[1])
}

const argumentKeys = keysFromInterface('src/lib/tauriCommandArgs.ts', 'TauriCommandArgs')
const resultKeys = keysFromInterface('src/lib/tauriCommandMap.ts', 'TauriCommandResults')

function assertSameCommands(label, actual) {
  const actualSet = new Set(actual)
  const missing = [...uniqueRegistered].filter((name) => !actualSet.has(name))
  const stale = [...actualSet].filter((name) => !uniqueRegistered.has(name))
  const duplicateCount = actual.length - actualSet.size
  if (missing.length || stale.length || duplicateCount) {
    throw new Error(
      `${label} command map differs from Rust handler. Missing: ${missing.join(', ') || 'none'}; ` +
        `stale: ${stale.join(', ') || 'none'}; duplicates: ${duplicateCount}`,
    )
  }
}

assertSameCommands('Argument', argumentKeys)
assertSameCommands('Result', resultKeys)
console.log(`Tauri command contract maps cover all ${registered.length} registered commands.`)
