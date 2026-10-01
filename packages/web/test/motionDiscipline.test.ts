/**
 * TG-108 motion gate, over packages/web and packages/ui sources (token files excepted):
 *
 *  1. every `transition*` / `animation*` declaration takes its timing from `--tg-*` tokens — no
 *     literal duration, no `cubic-bezier()`, no easing keyword — so the feel is tuned in one
 *     place (`packages/ui/src/tokens`), and the reduced-motion collapse there reaches it;
 *  2. a keyframe that moves (transform / translate / scale / rotate) runs on a duration the
 *     reduced-motion collapse shortens, unless its file has its own
 *     `prefers-reduced-motion` block — nothing keeps moving for a user who asked for stillness.
 */
import { expect, test } from 'bun:test'
import { readdir, readFile } from 'node:fs/promises'
import { join, relative, sep } from 'node:path'

const PACKAGES = join(import.meta.dir, '..', '..')
const ROOTS = ['web/src', 'ui/src']

async function cssFiles(): Promise<Array<{ file: string; source: string }>> {
  const out: Array<{ file: string; source: string }> = []
  const walk = async (dir: string): Promise<void> => {
    for (const entry of await readdir(dir, { withFileTypes: true })) {
      const full = join(dir, entry.name)
      if (entry.isDirectory()) {
        if (entry.name !== 'tokens' && entry.name !== 'node_modules') await walk(full)
      } else if (entry.name.endsWith('.css')) {
        out.push({ file: relative(PACKAGES, full).split(sep).join('/'), source: await readFile(full, 'utf8') })
      }
    }
  }
  for (const root of ROOTS) await walk(join(PACKAGES, root))
  return out
}

const stripComments = (css: string) => css.replace(/\/\*[\s\S]*?\*\//g, '')

/** Removes `var(...)` (nested parentheses included) so only what a component spelled itself remains. */
function withoutVars(value: string): string {
  let out = ''
  for (let i = 0; i < value.length; i += 1) {
    if (value.startsWith('var(', i)) {
      let depth = 0
      for (; i < value.length; i += 1) {
        if (value[i] === '(') depth += 1
        else if (value[i] === ')' && (depth -= 1) === 0) break
      }
      out += 'TOKEN'
    } else out += value[i]
  }
  return out
}

const DECLARATION = /(?:^|[;{\s])((?:transition|animation)(?:-[a-z-]+)?)\s*:\s*([^;{}]+)/g
const LITERAL_TIME = /(?<![\w-])\d*\.?\d+m?s\b/
const EASING_KEYWORD = /(?<![\w-])(ease|ease-in|ease-out|ease-in-out|linear)(?![\w-])|cubic-bezier\(/

export function timingViolations(file: string, css: string): string[] {
  const found: string[] = []
  for (const match of stripComments(css).matchAll(DECLARATION)) {
    const [, property, value] = match
    const own = withoutVars(value ?? '')
    if (property === 'animation-name' || property === 'transition-property') continue
    if (LITERAL_TIME.test(own) || EASING_KEYWORD.test(own)) found.push(`${file}: ${property}: ${value?.trim()}`)
  }
  return found
}

const NON_COLLAPSING = /--tg-(duration-fade|duration-loop|ui-loop-period|transition-fade)\b/

export function stillnessViolations(file: string, css: string): string[] {
  const source = stripComments(css)
  if (source.includes('prefers-reduced-motion')) return []
  const moving = new Set<string>()
  for (const match of source.matchAll(/@keyframes\s+([\w-]+)\s*\{([\s\S]*?\})\s*\}/g)) {
    if (/transform|translate|scale|rotate/.test(match[2] ?? '')) moving.add(match[1] ?? '')
  }
  const found: string[] = []
  for (const match of source.matchAll(/animation\s*:\s*([^;{}]+)/g)) {
    const value = match[1] ?? ''
    for (const part of value.split(',')) {
      const name = part.trim().split(/\s+/)[0] ?? ''
      if (moving.has(name) && NON_COLLAPSING.test(part)) found.push(`${file}: ${part.trim()}`)
    }
  }
  return found
}

test('motion timing comes from tokens only', async () => {
  const files = await cssFiles()
  expect(files.length).toBeGreaterThan(20)
  expect(files.flatMap(({ file, source }) => timingViolations(file, source)).join('\n')).toBe('')
})

test('nothing moves under prefers-reduced-motion', async () => {
  const files = await cssFiles()
  expect(files.flatMap(({ file, source }) => stillnessViolations(file, source)).join('\n')).toBe('')
})

test('the scanners catch what they are meant to catch', () => {
  expect(timingViolations('a.css', '.x { transition: opacity 200ms ease-out; }')).toHaveLength(1)
  expect(timingViolations('a.css', '.x { animation: spin 1s linear infinite; }')).toHaveLength(1)
  expect(timingViolations('a.css', '.x { transition: transform var(--tg-motion-press); }')).toEqual([])
  expect(
    timingViolations('a.css', '.x { animation: a calc(var(--tg-duration-loop) * 2) var(--tg-ease-linear); }'),
  ).toEqual([])
  const spin =
    '@keyframes s { to { transform: rotate(1turn); } } .x { animation: s var(--tg-duration-loop) var(--tg-ease-linear); }'
  expect(stillnessViolations('a.css', spin)).toHaveLength(1)
  expect(
    stillnessViolations('a.css', `${spin} @media (prefers-reduced-motion: reduce) { .x { animation: none; } }`),
  ).toEqual([])
})
