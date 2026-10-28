import { test, expect } from '@playwright/test'

test('a saved plugin connection can be edited from its context menu without duplication', async ({
  page,
}) => {
  await page.goto('/?demo&pluginFixture')
  const connection = page.locator('.connection-nav').filter({ hasText: '生产服务器' })
  await connection.click({ button: 'right' })
  await page.getByRole('menuitem', { name: '编辑连接…', exact: true }).click()
  const dialog = page.getByRole('dialog')
  await expect(dialog.getByRole('heading', { name: '编辑连接', exact: true })).toBeVisible()
  await expect(dialog.getByRole('textbox', { name: '主机', exact: true })).toHaveValue(
    'server.example.com',
  )
  await dialog.getByRole('textbox', { name: '连接名称', exact: true }).fill('Updated Session')
  await dialog.getByRole('textbox', { name: '主机', exact: true }).fill('updated.example.com')
  const credential = '-----BEGIN KEY-----\nmultiline-data\n-----END KEY-----'
  await dialog.getByRole('textbox', { name: '测试凭据', exact: true }).fill(credential)
  await expect(dialog.getByRole('textbox', { name: '测试凭据', exact: true })).toHaveValue(
    credential,
  )
  await dialog.getByRole('button', { name: '保存连接', exact: true }).click()
  await expect(dialog).toBeHidden()
  const updated = page.locator('.connection-nav').filter({ hasText: 'Updated Session' })
  await expect(updated).toHaveCount(1)
  await expect(connection).toHaveCount(0)
  await updated.click({ button: 'right' })
  await page.getByRole('menuitem', { name: '编辑连接…', exact: true }).click()
  await expect(
    page.getByRole('dialog').getByRole('textbox', { name: '主机', exact: true }),
  ).toHaveValue('updated.example.com')
})
