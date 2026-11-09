import { test, expect } from '@playwright/test'
import { openDocuments, chooseFixturePluginPackage } from '../fixtures/helpers'

test.beforeEach(async ({ page }) => {
  await page.goto('/?fixture&pluginFixture')
  await openDocuments(page)
})

test('activation reviews grouped permissions and grants nothing when cancelled', async ({
  page,
}) => {
  await page.getByRole('button', { name: '插件', exact: true }).click()
  await page.getByRole('menuitem', { name: '插件设置', exact: true }).click()
  const row = page.locator('.plugin-row').filter({ hasText: 'Git' })
  await row.getByRole('switch', { name: '停用 Git' }).click()
  await row.getByRole('switch', { name: '激活 Git' }).click()
  const sheet = page.getByRole('dialog', { name: '权限请求' })
  // One prompt per group; every declared capability starts selected.
  await expect(sheet.getByRole('checkbox', { name: '读取文件', exact: true })).toBeChecked()
  await sheet.getByRole('button', { name: '取消', exact: true }).click()
  await expect(sheet).toHaveCount(0)
  await expect(row.getByRole('switch')).toHaveAttribute('aria-checked', 'false')

  await row.getByRole('switch').click()
  const reopened = page.getByRole('dialog', { name: '权限请求' })
  await reopened.getByRole('button', { name: '继续', exact: true }).click()
  await reopened.getByRole('button', { name: '继续', exact: true }).click()
  await expect(reopened.getByRole('checkbox', { name: 'github.com', exact: true })).toBeChecked()
  await reopened.getByRole('button', { name: '继续', exact: true }).click()
  await expect(reopened.getByRole('checkbox', { name: 'git', exact: true })).toBeChecked()
  await reopened.getByRole('button', { name: '激活', exact: true }).click()
  await expect(reopened).toHaveCount(0)
  await expect(row.getByRole('switch')).toHaveAttribute('aria-checked', 'true')
  await page.screenshot({ path: 'artifacts/plugin-settings.png' })
})

test('activation can deselect a permission and target before activating', async ({ page }) => {
  await page.getByRole('button', { name: '插件', exact: true }).click()
  await page.getByRole('menuitem', { name: '插件设置', exact: true }).click()
  const row = page.locator('.plugin-row').filter({ hasText: 'Git' })
  await row.getByRole('switch', { name: '停用 Git' }).click()
  await row.getByRole('switch', { name: '激活 Git' }).click()
  const sheet = page.getByRole('dialog', { name: '权限请求' })
  const read = sheet.getByRole('checkbox', { name: '读取文件', exact: true })
  await expect(read).toBeChecked()
  await read.uncheck()
  await sheet.getByRole('button', { name: '继续', exact: true }).click()
  await sheet.getByRole('button', { name: '继续', exact: true }).click()
  await sheet.getByRole('checkbox', { name: 'github.com', exact: true }).uncheck()
  await sheet.getByRole('button', { name: '继续', exact: true }).click()
  await sheet.getByRole('button', { name: '激活', exact: true }).click()
  await expect(sheet).toHaveCount(0)
  await expect(row.getByRole('switch')).toHaveAttribute('aria-checked', 'true')

  // Only the selected subset is stored: the plugin stays active with seven of
  // its eight declared capabilities and no allowed host.
  await row.getByRole('button', { name: '插件详情', exact: true }).click()
  const details = page.getByRole('dialog', { name: '插件详情' })
  await expect(details).toContainText('7 / 8 已授权')
  await expect(details.getByRole('switch', { name: '授权 读取文件', exact: true })).toHaveAttribute(
    'aria-checked',
    'false',
  )
  await details.getByRole('button', { name: '配置网络域名' }).click()
  const domains = page.getByRole('dialog', { name: '网络域名' })
  await expect(domains.getByRole('switch', { name: '授权域名 github.com' })).toHaveAttribute(
    'aria-checked',
    'false',
  )
  await domains.getByRole('button', { name: '关闭', exact: true }).click()
  await details.getByRole('button', { name: '关闭', exact: true }).click()
})

test('using a plugin asks again after its read capability was revoked', async ({ page }) => {
  await page.goto('/?fixture&pluginFixture')
  await page.getByRole('button', { name: '插件', exact: true }).click()
  await page.getByRole('menuitem', { name: '插件设置', exact: true }).click()
  const manager = page.getByRole('dialog', { name: '插件', exact: true })
  const checksum = manager.locator('.plugin-row').filter({ hasText: '校验和' })
  await checksum.hover()
  await checksum.getByRole('button', { name: '插件详情', exact: true }).click()
  const details = page.getByRole('dialog', { name: '插件详情' })
  const read = details.getByRole('switch', { name: '授权 读取文件', exact: true })
  await read.click()
  await expect(read).toHaveAttribute('aria-checked', 'false')
  await details.getByRole('button', { name: '关闭', exact: true }).click()
  await manager.getByRole('button', { name: '关闭', exact: true }).click()
  await expect(manager).toBeHidden()
  await openDocuments(page)

  // Normal use: the checksum action reads the selected file, so Core asks
  // again even though the plugin itself is still running.
  await page.getByText('Notes.md', { exact: true }).first().click({ button: 'right' })
  await page.getByRole('menuitem', { name: '插件', exact: true }).hover()
  await page.getByRole('menuitem', { name: '查看校验和', exact: true }).click()
  const prompt = page.getByRole('dialog', { name: '权限请求' })
  await expect(prompt).toContainText('允许「校验和」读取文件？')
  await prompt.getByRole('button', { name: '允许', exact: true }).click()
  await expect(prompt).toHaveCount(0)
})

test('a copied package stays off without prompting and manual activation succeeds', async ({
  page,
}) => {
  await page.goto('/?fixture')
  await page.evaluate(async () => {
    const path = '/e2e/fixtures/plugins/index.ts'
    const { fixturePlugins } = await import(/* @vite-ignore */ path)
    fixturePlugins.push({
      id: 'org.test.copied',
      name: 'Copied Package',
      version: '1.0.0',
      enabled: false,
      approved: false,
      backendAvailable: true,
      declaredPermissions: [],
      grantedPermissions: [],
      requestedNetworkDomains: [],
      allowedNetworkDomains: [],
    })
    window.dispatchEvent(new Event('fiex:plugins-changed'))
  })
  await page.getByRole('button', { name: '插件', exact: true }).click()
  await page.getByRole('menuitem', { name: '插件设置', exact: true }).click()
  const row = page.locator('.plugin-row').filter({ hasText: 'Copied Package' })
  await expect(row.getByRole('switch')).toHaveAttribute('aria-checked', 'false')
  await expect(page.getByRole('dialog', { name: '权限请求' })).toHaveCount(0)
  await row.getByRole('switch').click()
  await page
    .getByRole('dialog', { name: '权限请求' })
    .getByRole('button', { name: '激活', exact: true })
    .click()
  await expect(row.getByRole('switch')).toHaveAttribute('aria-checked', 'true')
})

test('confirming a local import immediately opens activation consent', async ({ page }) => {
  await page.getByRole('button', { name: '插件', exact: true }).click()
  await page.getByRole('menuitem', { name: '插件设置', exact: true }).click()
  await page.getByRole('button', { name: '导入插件', exact: true }).click()
  await chooseFixturePluginPackage(page)
  await page
    .getByRole('dialog', { name: '导入插件' })
    .getByRole('button', { name: '导入插件', exact: true })
    .click()
  const consent = page.getByRole('dialog', { name: '权限请求' })
  await expect(consent).toBeVisible()
  await expect(consent).toContainText('本地示例插件')
  await consent.getByRole('button', { name: '取消', exact: true }).click()
  await expect(
    page.locator('.plugin-row').filter({ hasText: '本地示例插件' }).getByRole('switch'),
  ).toHaveAttribute('aria-checked', 'false')
})
