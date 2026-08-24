import { expect, test } from 'bun:test'

test('keeps the application rail beside the workspace', async () => {
  const css = await Bun.file(new URL('../src/index.css', import.meta.url)).text()

  expect(css).toMatch(
    /\.app-shell\.ant-layout\s*\{[^}]*flex-direction:\s*row;/,
  )
})
