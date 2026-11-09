import { test, expect } from '@playwright/test'
import { openDocuments, chooseFixturePluginPackage } from '../fixtures/helpers'

test.beforeEach(async ({ page }) => {
  await page.goto('/?fixture&pluginFixture')
  await openDocuments(page)
})

test('plugin search providers add results to the same query', async ({ page }) => {
  await expect(page.getByText('Notes.md', { exact: true }).first()).toBeVisible()
  await page.getByRole('textbox', { name: '搜索文件' }).fill('plugin-index-hit')
  await expect(page.getByText('Indexed result.txt', { exact: true })).toBeVisible({
    timeout: 15000,
  })
  await expect(page.locator('.statusbar')).toBeVisible()
})

test('connection plugin opens a full-height view', async ({ page }) => {
  await page.getByText('生产服务器', { exact: true }).click()
  await expect(page.locator('.plugin-connection-frame')).toBeVisible()
  await expect(page.locator('.explorer-toolbar')).toHaveCount(0)
  await page.getByRole('button', { name: '新建标签页', exact: true }).click()
  await expect(page.getByRole('tab')).toHaveCount(2)
  await expect(page.locator('.plugin-connection-frame')).toHaveCount(1)
  await expect(page.locator('.plugin-connection-frame')).toBeHidden()
  await expect(page.getByText('最近使用', { exact: true }).first()).toBeVisible()
  await page.getByRole('tab').first().click()
  await expect(page.locator('.plugin-connection-frame')).toBeVisible()
})

test('connection sessions can autostart and appear in the add menu', async ({ page }) => {
  await page.locator('.device-row').filter({ hasText: '生产服务器' }).click({ button: 'right' })
  await expect(page.getByRole('menuitem', { name: '保存 Session' })).toBeVisible()
  await expect
    .poll(async () => {
      const context = await page.locator('.connection-session-menu.floating').boundingBox()
      return Boolean(context && context.y + context.height <= 780)
    })
    .toBe(true)
  await page.getByRole('menuitem', { name: '设置为自启动' }).click()
  await expect(page.locator('.connection-autostart')).toHaveAttribute('title', '开机自启动')
  await page.getByRole('button', { name: '添加连接' }).click()
  await expect(page.getByRole('menuitem', { name: '生产服务器' })).toBeVisible()
  await expect
    .poll(async () => {
      const plus = await page.locator('.connection-session-menu.plus').boundingBox()
      return Boolean(plus && plus.y + plus.height <= 780)
    })
    .toBe(true)
})

test('core connection entrypoint and no preinstalled optional plugin', async ({ page }) => {
  await page.goto('/?fixture')
  await openDocuments(page)
  await page.getByRole('button', { name: '添加连接' }).click()
  await page.getByRole('menuitem', { name: '新建连接…' }).click()
  const dialog = page.getByRole('dialog')
  await expect(dialog).toBeVisible()
  await page.getByRole('button', { name: '关闭', exact: true }).click()
  await page.getByText('Notes.md', { exact: true }).first().click({ button: 'right' })
  await expect(page.getByRole('menuitem', { name: '压缩为…' })).toHaveCount(0)
})

test('optional archive and checksum features are not preinstalled', async ({ page }) => {
  await page.goto('/?fixture')
  await openDocuments(page)
  await page.getByText('Notes.md', { exact: true }).first().click({ button: 'right' })
  await expect(page.getByRole('menuitem', { name: '压缩为…' })).toHaveCount(0)
  await expect(page.getByRole('menuitem', { name: '查看校验和' })).toHaveCount(0)
  await page.getByRole('button', { name: '插件', exact: true }).click()
  await page.getByRole('menuitem', { name: '插件设置', exact: true }).click()
  await expect(page.getByText('用插件扩展 Fiex', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: '获取插件', exact: true }).click()
  const store = page.getByRole('dialog', { name: '获取插件' })
  await expect(store.getByRole('article').filter({ hasText: 'Git' })).toBeVisible()
  await expect(store.getByRole('article').filter({ hasText: 'WebDAV' })).toBeVisible()
  await expect(store.getByRole('article').filter({ hasText: '校验和' })).toBeVisible()
  await store
    .getByRole('article')
    .filter({ hasText: '校验和' })
    .getByRole('button', { name: '获取' })
    .click()
  const review = page.getByRole('dialog', { name: '安装插件' })
  await expect(review).toContainText('读取文件')
  await review.getByRole('button', { name: '安装并授权激活' }).click()
  await expect(page.locator('.plugin-row').filter({ hasText: '校验和' })).toBeVisible()
  await expect(page.getByRole('dialog', { name: '权限请求' })).toBeVisible()
})

test('local import shows identity and signature only', async ({ page }) => {
  await page.goto('/?fixture&pluginFixture')
  await page.getByRole('button', { name: '插件', exact: true }).click()
  await page.getByRole('menuitem', { name: '插件设置', exact: true }).click()
  await page.getByRole('button', { name: '导入插件', exact: true }).click()
  await chooseFixturePluginPackage(page)
  const dialog = page.getByRole('dialog', { name: '导入插件' })
  await expect(dialog).toContainText('本地示例插件')
  await expect(dialog).toContainText('Example Author')
  await expect(dialog).toContainText('插件包已签名')
  await expect(dialog).toContainText('签名者：Example Publisher')
  await expect(dialog).toContainText('Core 内置 Ed25519 校验')
  await expect(dialog).not.toContainText('未信任的发布者')
  await expect(dialog).not.toContainText('1.2.3')
  await expect(dialog).not.toContainText('读取文件')
  await page.screenshot({ path: 'artifacts/plugin-local-import.png' })
})

test('plugin rows expose a delete action with confirmation', async ({ page }) => {
  await page.goto('/?fixture&pluginFixture')
  await page.getByRole('button', { name: '插件', exact: true }).click()
  await page.getByRole('menuitem', { name: '插件设置', exact: true }).click()
  const checksum = page.locator('.plugin-row').filter({ hasText: '校验和' })
  await checksum.hover()
  await checksum.getByRole('button', { name: '卸载 校验和', exact: true }).click()
  const dialog = page.getByRole('dialog', { name: '卸载插件' })
  await expect(dialog).toContainText('确定卸载“校验和”吗？')
  await dialog.getByRole('button', { name: '取消', exact: true }).click()
  await expect(dialog).toHaveCount(0)
  await expect(checksum).toBeVisible()
})

test('installed plugin contributes submenu and isolated iframe modal with context bridge', async ({
  page,
}) => {
  await page.goto('/?fixture&pluginFixture')
  await expect(page.getByText('Notes.md', { exact: true }).first()).toBeVisible()
  await page.getByText('Notes.md', { exact: true }).first().click({ button: 'right' })
  // Plugin actions are grouped under one top-level submenu, not flattened.
  await expect(page.getByRole('menuitem', { name: '测试插件', exact: true })).toHaveCount(0)
  await page.getByRole('menuitem', { name: '插件', exact: true }).hover()
  await page.getByRole('menuitem', { name: '测试插件', exact: true }).hover()
  await page.getByRole('menuitem', { name: '信息面板', exact: true }).click()
  await expect(page.getByRole('dialog', { name: '信息面板' })).toBeVisible()
  const frame = page.frameLocator('iframe[title="信息面板"]')
  await expect(frame.getByText('示例插件面板')).toBeVisible()
  await expect(frame.locator('#selected')).toContainText('Notes.md')
})

test('nested plugin menus meet their parent without gap or overlap', async ({ page }) => {
  await page.getByText('Notes.md', { exact: true }).first().click({ button: 'right' })
  await page.getByRole('menuitem', { name: '插件', exact: true }).click()
  await page.getByRole('menuitem', { name: '测试插件', exact: true }).hover()
  const root = await page.locator('.context-menu').boundingBox()
  const plugin = await page.locator('.plugin-context-menu').boundingBox()
  const nested = await page.locator('.plugin-context-menu .context-submenu').boundingBox()
  if (!root || !plugin || !nested) throw new Error('missing context menu geometry')
  const flush = (child: { x: number; width: number }, parent: { x: number; width: number }) =>
    Math.min(
      Math.abs(child.x - (parent.x + parent.width)),
      Math.abs(child.x + child.width - parent.x),
    ) <= 1
  expect(flush(plugin, root)).toBe(true)
  expect(flush(nested, plugin)).toBe(true)
})

test('Core Open With action opens the application picker directly', async ({ page }) => {
  // Open With must work for Recent entries too, where the original regression
  // surfaced.
  await page.getByRole('button', { name: '最近使用', exact: true }).click()
  await page.getByText('Notes.md', { exact: true }).first().click({ button: 'right' })
  const trigger = page.getByRole('menuitem', { name: '打开方式', exact: true })
  await expect(trigger).not.toHaveAttribute('aria-haspopup', 'menu')
  await expect(trigger.locator('svg')).toHaveCount(1)
  await trigger.click()
  await expect(page.getByRole('menu', { name: '文件操作' })).toHaveCount(0)
  const picker = page.getByRole('dialog', { name: '选择应用打开文件' })
  await expect(picker).toBeVisible()
  await picker.getByRole('textbox', { name: '搜索应用' }).fill('Plain')
  await picker.getByRole('option', { name: 'Plain Text Editor' }).click()
  await expect(picker).toHaveCount(0)
})

test('closing and reopening the same plugin context retains its iframe', async ({ page }) => {
  const open = async () => {
    await page.getByText('Notes.md', { exact: true }).first().click({ button: 'right' })
    await page.getByRole('menuitem', { name: '插件', exact: true }).hover()
    await page.getByRole('menuitem', { name: '测试插件', exact: true }).hover()
    await page.getByRole('menuitem', { name: '信息面板', exact: true }).click()
  }
  await open()
  const frame = page.locator('iframe[title="信息面板"]')
  await expect(page.frameLocator('iframe[title="信息面板"]').locator('#selected')).toContainText(
    'Notes.md',
  )
  await frame.evaluate((element) => element.setAttribute('data-session-test', 'retained'))
  await page.getByRole('button', { name: '关闭插件面板', exact: true }).click()
  await expect(page.getByRole('dialog', { name: '信息面板' })).toBeHidden()
  await open()
  await expect(frame).toHaveAttribute('data-session-test', 'retained')
  await expect(frame).toHaveCount(1)
})
