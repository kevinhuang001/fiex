import { test, expect } from '@playwright/test'
import { openDocuments } from '../fixtures/helpers'

test.beforeEach(async ({ page }) => {
  await page.goto('/?fixture&pluginFixture')
  await openDocuments(page)
})

test('shortcut preset and custom binding', async ({ page }) => {
  await page.getByRole('button', { name: '设置', exact: true }).last().click()
  await page.getByRole('button', { name: '快捷键', exact: true }).click()
  await page.getByRole('button', { name: 'macOS', exact: true }).click()
  const copy = page.locator('.shortcut-row').filter({ hasText: '复制' }).locator('.shortcut-keys')
  await expect(copy).toContainText('⌘C')
  await copy.locator('.shortcut-pill').first().locator('button').first().click()
  await page.keyboard.press('F9')
  await expect(copy).toContainText('F9')
  await page.locator('.shortcut-footer button').click()
  await expect(copy).toContainText('⌘C')
  await page.getByRole('button', { name: '关闭', exact: true }).click()
})

test('settings creates tags and the file dialog only applies them', async ({ page }) => {
  await page.getByRole('button', { name: '设置', exact: true }).click()
  await page.locator('.settings-sidebar').getByText('标签', { exact: true }).click()
  await page.getByRole('button', { name: '添加标签', exact: true }).click()
  const create = page.getByRole('dialog', { name: '添加标签' })
  await create.getByRole('textbox', { name: '标签名称', exact: true }).fill('验收')
  await create.getByRole('button', { name: '添加', exact: true }).click()
  await page.getByRole('button', { name: '确认', exact: true }).click()
  await expect(
    page.locator('.sidebar').getByRole('button', { name: '验收', exact: true }),
  ).toBeVisible()
  await page.getByText('Notes.md', { exact: true }).first().click({ button: 'right' })
  await page.getByRole('menuitem', { name: '标签…', exact: true }).click()
  await expect(page.getByRole('dialog')).toContainText('标签')
  await expect(page.getByRole('textbox', { name: '自定义标签名称' })).toHaveCount(0)
  await page.getByRole('dialog').getByRole('button', { name: '验收', exact: true }).click()
  await page.getByRole('button', { name: '应用标签', exact: true }).click()
  await expect(page.getByRole('dialog')).toHaveCount(0)
  await expect(
    page.locator('[data-path="/Users/kevin/Documents/Notes.md"] .tag-dots'),
  ).toBeVisible()
  await page.locator('[data-path="/Users/kevin/Documents/Notes.md"]').click({ button: 'right' })
  await page.getByRole('menuitem', { name: '显示简介' }).click()
  await expect(page.getByRole('dialog')).toContainText('验收')
})

test('appearance can set and remove a background image', async ({ page }) => {
  await page.getByRole('button', { name: '设置', exact: true }).last().click()
  await page.getByRole('button', { name: '选择图片…' }).click()
  const picker = page.getByRole('dialog', { name: '打开文件' })
  await picker.getByRole('option', { name: /^Documents/ }).dblclick()
  await picker.getByRole('option', { name: /^Projects/ }).dblclick()
  await picker.getByRole('option', { name: /^logo\.png/ }).click()
  await picker.getByRole('button', { name: '打开', exact: true }).click()
  await expect(page.locator('.app-background')).toHaveCount(0)
  await page.getByRole('button', { name: '确认', exact: true }).click()
  await expect(page.locator('.app.has-background')).toHaveCount(1)
  await expect(page.getByRole('slider', { name: '背景不透明度' })).toHaveCount(0)
  await page.getByRole('button', { name: '设置', exact: true }).last().click()
  await page.getByRole('button', { name: '移除', exact: true }).click()
  await page.getByRole('button', { name: '确认', exact: true }).click()
  await expect(page.locator('.app.has-background')).toHaveCount(0)
})

test('settings are custom controls and theme persists', async ({ page }) => {
  await page.getByRole('button', { name: '设置', exact: true }).last().click()
  await page.getByRole('button', { name: '深色', exact: true }).click()
  await expect(page.locator('html')).not.toHaveClass('dark')
  await page.getByRole('button', { name: '通用', exact: true }).click()
  await page.getByRole('switch', { name: '显示隐藏文件' }).click()
  await expect(page.getByRole('switch', { name: '显示隐藏文件' })).toHaveAttribute(
    'aria-checked',
    'true',
  )
  await page.getByRole('button', { name: '确认', exact: true }).click()
  await expect(page.locator('html')).toHaveClass('dark')
  await page.screenshot({ path: 'artifacts/dark.png' })
  await page.reload()
  await expect(page.locator('html')).toHaveClass('dark')
})

test('settings sections keep preferences and release shortcut recording on exit', async ({
  page,
}) => {
  await page.getByRole('button', { name: '设置', exact: true }).click()
  await page.getByRole('button', { name: '快捷键', exact: true }).click()
  await page.getByRole('button', { name: '设置 复制 快捷键', exact: true }).first().click()
  await page.getByRole('button', { name: '通用', exact: true }).click()
  await page.getByRole('button', { name: '深色', exact: true }).click()
  await page.getByRole('button', { name: '侧边栏', exact: true }).click()
  const desktop = page.getByRole('checkbox', { name: '显示桌面', exact: true })
  await desktop.uncheck()
  await page.getByRole('button', { name: '忽略', exact: true }).click()
  await expect(page.locator('.settings-card-header').filter({ hasText: '显示忽略' })).toBeVisible()
  await page.getByRole('button', { name: '启动方式', exact: true }).click()
  await expect(page.getByText('默认终端', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: '关于', exact: true }).click()
  await expect(page.getByText('开源许可', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: '快捷键', exact: true }).click()
  await expect(page.getByText('按下快捷键…', { exact: true })).toHaveCount(0)
  await page.getByRole('button', { name: '确认', exact: true }).click()
  await page.reload()
  await expect(page.locator('html')).toHaveClass('dark')
  await expect(
    page.locator('.sidebar').getByRole('button', { name: '桌面', exact: true }),
  ).toHaveCount(0)
})

test('go to folder shortcut works while search is focused', async ({ page }) => {
  await page.getByRole('textbox', { name: '搜索文件' }).focus()
  await page.keyboard.press('Control+l')
  await expect(page.getByRole('textbox', { name: '文件夹路径' })).toBeFocused()
  await page.keyboard.press('Escape')
})

test('installed plugin rows expose permissions and settings', async ({ page }) => {
  await page.goto('/?fixture&pluginFixture')
  await page.getByRole('button', { name: '插件', exact: true }).click()
  await page.getByRole('menuitem', { name: '插件设置', exact: true }).click()
  const git = page.locator('.plugin-row').filter({ hasText: 'Git' })
  await expect(git).toBeVisible()
  await git.hover()
  await expect(git.getByRole('button', { name: '卸载 Git', exact: true })).toBeVisible()
  await git.getByRole('button', { name: '插件详情', exact: true }).click()
  const permissions = page.getByRole('dialog', { name: '插件详情' })
  await expect(permissions).toContainText('读取文件')
  await expect(permissions).toContainText('修改文件')
  await expect(permissions).toContainText('网络访问')
  await expect(permissions).toContainText('命令')
  await permissions.getByRole('button', { name: '配置允许的命令' }).click()
  const commands = page.getByRole('dialog', { name: '外部命令' })
  await expect(commands).toContainText('git')
  await commands.getByRole('button', { name: '关闭', exact: true }).click()
  await permissions.getByRole('button', { name: '配置网络域名' }).click()
  const domains = page.getByRole('dialog', { name: '网络域名' })
  await expect(domains).toContainText('github.com')
  await domains.getByRole('textbox', { name: '添加允许的域名' }).fill('gitea.example.com')
  await domains.getByRole('button', { name: '添加域名', exact: true }).click()
  const domainPrompt = page.getByRole('dialog', { name: '权限请求' })
  await expect(domainPrompt).toContainText('允许「Git」访问 gitea.example.com？')
  await domainPrompt.getByRole('button', { name: '允许', exact: true }).click()
  await expect(domains).toContainText('gitea.example.com')
  await domains.getByRole('button', { name: '关闭', exact: true }).click()
  await permissions.getByRole('button', { name: '关闭', exact: true }).click()
  await git.hover()
  await git.getByRole('button', { name: '设置 Git' }).click()
  await expect(page.getByRole('dialog', { name: /Git · 设置/ })).toContainText('历史提交显示条数')
})

test('settings creates, edits and deletes tags through the confirmation', async ({ page }) => {
  await page.getByRole('button', { name: '设置', exact: true }).click()
  await page.locator('.settings-sidebar').getByText('标签', { exact: true }).click()

  await page.getByRole('button', { name: '添加标签', exact: true }).click()
  const create = page.getByRole('dialog', { name: '添加标签' })
  await create.getByRole('textbox', { name: '标签名称' }).fill('审阅')
  await create.getByRole('button', { name: '选择颜色 #34c759' }).click()
  await create.getByRole('button', { name: '添加', exact: true }).click()
  await expect(page.getByRole('button', { name: '编辑标签 审阅' })).toBeVisible()

  await page.getByRole('button', { name: '编辑标签 审阅' }).click()
  const edit = page.getByRole('dialog', { name: '编辑标签' })
  await edit.getByRole('textbox', { name: '标签名称' }).fill('审阅中')
  await edit.getByRole('button', { name: '保存', exact: true }).click()
  await expect(page.getByRole('button', { name: '编辑标签 审阅中' })).toBeVisible()

  await page.getByRole('button', { name: '删除标签 审阅中' }).click()
  const confirm = page.getByRole('dialog', { name: '删除标签' })
  await expect(confirm).toContainText('确定删除标签“审阅中”吗？')
  await confirm.getByRole('button', { name: '取消', exact: true }).click()
  await expect(page.getByRole('button', { name: '删除标签 审阅中' })).toBeVisible()

  await page.getByRole('button', { name: '删除标签 审阅中' }).click()
  await page
    .getByRole('dialog', { name: '删除标签' })
    .getByRole('button', { name: '删除', exact: true })
    .click()
  await expect(page.getByRole('button', { name: '删除标签 审阅中' })).toHaveCount(0)
  await page.getByRole('button', { name: '确认', exact: true }).click()
  await expect(page.locator('.sidebar').getByRole('button', { name: '审阅中' })).toHaveCount(0)
})

test('caption buttons stay clear of tabs and the sidebar starts below the tab strip', async ({
  page,
}) => {
  await page.goto('/?fixture&platform=windows')
  await openDocuments(page)
  await expect(page.getByText('Fiex — 产品设计.pdf', { exact: true })).toBeVisible()
  for (let i = 0; i < 12; i += 1)
    await page.getByRole('button', { name: '新建标签页', exact: true }).click()
  const metrics = await page.evaluate(() => {
    const box = (selector: string) => document.querySelector(selector)?.getBoundingClientRect()
    const controls = box('.window-controls')
    const strip = box('.tabs-scroll')
    const tabs = box('.tabs')
    const label = box('.sidebar-content .sidebar-label')
    const plus = document.querySelector('.tabs-new')
    return {
      controlsLeft: controls?.left ?? 0,
      stripRight: strip?.right ?? 0,
      tabsBottom: tabs?.bottom ?? 0,
      labelTop: label?.top ?? 0,
      plusBackground: plus ? getComputedStyle(plus).backgroundColor : '',
    }
  })
  expect(metrics.controlsLeft).toBeGreaterThanOrEqual(metrics.stripRight)
  expect(metrics.labelTop).toBeGreaterThan(metrics.tabsBottom)
  expect(metrics.plusBackground).toBe('rgba(0, 0, 0, 0)')
})

test('settings and tag dialogs keep their compact widths', async ({ page }) => {
  await page.getByRole('button', { name: '设置', exact: true }).click()
  await page.locator('.settings-sidebar').getByText('标签', { exact: true }).click()
  const settingsWidth = await page
    .locator('.settings-modal')
    .evaluate((element) => element.getBoundingClientRect().width)
  expect(settingsWidth).toBeLessThanOrEqual(760)
  await page.getByRole('button', { name: '添加标签', exact: true }).click()
  const dialogWidth = await page
    .getByRole('dialog', { name: '添加标签' })
    .evaluate((element) => element.getBoundingClientRect().width)
  expect(dialogWidth).toBeLessThanOrEqual(420)
})
