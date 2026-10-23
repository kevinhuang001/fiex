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
        const file = path.join(officialPluginRoot, slug, 'ui', 'dist', 'index.html')
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
export default defineConfig({
  plugins: [react(), tailwindcss(), officialPluginDevServer()],
  resolve: { alias: { '@': fileURLToPath(new URL('./src', import.meta.url)) } },
  server: { strictPort: true },
  optimizeDeps: { include: ['pdfjs-dist/legacy/build/pdf.mjs'] },
  clearScreen: false,
})
