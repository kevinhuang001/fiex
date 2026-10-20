import { chromium, expect } from '@playwright/test'
import { mkdir, rename } from 'node:fs/promises'
import { resolve } from 'node:path'
import { createServer } from 'vite'

// Record the same explicitly enabled fixtures used by the UI regression suite.
const output = resolve('artifacts/demo')
await mkdir(output, { recursive: true })
const server = process.env.FIEX_DEMO_URL
  ? undefined
  : await createServer({
      server: {
        host: '127.0.0.1',
        port: 1420,
        strictPort: true,
        watch: { ignored: ['**/artifacts/**', '**/playwright-report/**', '**/test-results/**'] },
      },
    })
await server?.listen()
const browser = await chromium.launch({
  executablePath: process.env.FIEX_TEST_CHROMIUM || undefined,
  args: ['--no-sandbox', '--disable-dev-shm-usage'],
})
const context = await browser.newContext({
  viewport: { width: 1440, height: 900 },
  recordVideo: { dir: output, size: { width: 1440, height: 900 } },
})
const page = await context.newPage()
const errors = []
page.on('pageerror', (error) => errors.push(error.message))
const pause = () => page.waitForTimeout(1600)
const button = (name) => page.getByRole('button', { name, exact: true })
try {
  await page.goto(process.env.FIEX_DEMO_URL || 'http://127.0.0.1:1420/?demo')
  await expect(page.getByText('Notes.md', { exact: true })).toBeVisible()
  await pause()
  // Verify a completed move, then add the destination folder to Favorites.
  const meeting = page.locator('[data-path="/Users/kevin/Documents/Meeting.txt"]')
  const research = page.locator('[data-path="/Users/kevin/Documents/Research"]')
  const from = await meeting.boundingBox(),
    to = await research.boundingBox()
  if (!from || !to) throw new Error('Missing drag geometry')
  await page.mouse.move(from.x + 35, from.y + from.height / 2)
  await page.mouse.down()
  await page.mouse.move(to.x + 35, to.y + to.height / 2, { steps: 18 })
  await page.mouse.up()
  await pause()
  await button('移动（保留两者）').click()
  await expect(meeting).toHaveCount(0)
  await research.dblclick()
  await expect(page.getByText('Meeting.txt', { exact: true })).toBeVisible()
  await pause()
  await button('后退').click()
  const folder = await research.boundingBox(),
    favorites = await page.locator('.favorites-section').boundingBox()
  if (!folder || !favorites) throw new Error('Missing favorite geometry')
  await page.mouse.move(folder.x + 35, folder.y + folder.height / 2)
  await page.mouse.down()
  await page.mouse.move(favorites.x + 50, favorites.y + 15, { steps: 18 })
  await page.mouse.up()
  await expect(page.locator('.favorite-row').filter({ hasText: 'Research' })).toBeVisible()
  await pause()
  await button('源代码管理').click()
  await pause()
  await button('暂存 Notes.md').click()
  await pause()
  await button('提交修改 · 1').click()
  await pause()
  await page.getByRole('textbox', { name: '提交说明' }).fill('整理项目说明与文件结构')
  await pause()
  await button('提交 1 个文件').click()
  await expect(page.getByText('工作区干净', { exact: true })).toBeVisible()
  await pause()
  await button('选择 commit').click()
  await pause()
  await button('查看提交 aaaaaaaa').click()
  await expect(page.locator('.history-banner')).toContainText('只读')
  await pause()
  await button('关闭源代码管理').click()
  await page.getByText('Notes.md', { exact: true }).first().click()
  await page.keyboard.press('Space')
  await expect(page.getByRole('dialog').locator('pre')).toContainText('当前工作区没有改变')
  await pause()
  await page.keyboard.press('Space')
  await page.locator('.history-banner').getByRole('button', { name: '返回工作区' }).click()
  await expect(button('新建文件夹')).toBeEnabled()
  await pause()
  await page.locator('.sidebar').getByRole('button', { name: '下载', exact: true }).click()
  await button('源代码管理').click()
  await pause()
  await button('初始化 Git 仓库').click()
  await expect(button('选择 commit')).toContainText('main')
  await pause()
  await button('关闭源代码管理').click()
  await page.locator('.sidebar').getByRole('button', { name: '文稿', exact: true }).click()
  await page.goto('http://127.0.0.1:1420/?demo&count=10000')
  await expect(page.locator('.statusbar')).toContainText('10,010')
  await button('图标视图').click()
  await pause()
  await page.getByRole('grid', { name: '文件', exact: true }).focus()
  await page.keyboard.press('End')
  await expect(page.locator('[data-path="/Users/kevin/Documents/zz-file-09999.txt"]')).toBeVisible()
  await pause()
  await page.keyboard.press('Home')
  await pause()
  if (errors.length) throw new Error(errors.join('\n'))
} catch (error) {
  console.error(error)
  throw error
} finally {
  await context.close()
  try {
    const video = await page.video().path()
    await rename(video, resolve(output, 'fiex-current-demo.webm'))
  } finally {
    await browser.close()
    await server?.close()
  }
}
console.log(resolve(output, 'fiex-current-demo.webm'))
