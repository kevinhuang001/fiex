import { expect, type Page } from '@playwright/test'

export async function openDocuments(page: Page) {
  await page.getByRole('button', { name: '文稿', exact: true }).click()
  await expect(page.getByRole('tab').last()).toContainText('Documents')
}

export async function chooseFixturePluginPackage(page: Page) {
  const picker = page.getByRole('dialog', { name: '打开文件' })
  await picker.getByRole('option', { name: /^Downloads/ }).dblclick()
  await picker.getByRole('option', { name: /^Example\.fiexplugin/ }).click()
  await picker.getByRole('button', { name: '打开', exact: true }).click()
}

export function pdfFixture() {
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
