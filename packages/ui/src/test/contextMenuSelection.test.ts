import { expect, test } from 'bun:test'
import { readFileSync } from 'node:fs'

// TG-1201: Chrome clears a text selection the moment `user-select: none` applies to an ancestor
// of it. A right-click menu that switched selection off while open therefore wiped the user's
// selection before «引用» could read it, and every quote degraded to a whole-message reply.
// Only the touch long-press states may disable selection.
const css = readFileSync(new URL('../primitives/ContextMenu.css', import.meta.url), 'utf8')

function selectorsDisablingSelection(source: string): string[] {
  const rules = [...source.replace(/\/\*[\s\S]*?\*\//g, '').matchAll(/([^{}]+)\{([^}]*)\}/g)]
  return rules
    .filter(([, , body]) => /(^|[\s;])user-select:\s*none/.test(body ?? ''))
    .flatMap(([, selectors]) => (selectors ?? '').split(',').map((selector) => selector.trim()))
}

test('the context menu disables text selection only for touch long-press states', () => {
  const selectors = selectorsDisablingSelection(css)
  expect(selectors.length).toBeGreaterThan(0)
  for (const selector of selectors) {
    expect(selector).toMatch(/\[data-tg-(holding|touch-open)\]/)
    expect(selector).not.toContain('data-tg-context-open')
  }
})
