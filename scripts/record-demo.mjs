import { chromium, expect } from '@playwright/test'
import { mkdir, rename } from 'node:fs/promises'
import { resolve } from 'node:path'
import { createServer } from 'vite'

// Record the same explicitly enabled fixtures used by the UI regression suite.
const output = resolve('artifacts/demo')
await mkdir(output, { recursive: true })
const server = process.env.FIEX_DEMO_URL
  ? undefined
  : await createServer({ server: { host: '127.0.0.1', port: 1420, strictPort: true } })
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
  for (const view of ['图标', '画廊', '分栏', '列表']) {
    await button(view + '视图').click()
    await pause()
  }
  await button('分组方式').click()
  await page.getByRole('menuitem', { name: '种类', exact: true }).click()
  await pause()
  await button('分组方式').click()
  await page.getByRole('menuitem', { name: '无', exact: true }).click()
  await page.getByText('Research', { exact: true }).dblclick()
  await pause()
  await button('编辑路径').click()
  await pause()
  await page.keyboard.press('Escape')
  await button('后退').click()
  await button('新建标签页').click()
  await pause()
  await button('关闭标签页').last().click()
  await page.getByRole('textbox', { name: '搜索文件' }).fill('insb')
  await expect(page.getByText('InSb.pdf', { exact: true })).toBeVisible()
  await pause()
  await button('清除搜索').click()
  await page.getByText('Notes.md', { exact: true }).click()
  await page.keyboard.press('Space')
  await expect(page.getByRole('dialog')).toBeVisible()
  await pause()
  await page.keyboard.press('Space')
  await button('新建文件夹').click()
  const renameInput = page.getByRole('textbox', { name: /重命名 未命名文件夹/ })
  await renameInput.fill('演示文件夹')
  await pause()
  await renameInput.press('Enter')
  await pause()
  await page.getByText('Notes.md', { exact: true }).click({ button: 'right' })
  await page.getByRole('menuitem', { name: '压缩为…', exact: true }).hover()
  await pause()
  await page.keyboard.press('Escape')
  await button('源代码管理').click()
  await pause()
  await button('关闭源代码管理').click()
  await button('添加 WebDAV 或 SMB 连接').click()
  await pause()
  await button('SMB').click()
  await pause()
  await button('关闭').click()
  await button('设置').click()
  await pause()
  for (const section of ['快捷键', '侧边栏', '忽略', '启动方式', '关于', '外观']) {
    await button(section).click()
    await pause()
  }
  await button('深色').click()
  await pause()
  await button('关闭').click()
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
