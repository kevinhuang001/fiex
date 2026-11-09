import { test, expect } from '@playwright/test'
import { openDocuments } from '../fixtures/helpers'

test.beforeEach(async ({ page }) => {
  await page.goto('/?fixture&pluginFixture')
  await openDocuments(page)
})

test('status center records transfers and shows source and destination', async ({ page }) => {
  // Creates are intentionally not recorded, so they must not appear here.
  await page.getByRole('button', { name: '新建文件夹', exact: true }).click()
  await page.keyboard.press('Escape')
  await page.locator('[data-path="/Users/kevin/Documents/Notes.md"]').click()
  await page.keyboard.press('Control+c')
  await page.getByText('Projects', { exact: true }).dblclick()
  await page.keyboard.press('Control+v')
  const center = page.getByRole('complementary', { name: '状态中心' })
  const openCenter = page.getByRole('button', { name: '状态中心', exact: true })
  if (!(await center.isVisible())) await openCenter.click()
  await expect(center.getByText('复制', { exact: true })).toBeVisible()
  await expect(center.getByText('新建文件夹', { exact: true })).toHaveCount(0)
  await center.getByRole('button', { name: /查看详情：复制/ }).click()
  await expect(center.getByText('复制来源：')).toBeVisible()
  await expect(center.locator('.activity-detail-message')).toContainText(
    '/Users/kevin/Documents/Notes.md',
  )
  await expect(center.locator('.activity-detail-message')).toContainText(
    '/Users/kevin/Documents/Projects',
  )
  await center.getByRole('button', { name: '返回操作列表' }).click()
  await expect(center.getByText('开始时间')).toHaveCount(0)
})

test('inline rename works in icon and column views', async ({ page }) => {
  await page.getByRole('button', { name: '图标视图', exact: true }).click()
  await page.getByText('Notes.md', { exact: true }).click()
  await page.keyboard.press('F2')
  await expect(page.getByRole('textbox', { name: /重命名 Notes.md/ })).toBeVisible()
  await page.keyboard.press('Escape')
  await page.getByRole('button', { name: '分栏视图', exact: true }).click()
  await page.locator('.column-label', { hasText: 'Notes.md' }).click()
  await page.keyboard.press('F2')
  await expect(page.getByRole('textbox', { name: /重命名 Notes.md/ })).toBeVisible()
  await page.keyboard.press('Escape')
})

test('dragging moves files and bookmarks folders', async ({ page }) => {
  const source = page.locator('[data-path="/Users/kevin/Documents/Notes.md"]')
  const research = page.locator('[data-path="/Users/kevin/Documents/Research"]')
  const sourceBox = await source.boundingBox()
  const researchBox = await research.boundingBox()
  if (!sourceBox || !researchBox) throw new Error('missing drag geometry')
  await page.mouse.move(sourceBox.x + 30, sourceBox.y + sourceBox.height / 2)
  await page.mouse.down()
  await page.mouse.move(researchBox.x + 30, researchBox.y + researchBox.height / 2, { steps: 8 })
  await page.mouse.up()
  await expect(source).toHaveCount(0)
  await expect(page.getByRole('dialog')).toHaveCount(0)

  const folderBox = await research.boundingBox()
  const bookmarksBox = await page.locator('.favorites-section').boundingBox()
  if (!folderBox || !bookmarksBox) throw new Error('missing bookmark geometry')
  await page.mouse.move(folderBox.x + 30, folderBox.y + folderBox.height / 2)
  await page.mouse.down()
  await page.mouse.move(bookmarksBox.x + bookmarksBox.width / 2, bookmarksBox.y + 18, { steps: 8 })
  await page.mouse.up()
  await expect(
    page.locator('.favorite-row .nav-item').filter({ hasText: 'Research' }),
  ).toBeVisible()
})

test('batch rename previews and applies a pattern', async ({ page }) => {
  await page.getByText('Notes.md', { exact: true }).click()
  await page.getByText('Meeting.txt', { exact: true }).click({ modifiers: ['Control'] })
  await page.locator('[data-path="/Users/kevin/Documents/Notes.md"]').click({ button: 'right' })
  await page.getByRole('menuitem', { name: '批量重命名…' }).click()
  await page.getByRole('textbox', { name: '文件名格式' }).fill('{name}_renamed')
  await expect(page.locator('.batch-rename-list')).toContainText('Notes_renamed.md')
  await page.getByRole('button', { name: '开始重命名', exact: true }).click()
  await expect(page.getByText('Notes_renamed.md', { exact: true })).toBeVisible()
  await expect(page.getByText('Meeting_renamed.txt', { exact: true })).toBeVisible()
})

test('multi-select and select all', async ({ page }) => {
  await page.locator('[data-path="/Users/kevin/Documents/Notes.md"]').click()
  await page
    .locator('[data-path="/Users/kevin/Documents/Meeting.txt"]')
    .click({ modifiers: ['Control'] })
  await expect(page.locator('.statusbar')).toContainText('已选择 2 项')
  await page.keyboard.press('Control+a')
  await expect(page.locator('.statusbar')).toContainText('已选择 15 项')
  await page.locator('[data-path="/Users/kevin/Documents/Notes.md"]').click({ button: 'right' })
  await expect(page.getByRole('menuitem', { name: '快速查看' })).toHaveCount(0)
  await expect(page.getByRole('menuitem', { name: '显示简介' })).toHaveCount(0)
  await expect(page.getByRole('menuitem', { name: '重新命名' })).toHaveCount(0)
  await expect(page.getByRole('menuitem', { name: '批量重命名…' })).toBeVisible()
  await page.keyboard.press('Escape')
})

test('folders offer copy, cut and duplicate in their correct context menu sections', async ({
  page,
}) => {
  await page.locator('[data-path="/Users/kevin/Documents/Research"]').click({ button: 'right' })
  await expect(page.getByRole('menuitem', { name: '复制', exact: true })).toBeVisible()
  await expect(page.getByRole('menuitem', { name: '剪切', exact: true })).toBeVisible()
  await page.getByRole('menuitem', { name: '更多', exact: true }).click()
  await expect(page.getByRole('menuitem', { name: '创建副本', exact: true })).toBeVisible()
  await expect(page.getByRole('menuitem', { name: '彻底删除', exact: true })).toBeVisible()
})

test('complete a move into a folder and bookmark the folder', async ({ page }) => {
  const notes = page.locator('[data-path="/Users/kevin/Documents/Notes.md"]')
  const folder = page.locator('[data-path="/Users/kevin/Documents/Research"]')
  const source = await notes.boundingBox(),
    target = await folder.boundingBox()
  if (!source || !target) throw new Error('missing drag geometry')
  await page.mouse.move(source.x + 30, source.y + source.height / 2)
  await page.mouse.down()
  await page.mouse.move(target.x + 30, target.y + target.height / 2, { steps: 12 })
  await page.mouse.up()
  await expect(page.getByRole('dialog')).toHaveCount(0)
  await expect(notes).toHaveCount(0)
  await folder.dblclick()
  await expect(page.getByText('Notes.md', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: '后退', exact: true }).click()
  const box = await folder.boundingBox(),
    favorites = await page.locator('.favorites-section').boundingBox()
  if (!box || !favorites) throw new Error('missing bookmark geometry')
  await page.mouse.move(box.x + 30, box.y + box.height / 2)
  await page.mouse.down()
  await page.mouse.move(favorites.x + 30, favorites.y + 15, { steps: 12 })
  await page.mouse.up()
  await expect(page.locator('.favorite-row').filter({ hasText: 'Research' })).toBeVisible()
  await expect(folder).toBeVisible()
  await expect(
    page.locator('.favorite-row').filter({ hasText: 'Research' }).locator('img'),
  ).toHaveAttribute('src', (await folder.locator('.file-icon img').getAttribute('src'))!)
})

test('copy conflicts offer compare and keep both, ordinary pastes need no confirmation', async ({
  page,
}) => {
  const file = page.locator('[data-path="/Users/kevin/Documents/Notes.md"]')
  await file.click()
  await page.keyboard.press('Control+c')
  await page.keyboard.press('Control+v')
  const dialog = page.getByRole('dialog', { name: '替换或跳过文件' })
  await expect(dialog).toBeVisible()
  await dialog.getByRole('button', { name: '逐个比较文件' }).click()
  await expect(dialog.getByRole('checkbox', { name: '传入 Notes.md' })).toBeChecked()
  await expect(dialog.getByRole('checkbox', { name: '目标 Notes.md' })).toBeChecked()
  await dialog.getByRole('button', { name: '继续', exact: true }).click()
  await expect(page.getByText('Notes (2).md', { exact: true })).toBeVisible()
  await expect(file).toBeVisible()
  await expect(dialog).toHaveCount(0)
})
