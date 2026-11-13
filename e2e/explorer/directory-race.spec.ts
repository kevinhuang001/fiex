import { test, expect } from '@playwright/test'

const delayedPath = '/Users/kevin/Documents'

test('a late listing cannot replace the directory opened afterward', async ({ page }) => {
  await page.goto('/?fixture')
  await page.getByRole('button', { name: '下载', exact: true }).click()
  await expect(page.getByRole('tab').last()).toContainText('Downloads')

  await page.evaluate(async (path) => {
    const fixtureModule = '/e2e/fixtures/index.ts'
    const fixture = await import(/* @vite-ignore */ fixtureModule)
    fixture.holdNextDirectoryListing(path)
  }, delayedPath)

  await page.getByRole('button', { name: '文稿', exact: true }).click()
  await expect
    .poll(() =>
      page.evaluate(async (path) => {
        const fixtureModule = '/e2e/fixtures/index.ts'
        const fixture = await import(/* @vite-ignore */ fixtureModule)
        return fixture.directoryListingWasRequested(path)
      }, delayedPath),
    )
    .toBe(true)

  await page.getByRole('button', { name: '下载', exact: true }).click()
  await expect(page.getByRole('tab').last()).toContainText('Downloads')
  await expect(page.getByText('Example.fiexplugin', { exact: true })).toBeVisible()

  await page.evaluate(async (path) => {
    const fixtureModule = '/e2e/fixtures/index.ts'
    const fixture = await import(/* @vite-ignore */ fixtureModule)
    fixture.releaseDirectoryListing(path)
  }, delayedPath)

  await expect(page.getByRole('tab').last()).toContainText('Downloads')
  await expect(page.getByText('Example.fiexplugin', { exact: true })).toBeVisible()
  await expect(page.getByText('Notes.md', { exact: true })).toHaveCount(0)
})
