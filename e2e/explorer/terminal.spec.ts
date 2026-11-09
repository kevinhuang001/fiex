import { test, expect } from '@playwright/test'
import { openDocuments } from '../fixtures/helpers'

test.beforeEach(async ({ page }) => {
  await page.goto('/?fixture&pluginFixture')
  await openDocuments(page)
})

test('create, rename, recycle and terminal selection', async ({ page }) => {
  await page.getByRole('button', { name: '新建文件夹', exact: true }).click()
  const created = page.getByRole('textbox', { name: /重命名 未命名文件夹/ })
  await expect(created).toBeVisible()
  await created.fill('Test folder')
  await created.press('Enter')
  await expect(page.getByText('Test folder', { exact: true })).toBeVisible()
  await page.getByText('Test folder', { exact: true }).click()
  await page.keyboard.press('F2')
  const renamed = page.getByRole('textbox', { name: /重命名 Test folder/ })
  await expect(renamed).toBeVisible()
  await renamed.fill('Renamed folder')
  await renamed.press('Enter')
  await expect(page.getByText('Renamed folder', { exact: true })).toBeVisible()
  await page.getByText('Renamed folder', { exact: true }).click()
  await page.keyboard.press('Delete')
  await expect(page.getByText('Renamed folder', { exact: true })).toHaveCount(0)
  await page.getByRole('button', { name: '设置', exact: true }).last().click()
  await page.getByRole('button', { name: '启动方式', exact: true }).click()
  await expect(page.getByText('默认 IDE', { exact: true })).toBeVisible()
  await expect(page.getByText('默认终端', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: '关闭', exact: true }).click()
})

test('built-in terminal opens an integrated bottom panel', async ({ page }) => {
  await page.goto('/?fixture')
  await openDocuments(page)
  await page.evaluate(() => {
    const key = 'fiex:e2e-state:explorer:preferences'
    const prefs = JSON.parse(localStorage.getItem(key) || '{}')
    localStorage.setItem(key, JSON.stringify({ ...prefs, terminal: 'builtin' }))
  })
  await page.reload()
  await openDocuments(page)
  const contentHeight = await page.locator('.pane-main').evaluate((element) => element.clientHeight)
  await page.getByRole('button', { name: 'Open in Terminal' }).click()
  await expect(page.locator('.terminal-panel')).toBeVisible()
  expect(await page.locator('.pane-main').evaluate((element) => element.clientHeight)).toBe(
    contentHeight,
  )
  await expect(page.locator('.terminal-panel-tab')).toHaveCount(1)
  await expect(page.locator('.terminal-panel-tab')).toContainText('zsh')
  const resizer = await page.locator('.terminal-panel-resizer').boundingBox()
  if (!resizer) throw new Error('terminal resizer is missing')
  await page.mouse.move(resizer.x + 20, resizer.y + 3)
  await page.mouse.down()
  await page.mouse.move(resizer.x + 20, resizer.y - 45, { steps: 4 })
  await page.mouse.up()
  expect(await page.locator('.pane-main').evaluate((element) => element.clientHeight)).toBe(
    contentHeight,
  )
  await page.getByRole('button', { name: '终端设置' }).click()
  const settings = page.getByRole('group', { name: '终端设置' })
  await settings.getByRole('slider').fill('17')
  await expect
    .poll(() =>
      page.evaluate(
        () => JSON.parse(localStorage.getItem('fiex:preferences') || '{}').terminalFontSize,
      ),
    )
    .toBe(17)
  // Terminal colors are not user settings: they follow the active theme.
  await expect(settings.getByLabel('字体颜色')).toHaveCount(0)
  expect(
    await page.evaluate(
      () => JSON.parse(localStorage.getItem('fiex:preferences') || '{}').terminalColor,
    ),
  ).toBeUndefined()
  await page.getByRole('button', { name: '终端设置' }).click()
  await page.getByRole('button', { name: '新建终端' }).click()
  await page.getByRole('menuitem', { name: 'bash' }).click()
  await expect(page.locator('.terminal-panel-tab')).toHaveCount(2)
  await page.getByRole('button', { name: '收起终端' }).click()
  await expect(page.locator('.terminal-panel')).toBeHidden()
  await page.getByRole('button', { name: 'Open in Terminal' }).click()
  await expect(page.locator('.terminal-panel')).toBeVisible()
  await expect(page.locator('.terminal-panel-tab')).toHaveCount(2)
  await page.getByRole('button', { name: '打开终端' }).click()
  await expect(page.locator('.terminal-panel')).toBeHidden()
  await page.getByRole('button', { name: '打开终端' }).click()
  await expect(page.locator('.terminal-panel')).toBeVisible()
})
