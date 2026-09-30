import { expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import * as api from '../primitives'
import { DOC_PAGES, DOC_PAGE_IDS } from './pages'

/**
 * The 19 atoms named on the TG-010 roadmap card. Hard-coded rather than derived, so that removing a
 * component cannot make its own coverage check pass.
 */
const ATOMS = [
  'Button',
  'IconButton',
  'Ripple',
  'TextField',
  'Toggle',
  'Checkbox',
  'Radio',
  'Menu',
  'ContextMenu',
  'Popover',
  'Modal',
  'Sheet',
  'Tooltip',
  'Tabs',
  'Avatar',
  'Badge',
  'Spinner',
  'Skeleton',
  'ScrollArea',
] as const

test('all 19 atoms are exported from @tg/ui', () => {
  const missing = ATOMS.filter((name) => !(name in api))
  expect(missing).toEqual([])
  expect(ATOMS).toHaveLength(19)
})

test('every atom has a documentation page', () => {
  // `Radio` is documented together with `RadioGroup`, which is why the page id is `radio`.
  const expected = ATOMS.map((name) => name.replace(/([a-z0-9])([A-Z])/g, '$1-$2').toLowerCase())
  expect([...DOC_PAGE_IDS].sort()).toEqual([...expected].sort())
})

test('page ids are unique and every page has a title, a summary and notes', () => {
  expect(new Set(DOC_PAGE_IDS).size).toBe(DOC_PAGE_IDS.length)
  for (const page of DOC_PAGES) {
    expect(page.title.length).toBeGreaterThan(0)
    expect(page.summary.length).toBeGreaterThan(0)
    expect(page.notes?.length ?? 0).toBeGreaterThan(0)
  }
})

test('every page renders without throwing', () => {
  // Server rendering skips effects and portals, so this proves the render path of each demo and of
  // every component it mounts in its default state - not the overlays, which are covered by the
  // ARIA contract tests against their portal-free surfaces.
  for (const page of DOC_PAGES) {
    const { Demo } = page
    expect(() => renderToStaticMarkup(<Demo />)).not.toThrow()
  }
})
