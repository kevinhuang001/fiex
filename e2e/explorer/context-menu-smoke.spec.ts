import { test, expect } from '@playwright/test'
import { openDocuments } from '../fixtures/helpers'

test.beforeEach(async ({ page }) => {
  await page.goto('/?fixture&pluginFixture')
  await openDocuments(page)
})

test('Applications and Trash context menus do not offer workspace actions', async ({ page }) => {
  await page.getByRole('button', { name: '应用程序', exact: true }).click()
  const application = page.locator('[data-path="applications://app_fiex"]')
  await expect(application).toBeVisible()
  await application.click({ button: 'right' })
  const applicationMenu = page.getByRole('menu', { name: '文件操作' })
  await expect(applicationMenu.getByRole('menuitem', { name: '添加到工作区' })).toHaveCount(0)
  await page.keyboard.press('Escape')

  await page.getByRole('button', { name: '废纸篓', exact: true }).click()
  await page.getByText('Old draft.txt', { exact: true }).click({ button: 'right' })
  const trashMenu = page.getByRole('menu', { name: '文件操作' })
  await trashMenu.getByRole('menuitem', { name: '更多', exact: true }).click()
  await expect(trashMenu.getByRole('menuitem', { name: '添加到工作区' })).toHaveCount(0)
})

test('multiple selected Trash items can be permanently deleted', async ({ page }) => {
  await page.getByRole('button', { name: '废纸篓', exact: true }).click()
  const first = page.getByText('Old draft.txt', { exact: true })
  const second = page.getByText('Old notes.txt', { exact: true })
  await first.click()
  await second.click({ modifiers: ['Control'] })
  await first.click({ button: 'right' })

  const menu = page.getByRole('menu', { name: '文件操作' })
  await menu.getByRole('menuitem', { name: '更多', exact: true }).click()
  await menu.getByRole('menuitem', { name: '彻底删除', exact: true }).click()

  const confirmation = page.getByRole('dialog', { name: '彻底删除 2 个项目？' })
  await expect(confirmation).toBeVisible()
  await confirmation.getByRole('button', { name: '彻底删除', exact: true }).click()
  await expect(page.getByText('Old draft.txt', { exact: true })).toHaveCount(0)
  await expect(page.getByText('Old notes.txt', { exact: true })).toHaveCount(0)
})

test('context submenus fit their labels, switch immediately, and stay open while hovered', async ({
  page,
}) => {
  const notes = page.locator('[data-path="/Users/kevin/Documents/Notes.md"]')
  const row = await notes.boundingBox()
  if (!row) throw new Error('missing file row geometry')
  await notes.click({ button: 'right', position: { x: row.width - 2, y: row.height - 2 } })

  const menu = page.getByRole('menu', { name: '文件操作' })
  await expect(menu).toBeVisible()
  await expect(menu).not.toHaveCSS('width', '210px')
  const menuBox = await menu.boundingBox()
  if (!menuBox) throw new Error('missing context menu geometry')
  expect(menuBox.x).toBeGreaterThanOrEqual(8)
  expect(menuBox.x + menuBox.width).toBeLessThanOrEqual(1192)

  await menu.getByRole('menuitem', { name: '更多', exact: true }).hover()
  const more = page.getByRole('menu', { name: '更多' })
  await expect(more).toBeVisible()
  await expect(more).not.toHaveCSS('width', '216px')
  await more.hover()
  await page.waitForTimeout(600)
  await expect(more).toBeVisible()

  await menu.getByRole('menuitem', { name: '插件', exact: true }).hover()
  await expect(page.getByRole('menu', { name: '插件操作' })).toBeVisible()
  await expect(more).toHaveCount(0)
})
