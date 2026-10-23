// Copies the MIT-licensed Material Icon Theme assets used for file-type icons.
// The icon set is a build-time dependency; only SVG files referenced by its
// manifest are published, together with the upstream licence.
import { copyFileSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { basename, dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const require = createRequire(import.meta.url)
const root = join(dirname(fileURLToPath(import.meta.url)), '..')
const packageDir = dirname(require.resolve('material-icon-theme/package.json'))
const manifest = JSON.parse(readFileSync(join(packageDir, 'dist', 'material-icons.json'), 'utf8'))
const output = join(root, 'public', 'file-icons')

mkdirSync(output, { recursive: true })

const assets = new Set()
for (const definition of Object.values(manifest.iconDefinitions)) {
  if (definition && definition.iconPath) assets.add(basename(definition.iconPath))
}
for (const name of assets) {
  copyFileSync(join(packageDir, 'icons', name), join(output, name))
}
// Remove icons from older releases without deleting the directory itself:
// recreating it would break a running Vite dev server's public-file index.
for (const name of readdirSync(output))
  if (name !== 'LICENSE' && !assets.has(name))
    rmSync(join(output, name), { recursive: true, force: true })
// Material Icon Theme exposes folder colours through its configuration; apply
// the same blue to the generic folder so it reads like Finder instead of the
// upstream grey-blue. This only replaces a fill value in the MIT asset.
for (const name of ['folder.svg', 'folder-open.svg']) {
  const file = join(output, name)
  writeFileSync(file, readFileSync(file, 'utf8').replaceAll('#90a4ae', '#4a9fd8'))
}
writeFileSync(join(output, 'LICENSE'), readFileSync(join(packageDir, 'LICENSE'), 'utf8'))
console.log(`Copied ${assets.size} Material Icon Theme file icons to public/file-icons.`)
