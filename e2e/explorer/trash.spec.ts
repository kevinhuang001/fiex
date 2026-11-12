import { test, expect } from '@playwright/test'
import { openDocuments } from '../fixtures/helpers'

test.beforeEach(async ({ page }) => {
  await page.goto('/?fixture&pluginFixture')
  await openDocuments(page)
})

test('restoring a Trash item returns it to its original directory', async ({ page }) => {
  await page.getByRole('button', { name: '废纸篓', exact: true }).click()
  const trashedFile = page.getByText('Old draft.txt', { exact: true })
  await expect(trashedFile).toBeVisible()
  await trashedFile.click({ button: 'right' })
  await page.getByRole('menuitem', { name: '放回原处', exact: true }).click()

  await expect(trashedFile).toHaveCount(0)
  await openDocuments(page)
  await expect(page.locator('[data-path="/Users/kevin/Documents/Old draft.txt"]')).toBeVisible()
})
