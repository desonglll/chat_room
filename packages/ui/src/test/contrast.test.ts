/**
 * TG-606 contrast audit, from the token files themselves (no browser). Two kinds of pair:
 *
 * - STRICT: body text and any text that carries information must reach WCAG AA (4.5:1) in both
 *   themes. A token change that breaks one fails here.
 * - DEVIATIONS: the integration lead's ruling (roadmap TG-606, 2026-09-30) keeps Telegram's own
 *   colours for text on accent fills and for meta/decoration that has a second, non-colour signal.
 *   Each is listed with its use and the reason; the measured ratio is pinned (±0.05) so a change
 *   is a conscious edit of this register, never an unnoticed regression. The register is
 *   mirrored in docs/tg/contrast-deviations.md.
 */
import { describe, expect, test } from 'bun:test'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'

const TOKENS = join(import.meta.dir, '../tokens')

function declarations(file: string): Map<string, string> {
  const css = readFileSync(join(TOKENS, file), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '')
  const map = new Map<string, string>()
  for (const match of css.matchAll(/(--tg-[\w-]+)\s*:\s*([^;]+);/g)) map.set(match[1]!, match[2]!.trim())
  return map
}

function themeTokens(theme: 'day' | 'night'): Map<string, string> {
  const merged = new Map([
    ...declarations('primitive.css'),
    ...declarations('semantic.css'),
    ...declarations('themes/day.css'),
  ])
  if (theme === 'night') for (const [key, value] of declarations('themes/night.css')) merged.set(key, value)
  return merged
}

type Rgba = [number, number, number, number]

function parseColour(value: string, tokens: Map<string, string>, depth = 0): Rgba | null {
  const v = value.trim()
  const ref = /^var\((--tg-[\w-]+)\)$/.exec(v)
  if (ref) return depth > 8 ? null : parseColour(tokens.get(ref[1]!) ?? '', tokens, depth + 1)
  const hex = /^#([0-9a-f]{3}|[0-9a-f]{6})$/i.exec(v)
  if (hex) {
    const h = hex[1]!.length === 3 ? [...hex[1]!].map((c) => c + c).join('') : hex[1]!
    return [parseInt(h.slice(0, 2), 16), parseInt(h.slice(2, 4), 16), parseInt(h.slice(4, 6), 16), 1]
  }
  const rgb = /^rgba?\(\s*([\d.]+)[\s,]+([\d.]+)[\s,]+([\d.]+)\s*(?:[/,]\s*([\d.]+)(%?))?\s*\)$/.exec(v)
  if (rgb) {
    const alpha = rgb[4] === undefined ? 1 : Number(rgb[4]) / (rgb[5] === '%' ? 100 : 1)
    return [Number(rgb[1]), Number(rgb[2]), Number(rgb[3]), alpha]
  }
  return null
}

const luminance = ([r, g, b]: Rgba) => {
  const channel = (c: number) => {
    const s = c / 255
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4
  }
  return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
}

const over = (top: Rgba, bottom: Rgba): Rgba => {
  const a = top[3]
  return [top[0] * a + bottom[0] * (1 - a), top[1] * a + bottom[1] * (1 - a), top[2] * a + bottom[2] * (1 - a), 1]
}

export function ratio(theme: 'day' | 'night', fg: string, bg: string): number {
  const tokens = themeTokens(theme)
  const background = parseColour(`var(${bg})`, tokens)
  const foreground = parseColour(`var(${fg})`, tokens)
  if (!background || !foreground) throw new Error(`${theme}: cannot resolve ${fg} on ${bg}`)
  const [l1, l2] = [luminance(over(foreground, background)), luminance(background)].sort((a, b) => b - a)
  return Math.round(((l1! + 0.05) / (l2! + 0.05)) * 100) / 100
}

const STRICT: [string, string][] = [
  ['--tg-text-primary', '--tg-surface'],
  ['--tg-text-secondary', '--tg-surface'],
  ['--tg-text-link', '--tg-surface'],
  ['--tg-text-danger', '--tg-surface'],
  ['--tg-bubble-in-text', '--tg-bubble-in'],
  ['--tg-bubble-out-text', '--tg-bubble-out'],
  ['--tg-text-primary', '--tg-bg-secondary'],
]

/** [theme, fg, bg, measured ratio, use, why it stays] */
export const DEVIATIONS: [string, string, string, number, string, string][] = [
  [
    'day',
    '--tg-text-on-accent',
    '--tg-accent',
    3.31,
    'primary buttons, selected chat row, unread badge',
    "Telegram's accent fill (#3390ec); the visual identity per the ruling",
  ],
  [
    'night',
    '--tg-text-on-accent',
    '--tg-accent',
    3.74,
    'primary buttons, selected chat row, unread badge (night)',
    "Telegram's night accent fill (#8774e1)",
  ],
  [
    'night',
    '--tg-bubble-out-text',
    '--tg-bubble-out',
    3.74,
    'text in outgoing bubbles (night)',
    'the outgoing bubble is an accent fill; the ruling keeps it',
  ],
  [
    'day',
    '--tg-bubble-out-meta',
    '--tg-bubble-out',
    2.75,
    'time and ticks in outgoing bubbles',
    'meta; the ticks carry the state by shape, the time is repeated in the menu',
  ],
  [
    'day',
    '--tg-text-tertiary',
    '--tg-surface',
    2.64,
    'input placeholders',
    'decorative hint; the field has a visible label or aria-label',
  ],
  [
    'night',
    '--tg-text-tertiary',
    '--tg-surface',
    4.98,
    'input placeholders (night)',
    'meets AA in night; listed for symmetry with day',
  ],
]

describe('TG-606 contrast', () => {
  for (const theme of ['day', 'night'] as const) {
    for (const [fg, bg] of STRICT) {
      if (DEVIATIONS.some(([t, f, b]) => t === theme && f === fg && b === bg)) continue
      test(`${theme}: ${fg} on ${bg} reaches AA`, () => {
        expect(ratio(theme, fg, bg)).toBeGreaterThanOrEqual(4.5)
      })
    }
  }

  test('every deviation is the documented, measured value', () => {
    for (const [theme, fg, bg, measured] of DEVIATIONS) {
      expect(
        Math.abs(ratio(theme as 'day' | 'night', fg, bg) - measured),
        `${theme} ${fg} on ${bg}`,
      ).toBeLessThanOrEqual(0.05)
    }
  })
})
