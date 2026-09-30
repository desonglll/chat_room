import { expect, test } from 'bun:test'
import { formatCount } from './badgeText'
import { initialsFrom, paletteIndex } from './initials'
import { isTopLayer, layerDepth, pushLayer, removeLayer, resetLayers } from './layerStack'

// ── Badge counter ───────────────────────────────────────────────────────────────

test('a count below the cap is shown exactly', () => {
  expect(formatCount(7, 99)).toEqual({ text: '7', exact: '7' })
  expect(formatCount(99, 99)).toEqual({ text: '99', exact: '99' })
})

test('above the cap the VISIBLE text truncates but the accessible value does not', () => {
  expect(formatCount(1284, 99)).toEqual({ text: '99+', exact: '1284' })
})

test('max 0 means never truncate, and nonsense input degrades to zero', () => {
  expect(formatCount(1284, 0).text).toBe('1284')
  expect(formatCount(-5, 99).text).toBe('0')
  expect(formatCount(Number.NaN, 99).text).toBe('0')
  expect(formatCount(3.7, 99).text).toBe('3')
})

// ── Avatar initials ─────────────────────────────────────────────────────────────

test('initials take the first and last word', () => {
  expect(initialsFrom('Ada Lovelace')).toBe('AL')
  expect(initialsFrom('Ada Byron King Lovelace')).toBe('AL')
  expect(initialsFrom('Ada')).toBe('A')
})

test('initials do not split a surrogate pair or an emoji in half', () => {
  // charAt(0) would return half of this code point and render as a replacement glyph.
  expect(initialsFrom('\u{1F680} Launch')).toBe('\u{1F680}L')
  expect(Array.from(initialsFrom('\u{1F680}'))).toHaveLength(1)
})

test('initials handle whitespace-only and empty input', () => {
  expect(initialsFrom('')).toBe('')
  expect(initialsFrom('   ')).toBe('')
})

test('the palette slot is stable and stays inside the range', () => {
  expect(paletteIndex('Ada Lovelace', 7)).toBe(paletteIndex('Ada Lovelace', 7))
  expect(paletteIndex('Ada Lovelace', 7)).not.toBe(paletteIndex('Grace Hopper', 7))
  for (const label of ['', 'a', 'Ada', '\u{1F680}', 'x'.repeat(500)]) {
    const slot = paletteIndex(label, 7)
    expect(slot).toBeGreaterThanOrEqual(0)
    expect(slot).toBeLessThan(7)
  }
  expect(paletteIndex('Ada', 0)).toBe(0)
})

// ── Overlay layer stack ─────────────────────────────────────────────────────────

test('only the topmost layer is the top, so Escape closes one overlay at a time', () => {
  resetLayers()
  const modal = pushLayer()
  expect(isTopLayer(modal)).toBe(true)

  const menu = pushLayer()
  expect(isTopLayer(menu)).toBe(true)
  expect(isTopLayer(modal)).toBe(false)

  removeLayer(menu)
  expect(isTopLayer(modal)).toBe(true)
  expect(layerDepth()).toBe(1)

  removeLayer(modal)
  expect(layerDepth()).toBe(0)
  expect(isTopLayer(modal)).toBe(false)
})

test('removing a layer out of order does not corrupt the stack', () => {
  resetLayers()
  const a = pushLayer()
  const b = pushLayer()
  removeLayer(a)
  expect(isTopLayer(b)).toBe(true)
  expect(layerDepth()).toBe(1)
  removeLayer(a)
  expect(layerDepth()).toBe(1)
  resetLayers()
})
