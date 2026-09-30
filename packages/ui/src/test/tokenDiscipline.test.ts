import { expect, test } from 'bun:test'
import { join } from 'node:path'
import { collectScanTargets, formatViolations, scanSource } from './tokenDiscipline'

const SRC = join(import.meta.dir, '..')

// ── The gate ────────────────────────────────────────────────────────────────────
// Two assertions, and the first one is not decoration: a scan that silently walks an empty tree
// passes forever. TG-002 learned this on the packages/core boundary check.

test('the token-discipline scan covers a non-empty set of files', async () => {
  const targets = await collectScanTargets(SRC)
  expect(targets.length).toBeGreaterThan(30)
  expect(targets.map((t) => t.file)).toContain('styles.css')
  expect(targets.map((t) => t.file)).toContain('primitives/Button.css')
  expect(targets.map((t) => t.file)).toContain('primitives/Button.tsx')
})

test('no component reads a --tg-raw- primitive token and none contains a colour literal', async () => {
  const targets = await collectScanTargets(SRC)
  const violations = targets.flatMap((target) => scanSource(target.file, target.source))
  expect(formatViolations(violations)).toBe('')
  expect(violations).toHaveLength(0)
})

// ── The scanner's own fixtures ──────────────────────────────────────────────────
// The check itself can be wrong, so each rule is proved to fire and proved not to fire on the
// things that look like violations and are not.

test('rule 1 fires on a raw token in CSS and in TypeScript', () => {
  expect(scanSource('x.css', '.a{color:var(--tg-raw-blue-500)}')).toMatchObject([
    { rule: 'raw-token', line: 1, evidence: '--tg-raw-blue-500' },
  ])
  expect(scanSource('x.ts', "const t = '--tg-raw-accent'")).toHaveLength(1)
})

test('rule 1 does not fire on the semantic layer', () => {
  expect(scanSource('x.css', '.a{color:var(--tg-accent);background:var(--tg-bg)}')).toEqual([])
})

test('rule 2 fires on hex colours of every legal length and on colour functions', () => {
  const hex = scanSource('x.css', '.a{color:#fff}.b{color:#ffff}.c{color:#3390ec}.d{color:#3390ecff}')
  expect(hex).toHaveLength(4)
  expect(hex.every((v) => v.rule === 'colour-literal')).toBe(true)

  for (const value of ['rgb(0 0 0)', 'rgba(1,2,3,.5)', 'hsl(0 0% 0%)', 'oklch(0.5 0 0)', 'color-mix(in srgb, a, b)']) {
    expect(scanSource('x.css', `.a{color:${value}}`)).toHaveLength(1)
  }
})

test('rule 2 does not fire on a URL fragment, an id selector or a non-hex word', () => {
  expect(scanSource('x.html', '<a href="#page=button">b</a>')).toEqual([])
  expect(scanSource('x.tsx', "const href = '#accent=cyan'")).toEqual([])
  // `#decade` is 6 characters but `d`,`e`,`c`,`a`,`d`,`e` are all hex digits, so it IS flagged and
  // that is the accepted false-positive direction: over-strict costs a rename, under-strict hides
  // a literal. `#pager` is not hex and is not flagged.
  expect(scanSource('x.css', '#pager{display:none}')).toEqual([])
  expect(scanSource('x.css', '#decade{display:none}')).toHaveLength(1)
})

test('rule 3 fires on a named colour in a CSS value and not on a class or keyframe name', () => {
  expect(scanSource('x.css', '.a{border:1px solid red}')).toMatchObject([{ rule: 'named-colour', evidence: 'red' }])
  expect(scanSource('x.css', '.tg-snow__plum{padding:0}')).toEqual([])
  expect(scanSource('x.css', '@keyframes tomato{to{opacity:1}}')).toEqual([])
})

test('rule 3 does not fire on a word that merely contains a colour name', () => {
  expect(scanSource('x.css', '.a{transition:color var(--tg-transition-fade)}')).toEqual([])
  expect(scanSource('x.css', '.a{scrollbar-color:var(--tg-scrollbar) transparent}')).toEqual([])
})

test('rule 3 is CSS-only, so an accent NAME in TypeScript is not a colour', () => {
  expect(scanSource('x.tsx', "const accents = ['blue', 'cyan', 'green', 'orange', 'pink', 'purple']")).toEqual([])
})

test('rule 4 fires on an inline colour style and not on theme-neutral or tokenised values', () => {
  expect(scanSource('x.tsx', "<div style={{ color: 'firebrick' }} />")).toMatchObject([{ rule: 'inline-colour-style' }])
  expect(scanSource('x.tsx', "const s = { stroke: 'currentColor', fill: 'none' }")).toEqual([])
  expect(scanSource('x.tsx', "const s = { backgroundColor: 'var(--tg-accent)' }")).toEqual([])
  expect(scanSource('x.tsx', "const s = { backgroundColor: 'var(--tg-raw-blue-500)' }")).toHaveLength(2)
})

test('comments are exempt but string contents are not', () => {
  expect(scanSource('x.css', '/* tweb ships #3390ec here */\n.a{color:var(--tg-accent)}')).toEqual([])
  expect(scanSource('x.ts', '// --tg-raw-accent is the primitive\nexport const a = 1')).toEqual([])
  expect(scanSource('x.ts', "export const a = '#3390ec'")).toHaveLength(1)
})

test('violation line numbers survive comment blanking', () => {
  const source = ['/* a', '   multi', '   line note */', '.a {', '  color: #fff;', '}'].join('\n')
  expect(scanSource('x.css', source)).toMatchObject([{ line: 5, rule: 'colour-literal' }])
})
