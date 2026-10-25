import { defineConfig, type Plugin } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import { fileURLToPath, URL } from 'node:url'
import fs from 'node:fs'
import path from 'node:path'
const officialPluginRoot = fileURLToPath(
  new URL('../fiex-official-plugins/official', import.meta.url),
)
function officialPluginDevServer(): Plugin {
  return {
    name: 'fiex-official-plugin-dev-server',
    configureServer(server) {
      server.middlewares.use('/__fiex_official_plugin', (req, res, next) => {
        const slug = (req.url || '').split('/')[1]?.replace(/[^a-z0-9_-]/gi, '')
        if (!slug) return next()
        const built = path.join(officialPluginRoot, slug, 'ui', 'dist', 'index.html')
        // Self-contained plugins ship ui/index.html without a build step.
        const source = path.join(officialPluginRoot, slug, 'ui', 'index.html')
        const file = fs.existsSync(built) ? built : source
        if (!fs.existsSync(file)) {
          res.statusCode = 404
          res.end(`official plugin UI not built: ${slug}`)
          return
        }
        res.setHeader('Content-Type', 'text/html; charset=utf-8')
        res.end(fs.readFileSync(file, 'utf8'))
      })
    },
  }
}
/** Read installed-shape theme packages from the sibling official plugins
 * checkout. Only the development server uses this: the browser demo has no
 * plugin host, so without it theme packages could not be previewed in the web
 * build. Production always reads themes through Core. */
function readOfficialThemes() {
  const themes: Array<Record<string, unknown>> = []
  if (!fs.existsSync(officialPluginRoot)) return themes
  for (const entry of fs.readdirSync(officialPluginRoot, { withFileTypes: true })) {
    if (!entry.isDirectory()) continue
    const folder = path.join(officialPluginRoot, entry.name)
    try {
      const manifest = JSON.parse(fs.readFileSync(path.join(folder, 'manifest.json'), 'utf8'))
      for (const contribution of manifest.extensions?.themes || []) {
        const file = path.resolve(folder, String(contribution.path || ''))
        if (!file.startsWith(folder + path.sep) || !fs.existsSync(file)) continue
        const theme = JSON.parse(fs.readFileSync(file, 'utf8'))
        // Core namespaces theme ids by plugin id; mirror it so a previewed id
        // always matches the installed one.
        themes.push({
          id: `${manifest.id}:${theme.id}`,
          name: theme.name,
          mode: theme.mode,
          tokens: theme.tokens || {},
          darkTokens: theme.darkTokens || {},
        })
      }
    } catch {
      // A malformed sibling package must not break the dev server.
    }
  }
  return themes
}
function officialThemeDevServer(): Plugin {
  return {
    name: 'fiex-official-theme-dev-server',
    configureServer(server) {
      server.middlewares.use('/__fiex_theme/index.json', (_req, res) => {
        res.setHeader('Content-Type', 'application/json; charset=utf-8')
        res.end(JSON.stringify(readOfficialThemes()))
      })
    },
  }
}
export default defineConfig({
  plugins: [react(), tailwindcss(), officialPluginDevServer(), officialThemeDevServer()],
  resolve: { alias: { '@': fileURLToPath(new URL('./src', import.meta.url)) } },
  server: { strictPort: true },
  optimizeDeps: { include: ['pdfjs-dist/legacy/build/pdf.mjs'] },
  clearScreen: false,
})
