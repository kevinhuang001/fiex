import { test, expect } from '@playwright/test'
test.beforeEach(async ({ page }) => {
  await page.goto('/?demo')
  await expect(page.getByText('Fiex — 产品设计.pdf', { exact: true })).toBeVisible()
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
test('folder navigation, tabs and fast search', async ({ page }) => {
  await page.getByText('Research', { exact: true }).dblclick()
  await expect(page.getByText('InSb.pdf', { exact: true })).toBeVisible()
  await expect(page.getByRole('textbox', { name: '搜索文件' })).toBeFocused()
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
  await page.getByRole('button', { name: '移到废纸篓', exact: true }).click()
  await expect(page.getByText('Renamed folder', { exact: true })).toHaveCount(0)
  await page.getByRole('button', { name: '选择终端', exact: true }).click()
  await expect(page.getByRole('menuitem', { name: 'iTerm2' })).toBeVisible()
  await page.keyboard.press('Escape')
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
  await expect(page.getByRole('dialog')).toContainText('移动项目')
  await page.getByRole('button', { name: '取消', exact: true }).click()

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
test('Finder tags can be applied to a file', async ({ page }) => {
  await page.getByText('Notes.md', { exact: true }).first().click({ button: 'right' })
  await page.getByRole('menuitem', { name: '标签…', exact: true }).click()
  await expect(page.getByRole('dialog')).toContainText('标签')
  await page.getByRole('textbox', { name: '自定义标签名称' }).fill('验收')
  await page.getByRole('button', { name: '添加', exact: true }).click()
  await page.getByRole('button', { name: '应用标签', exact: true }).click()
  await expect(page.getByRole('dialog')).toHaveCount(0)
  await expect(
    page.locator('[data-path="/Users/kevin/Documents/Notes.md"] .tag-dots'),
  ).toBeVisible()
  await page.locator('[data-path="/Users/kevin/Documents/Notes.md"]').click({ button: 'right' })
  await page.getByRole('menuitem', { name: '显示简介' }).click()
  await expect(page.getByRole('dialog')).toContainText('验收')
})
test('Applications entry and search focus', async ({ page }) => {
  await page.getByRole('button', { name: '应用程序', exact: true }).click()
  await expect(page.getByText('Fiex.app', { exact: true })).toBeVisible()
  await expect(page.getByRole('textbox', { name: '搜索文件' })).toBeFocused()
})
test('opens the Trash inside Fiex', async ({ page }) => {
  await page.getByRole('button', { name: '废纸篓', exact: true }).click()
  await expect(page.getByText('Old draft.txt', { exact: true })).toBeVisible()
  await expect(page.getByRole('heading', { name: '废纸篓' })).toBeVisible()
})
test('settings are custom controls and theme persists', async ({ page }) => {
  await page.getByRole('button', { name: '设置', exact: true }).last().click()
  await page.getByRole('button', { name: '深色', exact: true }).click()
  await expect(page.locator('html')).toHaveClass('dark')
  await page.getByRole('switch').click()
  await expect(page.getByRole('switch')).toHaveAttribute('aria-checked', 'true')
  await page.screenshot({ path: 'artifacts/dark.png' })
  await page.getByRole('button', { name: '关闭', exact: true }).click()
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

test('git status, tags and connection dialog', async ({ page }) => {
  await expect(page.locator('.git-file-m')).toBeVisible()
  await expect(page.getByText('main', { exact: false })).toBeVisible()
  await page.getByRole('button', { name: '源代码管理' }).click()
  await expect(page.getByRole('complementary', { name: '源代码管理' })).toContainText('Notes.md')
  await page.getByRole('button', { name: '关闭源代码管理' }).click()
  await page.getByRole('button', { name: '添加 WebDAV 或 SMB 连接' }).click()
  const dialog = page.getByRole('dialog')
  await expect(dialog).toContainText('WebDAV')
  await expect(dialog).toContainText('SMB')
  await page.getByRole('button', { name: '关闭', exact: true }).click()
  await page.getByText('Notes.md', { exact: true }).first().click({ button: 'right' })
  await page.getByRole('menuitem', { name: '压缩为…' }).hover()
  await expect(page.getByRole('menuitem', { name: 'TAR.GZ' })).toBeVisible()
})
