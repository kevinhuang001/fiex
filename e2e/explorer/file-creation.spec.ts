import { test, expect } from '@playwright/test'
import { openDocuments } from '../fixtures/helpers'

test.beforeEach(async ({ page }) => {
  await page.goto('/?fixture&pluginFixture')
  await openDocuments(page)
})

async function openBlankCanvasMenu(page: import('@playwright/test').Page) {
  const grid = page.getByRole('grid', { name: '文件' })
  const box = await grid.boundingBox()
  if (!box) throw new Error('missing file grid geometry')
  await grid.click({
    button: 'right',
    position: { x: box.width - 20, y: box.height - 20 },
  })
  return page.getByRole('menu', { name: '文件操作' })
}

test('New File offers a blank file and valid Office templates', async ({ page }) => {
  const menu = await openBlankCanvasMenu(page)
  await menu.getByRole('menuitem', { name: '新建文件', exact: true }).click()
  await page.getByRole('menuitem', { name: '空白文件 (.txt)', exact: true }).click()
  await page.keyboard.press('Enter')
  await expect(page.getByText('未命名文件.txt', { exact: true })).toBeVisible()

  for (const [format, filename] of [
    ['docx', '未命名文档.docx'],
    ['xlsx', '未命名工作簿.xlsx'],
    ['pptx', '未命名演示文稿.pptx'],
  ]) {
    const context = await openBlankCanvasMenu(page)
    await context.getByRole('menuitem', { name: '新建文件', exact: true }).click()
    await page.getByRole('menuitem', { name: 'Office', exact: true }).click()
    await page.getByRole('menuitem', { name: new RegExp(`\\.${format}\\)$`) }).click()
    await page.keyboard.press('Enter')
    await expect(page.getByText(filename, { exact: true })).toBeVisible()
  }
})

test('Office context menu converts DOCX, XLSX and PPTX to PDF', async ({ page }) => {
  for (const source of ['季度报告.docx', '预算表.xlsx', '季度演示.pptx']) {
    const file = page.locator(`[data-path="/Users/kevin/Documents/${source}"]`)
    await file.click({ button: 'right' })
    const menu = page.getByRole('menu', { name: '文件操作' })
    await menu.getByRole('menuitem', { name: '插件', exact: true }).click()
    await page.getByRole('menuitem', { name: 'Office', exact: true }).click()
    await page.getByRole('menuitem', { name: '转换为 PDF', exact: true }).click()
    const pdfName = source.replace(/\.[^.]+$/, '.pdf')
    await expect(page.getByText(pdfName, { exact: true })).toBeVisible()
    await expect(file).toBeVisible()
  }
})
