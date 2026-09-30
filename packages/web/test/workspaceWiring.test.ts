/**
 * Proves the bun workspace is really wired, not just declared: `@tg/web` can resolve both
 * sibling packages by their published specifier rather than by a relative path.
 *
 * Deliberately a resolution test and not an API test — TG-011 owns what `@tg/core` exports, so
 * asserting on a symbol here would invent contract this task has no right to freeze.
 */
import { expect, test } from 'bun:test'

test('@tg/core resolves from @tg/web, including its subpath exports', async () => {
  expect(await import('@tg/core')).toBeDefined()
  for (const layer of ['types', 'api', 'realtime', 'stores', 'domain']) {
    expect(await import(`@tg/core/${layer}`)).toBeDefined()
  }
})

test('@tg/ui resolves from @tg/web and exports its primitives barrel', async () => {
  const ui = (await import('@tg/ui')) as Record<string, unknown>
  expect(typeof ui.VisuallyHidden).toBe('function')
})
