// Temporary diagnostic helper: emit formatter output in bounded chunks for
// patch reconstruction; does not change files in CI.
import { execFileSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import prettier from 'prettier'

let changed = []
try {
  execFileSync('node', ['node_modules/prettier/bin/prettier.cjs', '--list-different', '.'], {
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
  })
} catch (error) {
  changed = String(error.stdout ?? '').split('\n').map((v) => v.trim()).filter(Boolean)
}
for (const path of changed) {
  const input = readFileSync(path, 'utf8')
  const config = (await prettier.resolveConfig(path)) ?? {}
  const formatted = await prettier.format(input, { ...config, filepath: path })
  const encoded = Buffer.from(formatted).toString('base64')
  for (let offset = 0; offset < encoded.length; offset += 2048) {
    console.log('FORMAT_PART|' + path + '|' + offset + '|' + encoded.slice(offset, offset + 2048))
  }
}
console.log('FORMAT_TOTAL|' + changed.length)
if (changed.length) process.exitCode = 1
