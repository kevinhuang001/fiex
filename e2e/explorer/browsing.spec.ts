import { test, expect } from '@playwright/test'
import { openDocuments, pdfFixture } from '../fixtures/helpers'

test.beforeEach(async ({ page }) => {
  await page.goto('/?fixture&pluginFixture')
  await openDocuments(page)
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
  await openDocuments(page)
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
  await expect(page.getByRole('tab')).toContainText('最近使用')
  await expect(page.getByText('Notes.md', { exact: true }).first()).toBeVisible()
})

test('closing the last tab collapses the split pane', async ({ page }) => {
  await page.getByRole('button', { name: '双面板' }).click()
  await expect(page.getByRole('complementary', { name: '第二个面板' })).toBeVisible()
  await page.getByRole('button', { name: '关闭标签页', exact: true }).click()
  await expect(page.getByRole('tab')).toHaveCount(1)
  await expect(page.getByRole('tab')).toContainText('最近使用')
  await expect(page.getByRole('complementary', { name: '第二个面板' })).toHaveCount(0)
})

test('each tab keeps its own one or two panes', async ({ page }) => {
  await page.getByText('Research', { exact: true }).dblclick()
  await page.getByRole('button', { name: '双面板' }).click()
  await expect(page.getByRole('complementary', { name: '第二个面板' })).toBeVisible()
  await expect(page.getByRole('navigation', { name: '第二个面板的路径' })).toContainText(
    'Documents',
  )
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

test('a new second pane starts in the directory visited immediately before the current one', async ({
  page,
}) => {
  await page.getByText('Research', { exact: true }).dblclick()
  await expect(page.getByRole('navigation', { name: '当前位置' })).toContainText('Research')
  await page.getByRole('button', { name: '双面板' }).click()

  const secondPath = page.getByRole('navigation', { name: '第二个面板的路径' })
  await expect(secondPath).toBeVisible()
  await expect(secondPath).not.toContainText('Research')
})

test('toolbar sorting changes only the active folder in a split view', async ({ page }) => {
  await page
    .getByRole('grid', { name: '文件', exact: true })
    .locator('[data-path$="/Projects"]')
    .dblclick()
  await expect(page.getByRole('navigation', { name: '当前位置' })).toContainText('Projects')
  await page.getByRole('button', { name: '双面板' }).click()

  const secondary = page.getByRole('complementary', { name: '第二个面板' })
  const secondaryPath = page.getByRole('navigation', { name: '第二个面板的路径' })
  await expect(secondaryPath).toContainText('Documents')
  const leftGrid = page.getByRole('grid', { name: '文件', exact: true }).first()
  const rightGrid = secondary.getByRole('grid', { name: '文件', exact: true })
  const paths = (grid: typeof leftGrid) =>
    grid
      .locator('[data-path]')
      .evaluateAll((items) => items.map((item) => item.getAttribute('data-path')))
  const leftBefore = await paths(leftGrid)
  const rightBefore = await paths(rightGrid)

  await rightGrid.locator('[data-path]').first().click()
  const sort = page.getByRole('button', { name: '排序方式' })
  await sort.click()
  await page.getByRole('menuitem', { name: '名称' }).click()
  await sort.click()
  await page.getByRole('menuitem', { name: '降序' }).click()

  await expect.poll(() => paths(leftGrid)).toEqual(leftBefore)
  await expect.poll(() => paths(rightGrid)).not.toEqual(rightBefore)
})

test('workspace opens as a compact dock and can be collapsed', async ({ page }) => {
  await page.getByRole('button', { name: '工作区' }).click()
  const dock = page.getByRole('region', { name: '工作区' })
  await expect(dock.getByText('工作区为空')).toBeVisible()
  await page.getByRole('button', { name: '工作区' }).click()
  await expect(dock).toHaveCount(0)
})

test('workspace items can be added and removed from both context menus', async ({ page }) => {
  const notes = page.locator('[data-path="/Users/kevin/Documents/Notes.md"]')
  await notes.click({ button: 'right' })
  await page.getByRole('menuitem', { name: '更多', exact: true }).click()
  await page.getByRole('menuitem', { name: '添加到工作区', exact: true }).click()

  const chip = page.locator('.workspace-chip').filter({ hasText: 'Notes.md' })
  await expect(chip).toBeVisible()
  await notes.click({ button: 'right' })
  await page.getByRole('menuitem', { name: '更多', exact: true }).click()
  await page.getByRole('menuitem', { name: '移出工作区', exact: true }).click()
  await expect(chip).toHaveCount(0)

  await notes.click({ button: 'right' })
  await page.getByRole('menuitem', { name: '更多', exact: true }).click()
  await page.getByRole('menuitem', { name: '添加到工作区', exact: true }).click()
  await expect(chip).toBeVisible()
  await chip.click({ button: 'right' })
  await page.getByRole('menuitem', { name: '移出工作区 Notes.md', exact: true }).click()
  await expect(chip).toHaveCount(0)
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

test('Info computes only the on-demand field the user selects', async ({ page }) => {
  await page.locator('[data-path="/Users/kevin/Documents/Notes.md"]').click({ button: 'right' })
  await page.getByRole('menuitem', { name: '显示简介' }).click()

  const dialog = page.getByRole('dialog', { name: '显示简介' })
  await expect(dialog).toContainText('Notes.md')
  const sections = dialog.locator('.file-info-provider-section')
  await expect(sections).toHaveCount(2)
  await expect(
    dialog.locator('.file-info-provider-title').filter({ hasText: '元数据' }),
  ).toHaveCount(1)
  await expect(
    dialog.locator('.file-info-provider-title').filter({ hasText: '校验和' }),
  ).toHaveCount(1)
  const sha256 = dialog.locator('.file-info-row').filter({ hasText: 'SHA-256' })
  const sha512 = dialog.locator('.file-info-row').filter({ hasText: 'SHA-512' })
  await expect(sha256).toContainText('尚未计算')
  await expect(sha512).toContainText('尚未计算')

  await sha256.getByRole('button', { name: '计算', exact: true }).click()
  await expect(sha256).toContainText(
    'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad',
  )
  await expect(sha512).toContainText('尚未计算')
  const compute = sha512.getByRole('button', { name: '计算', exact: true })
  await expect(compute).toBeVisible()
  const details = await sha512.locator('dd').boundingBox()
  const button = await compute.boundingBox()
  if (!details || !button) throw new Error('missing Info compute button geometry')
  expect(button.x + button.width).toBeGreaterThanOrEqual(details.x + details.width - 12)
})

test('Applications entry and search control', async ({ page }) => {
  await page.getByRole('button', { name: '应用程序', exact: true }).click()
  await expect(
    page.locator('[data-path="applications://app_fiex"]').getByText('Fiex', { exact: true }),
  ).toBeVisible()
  await expect(page.getByRole('textbox', { name: '搜索文件' })).toBeVisible()
})

test('opens the Trash inside Fiex', async ({ page }) => {
  await page.getByRole('button', { name: '废纸篓', exact: true }).click()
  await expect(page.getByText('Old draft.txt', { exact: true })).toBeVisible()
  await expect(page.getByRole('heading', { name: '废纸篓' })).toBeVisible()
})

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

test('ten thousand icons stay virtualized and support end selection and rename', async ({
  page,
}) => {
  await page.goto('/?fixture&count=10000')
  await openDocuments(page)
  await expect(page.locator('.statusbar')).toContainText('10,015')
  await page.getByRole('button', { name: '图标视图', exact: true }).click()
  await expect(page.locator('.virtual-file-grid')).toBeVisible()
  expect(await page.getByRole('gridcell').count()).toBeLessThan(180)
  const sort = page.getByRole('button', { name: '排序方式' })
  await sort.click()
  await page.getByRole('menuitem', { name: '名称' }).click()
  await sort.click()
  await page.getByRole('menuitem', { name: '降序' }).click()
  await page.getByRole('grid', { name: '文件', exact: true }).focus()
  await page.keyboard.press('End')
  await expect(page.locator('[data-path="/Users/kevin/Documents/zz-file-09999.txt"]')).toBeVisible()
  await page.keyboard.press('F2')
  await expect(page.getByRole('textbox', { name: '重命名 zz-file-09999.txt' })).toBeVisible()
  await page.keyboard.press('Escape')
  await page.getByRole('button', { name: '列表视图', exact: true }).click()
  expect(await page.locator('.file-row').count()).toBeLessThan(80)
})

test('tab dragging preserves the active folder', async ({ page }) => {
  await page.getByRole('button', { name: '新建标签页', exact: true }).click()
  await openDocuments(page)
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
