import { test, expect } from '@playwright/test'
test.beforeEach(async ({ page }) => {
  await page.goto('/?demo&pluginFixture')
})
test('four views, grouping, and keyboard preview', async ({ page }) => {
  await page.getByRole('grid', { name: '文件' }).click({ position: { x: 400, y: 400 } })
  await page.getByText('Notes.md', { exact: true }).first().click()
  await page.keyboard.press('Space')
  await expect(page.getByRole('dialog')).toBeVisible()
  await expect(page.getByRole('dialog').locator('pre')).toContainText('Fiex')
  await page.keyboard.press('Space')
  await expect(page.getByRole('dialog')).toHaveCount(0)
  for (const view of ['图标', '画廊', '分栏', '列表']) {
    await page.getByRole('button', { name: view + '视图', exact: true }).click()
    await expect(page.getByText('Notes.md', { exact: true }).first()).toBeVisible()
  }
  await page.getByRole('button', { name: '分组方式', exact: true }).click()
  await page.getByRole('menuitem', { name: '种类', exact: true }).first().click()
  await expect(page.locator('.group-heading').filter({ hasText: '文件夹' })).toBeVisible()
  await page.screenshot({ path: 'artifacts/list.png' })
})
test('code preview highlights JSON', async ({ page }) => {
  await page.getByText('Palette.json', { exact: true }).first().click()
  await page.keyboard.press('Space')
  const dialog = page.getByRole('dialog')
  await expect(dialog).toBeVisible()
  await expect(dialog.locator('pre.preview-code .hljs-attr').first()).toBeVisible()
  await expect(dialog.locator('pre.preview-code .hljs-string').first()).toBeVisible()
  await page.keyboard.press('Space')
})
test('folder navigation, tabs and fast search', async ({ page }) => {
  await page.getByText('Research', { exact: true }).dblclick()
  await expect(page.getByText('InSb.pdf', { exact: true })).toBeVisible()
  await expect(page.getByRole('textbox', { name: '搜索文件' })).toBeVisible()
  await page.getByRole('button', { name: '后退', exact: true }).click()
  await expect(page.getByText('Notes.md', { exact: true }).first()).toBeVisible()
  await page.getByRole('button', { name: '新建标签页', exact: true }).click()
  await expect(page.getByRole('tab')).toHaveCount(2)
  await page.getByRole('button', { name: '关闭标签页', exact: true }).last().click()
  await expect(page.getByRole('tab')).toHaveCount(1)
  await page.getByRole('textbox', { name: '搜索文件' }).fill('insb')
  await expect(page.getByText('InSb.pdf', { exact: true })).toBeVisible()
  await expect(page.getByText('Notes.md', { exact: true }).first()).toHaveCount(0)
  await page.getByRole('button', { name: '清除搜索' }).click()
  await expect(page.getByText('Notes.md', { exact: true }).first()).toBeVisible()
})
test('closing the last tab reopens the configured start page', async ({ page }) => {
  await page.getByText('Research', { exact: true }).dblclick()
  await expect(page.getByText('InSb.pdf', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: '关闭标签页', exact: true }).click()
  await expect(page.getByRole('tab')).toHaveCount(1)
  await expect(page.getByRole('tab')).toContainText('Documents')
  await expect(page.getByText('Notes.md', { exact: true }).first()).toBeVisible()
})
test('closing the last tab collapses the split pane', async ({ page }) => {
  await page.getByRole('button', { name: '双面板' }).click()
  await expect(page.getByRole('complementary', { name: '第二个面板' })).toBeVisible()
  await page.getByRole('button', { name: '关闭标签页', exact: true }).click()
  await expect(page.getByRole('tab')).toHaveCount(1)
  await expect(page.getByRole('tab')).toContainText('Documents')
  await expect(page.getByRole('complementary', { name: '第二个面板' })).toHaveCount(0)
})
test('plugin search providers add results to the same query', async ({ page }) => {
  await expect(page.getByText('Notes.md', { exact: true }).first()).toBeVisible()
  await page.getByRole('textbox', { name: '搜索文件' }).fill('plugin-index-hit')
  await expect(page.getByText('Indexed result.txt', { exact: true })).toBeVisible({
    timeout: 15000,
  })
  await expect(page.locator('.statusbar')).toBeVisible()
})
test('each tab keeps its own one or two panes', async ({ page }) => {
  await page.getByRole('button', { name: '双面板' }).click()
  await expect(page.getByRole('complementary', { name: '第二个面板' })).toBeVisible()
  await page
    .getByRole('complementary', { name: '第二个面板' })
    .getByText('Research', { exact: true })
    .dblclick()
  await expect(page.getByRole('navigation', { name: '第二个面板的路径' })).toContainText('Research')
  await page.getByRole('button', { name: '新建标签页' }).click()
  await expect(page.getByRole('complementary', { name: '第二个面板' })).toHaveCount(0)
  await page.getByRole('tab').first().click()
  await expect(page.getByRole('complementary', { name: '第二个面板' })).toBeVisible()
  await expect(page.getByRole('navigation', { name: '第二个面板的路径' })).toContainText('Research')
  await page.getByRole('button', { name: '后退', exact: true }).click()
  await expect(page.getByRole('navigation', { name: '第二个面板的路径' })).not.toContainText(
    'Research',
  )
  await page.getByRole('button', { name: '前进', exact: true }).click()
  await expect(page.getByRole('navigation', { name: '第二个面板的路径' })).toContainText('Research')
  await page.getByRole('button', { name: '关闭左侧面板' }).click()
  await expect(page.getByRole('complementary', { name: '第二个面板' })).toHaveCount(0)
  await expect(page.getByRole('navigation', { name: '当前位置' })).toContainText('Research')
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
test('workspace opens as a compact dock and can be collapsed', async ({ page }) => {
  await page.getByRole('button', { name: '工作区' }).click()
  const dock = page.getByRole('region', { name: '工作区' })
  await expect(dock.getByText('工作区为空')).toBeVisible()
  await page.getByRole('button', { name: '工作区' }).click()
  await expect(dock).toHaveCount(0)
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
test('address bar selects the whole path for copying', async ({ page }) => {
  await page.context().grantPermissions(['clipboard-read', 'clipboard-write'])
  await page.getByText('Research', { exact: true }).dblclick()
  await expect(page.getByRole('textbox', { name: '文件夹路径' })).toHaveCount(0)
  await page.locator('.breadcrumbs > span:last-child button').click()
  const input = page.getByRole('textbox', { name: '文件夹路径' })
  await expect(input).toHaveValue('/Users/kevin/Documents/Research')
  await expect(input).toBeFocused()
  expect(
    await input.evaluate((element: HTMLInputElement) => [
      element.selectionStart,
      element.selectionEnd,
    ]),
  ).toEqual([0, '/Users/kevin/Documents/Research'.length])
  await page.keyboard.press('Control+c')
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(
    '/Users/kevin/Documents/Research',
  )
  await page.keyboard.press('Escape')
  await expect(input).toHaveCount(0)
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
test('Applications entry and search control', async ({ page }) => {
  await page.getByRole('button', { name: '应用程序', exact: true }).click()
  await expect(
    page.locator('[data-path="applications://ZmlleHQ="]').getByText('Fiex', { exact: true }),
  ).toBeVisible()
  await expect(page.getByRole('textbox', { name: '搜索文件' })).toBeVisible()
})
test('built-in terminal opens an integrated bottom panel', async ({ page }) => {
  await page.goto('/?demo')
  await page.evaluate(() => {
    const key = 'fiex:preferences'
    const prefs = JSON.parse(localStorage.getItem(key) || '{}')
    localStorage.setItem(key, JSON.stringify({ ...prefs, terminal: 'builtin' }))
  })
  await page.reload()
  const contentHeight = await page.locator('.pane-main').evaluate((element) => element.clientHeight)
  await page.getByRole('button', { name: 'Open in Terminal' }).click()
  await expect(page.locator('.terminal-panel')).toBeVisible()
  expect(await page.locator('.pane-main').evaluate((element) => element.clientHeight)).toBe(
    contentHeight,
  )
  await expect(page.locator('.terminal-panel-tab')).toHaveCount(1)
  await expect(page.locator('.terminal-panel')).toContainText('Fiex 终端')
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
  await settings.getByLabel('字体颜色').fill('#55aaff')
  await expect
    .poll(() =>
      page.evaluate(
        () => JSON.parse(localStorage.getItem('fiex:preferences') || '{}').terminalFontSize,
      ),
    )
    .toBe(17)
  await expect
    .poll(() =>
      page.evaluate(
        () => JSON.parse(localStorage.getItem('fiex:preferences') || '{}').terminalColor,
      ),
    )
    .toBe('#55aaff')
  await settings.getByRole('button', { name: '跟随主题' }).click()
  await page.getByRole('button', { name: '终端设置' }).click()
  await page.getByRole('button', { name: '新建终端' }).click()
  await page.getByRole('menuitem', { name: 'bash' }).click()
  await expect(page.locator('.terminal-panel-tab')).toHaveCount(2)
  await page.getByRole('button', { name: '收起终端' }).click()
  await expect(page.locator('.terminal-panel')).toBeHidden()
  await page.getByRole('button', { name: 'Open in Terminal' }).click()
  await expect(page.locator('.terminal-panel')).toBeVisible()
  await page.getByRole('button', { name: '打开终端' }).click()
  await expect(page.locator('.terminal-panel')).toBeHidden()
  await page.getByRole('button', { name: '打开终端' }).click()
  await expect(page.locator('.terminal-panel')).toBeVisible()
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
  const context = await page.locator('.connection-session-menu.floating').boundingBox()
  expect(context && context.y + context.height).toBeLessThanOrEqual(780)
  await page.getByRole('menuitem', { name: '设置为自启动' }).click()
  await expect(page.locator('.connection-autostart')).toHaveAttribute('title', '开机自启动')
  await page.getByRole('button', { name: '添加连接' }).click()
  await expect(page.getByRole('menuitem', { name: '生产服务器' })).toBeVisible()
  const plus = await page.locator('.connection-session-menu.plus').boundingBox()
  expect(plus && plus.y + plus.height).toBeLessThanOrEqual(780)
})

test('opens the Trash inside Fiex', async ({ page }) => {
  await page.getByRole('button', { name: '废纸篓', exact: true }).click()
  await expect(page.getByText('Old draft.txt', { exact: true })).toBeVisible()
  await expect(page.getByRole('heading', { name: '废纸篓' })).toBeVisible()
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

test('appearance can set and remove a background image', async ({ page }) => {
  await page.getByRole('button', { name: '设置', exact: true }).last().click()
  await page.getByRole('button', { name: '选择图片…' }).click()
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

function pdfFixture() {
  const stream = 'BT /F1 24 Tf 50 100 Td (Hello Fiex) Tj ET'
  const objects = [
    '<< /Type /Catalog /Pages 2 0 R >>',
    '<< /Type /Pages /Kids [3 0 R] /Count 1 >>',
    '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>',
    '<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>',
    `<< /Length ${stream.length} >>\nstream\n${stream}\nendstream`,
  ]
  let text = '%PDF-1.4\n'
  const offsets = [0]
  objects.forEach((obj, i) => {
    offsets.push(text.length)
    text += `${i + 1} 0 obj\n${obj}\nendobj\n`
  })
  const start = text.length
  text += `xref\n0 6\n0000000000 65535 f \n${offsets
    .slice(1)
    .map((n) => String(n).padStart(10, '0') + ' 00000 n ')
    .join('\n')}\ntrailer << /Size 6 /Root 1 0 R >>\nstartxref\n${start}\n%%EOF`
  return Buffer.from(text)
}
test('PDF preview renders a real document', async ({ page }) => {
  await page.route('**/preview-fixture.pdf', (route) =>
    route.fulfill({ contentType: 'application/pdf', body: pdfFixture() }),
  )
  await page.evaluate(async () => {
    const componentPath = '/src/components/PdfPreview.tsx'
    const reactPath = '/node_modules/.vite/deps/react.js'
    const domPath = '/node_modules/.vite/deps/react-dom_client.js'
    const [{ default: Component }, react, dom] = await Promise.all([
      import(componentPath),
      import(reactPath),
      import(domPath),
    ])
    const host = document.createElement('div')
    host.id = 'pdf-test'
    host.style.cssText = 'position:fixed;inset:40px;background:white;z-index:200'
    document.body.append(host)
    const mount = dom.createRoot || dom.default.createRoot
    const element = react.createElement || react.default.createElement
    mount(host).render(element(Component, { url: '/preview-fixture.pdf' }))
  })
  await expect(page.locator('#pdf-test .pdf-controls')).toContainText('1 / 1')
  await expect
    .poll(() =>
      page.locator('#pdf-test canvas').evaluate((canvas: HTMLCanvasElement) => {
        const data = canvas.getContext('2d')!.getImageData(0, 0, canvas.width, canvas.height).data
        let dark = 0
        for (let i = 0; i < data.length; i += 4) if (data[i] < 100 && data[i + 3] > 0) dark++
        return dark
      }),
    )
    .toBeGreaterThan(50)
})

test('core connection entrypoint and no preinstalled optional plugin', async ({ page }) => {
  await page.goto('/?demo')
  await page.getByRole('button', { name: '添加连接' }).click()
  await page.getByRole('menuitem', { name: '新建连接…' }).click()
  const dialog = page.getByRole('dialog')
  await expect(dialog).toBeVisible()
  await page.getByRole('button', { name: '关闭', exact: true }).click()
  await page.getByText('Notes.md', { exact: true }).first().click({ button: 'right' })
  await expect(page.getByRole('menuitem', { name: '压缩为…' })).toHaveCount(0)
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

test('ten thousand icons stay virtualized and support end selection and rename', async ({
  page,
}) => {
  await page.goto('/?demo&count=10000')
  await expect(page.locator('.statusbar')).toContainText('10,015')
  await page.getByRole('button', { name: '图标视图', exact: true }).click()
  await expect(page.locator('.virtual-file-grid')).toBeVisible()
  expect(await page.getByRole('gridcell').count()).toBeLessThan(180)
  await page.getByRole('grid', { name: '文件', exact: true }).focus()
  await page.keyboard.press('End')
  await expect(page.locator('[data-path="/Users/kevin/Documents/zz-file-09999.txt"]')).toBeVisible()
  await page.keyboard.press('F2')
  await expect(page.getByRole('textbox', { name: '重命名 zz-file-09999.txt' })).toBeVisible()
  await page.keyboard.press('Escape')
  await page.getByRole('button', { name: '列表视图', exact: true }).click()
  expect(await page.locator('.file-row').count()).toBeLessThan(80)
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

test('go to folder shortcut works while search is focused', async ({ page }) => {
  await page.getByRole('textbox', { name: '搜索文件' }).focus()
  await page.keyboard.press('Control+l')
  await expect(page.getByRole('textbox', { name: '文件夹路径' })).toBeFocused()
  await page.keyboard.press('Escape')
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

test('optional archive and checksum features are not preinstalled', async ({ page }) => {
  await page.goto('/?demo')
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
  await page.goto('/?demo&pluginFixture')
  await page.getByRole('button', { name: '插件', exact: true }).click()
  await page.getByRole('menuitem', { name: '插件设置', exact: true }).click()
  await page.getByRole('button', { name: '导入插件', exact: true }).click()
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
  await page.goto('/?demo&pluginFixture')
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

test('installed plugin rows expose permissions and settings', async ({ page }) => {
  await page.goto('/?demo&pluginFixture')
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

test('tab dragging preserves the active folder', async ({ page }) => {
  await page.getByRole('button', { name: '新建标签页', exact: true }).click()
  await page.getByText('Research', { exact: true }).dblclick()
  const tabs = page.getByRole('tab')
  const from = await tabs.nth(1).boundingBox()
  const to = await tabs.nth(0).boundingBox()
  if (!from || !to) throw new Error('missing tab geometry')
  await page.mouse.move(from.x + from.width / 2, from.y + from.height / 2)
  await page.mouse.down()
  await page.mouse.move(to.x + to.width / 2, to.y + to.height / 2, { steps: 8 })
  await page.mouse.up()
  await expect(tabs.nth(0)).toContainText('Research')
  await expect(tabs.nth(0)).toHaveAttribute('aria-selected', 'true')
  await tabs.nth(1).click()
  await expect(page.getByText('Notes.md', { exact: true })).toBeVisible()
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
  await page.goto('/?demo&platform=windows')
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

test('installed plugin contributes submenu and isolated iframe modal with context bridge', async ({
  page,
}) => {
  await page.goto('/?demo&pluginFixture')
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
  await page.goto('/?demo&pluginFixture')
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

test('a copied package stays off without prompting and manual activation succeeds', async ({
  page,
}) => {
  await page.goto('/?demo')
  await page.evaluate(async () => {
    const path = '/src/lib/demo/plugins/index.ts'
    const { demoPlugins } = await import(/* @vite-ignore */ path)
    demoPlugins.push({
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
