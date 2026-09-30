/**
 * The same token-discipline gate packages/ui runs, over this package's sources:
 * no `--tg-raw-*`, no colour literal, no named colour, no inline colour style.
 * The scanner is deliberately REUSED from packages/ui (a second copy would drift);
 * the relative import reaches a test-support module that the ui package does not
 * export publicly, which is acceptable for a test-only dependency between siblings.
 */
import { expect, test } from 'bun:test'
import { readdir, readFile } from 'node:fs/promises'
import { join, relative, sep } from 'node:path'
import { formatViolations, scanSource } from '../../ui/src/test/tokenDiscipline'

const ROOT = join(import.meta.dir, '..')
const SCAN_ROOTS = ['src']
const SCAN_SUFFIXES = ['.ts', '.tsx', '.css', '.html']

async function collect(): Promise<Array<{ file: string; source: string }>> {
  const targets: Array<{ file: string; source: string }> = []
  const walk = async (dir: string): Promise<void> => {
    for (const entry of await readdir(dir, { withFileTypes: true })) {
      const full = join(dir, entry.name)
      if (entry.isDirectory()) {
        await walk(full)
        continue
      }
      if (!SCAN_SUFFIXES.some((suffix) => entry.name.endsWith(suffix))) continue
      targets.push({ file: relative(ROOT, full).split(sep).join('/'), source: await readFile(full, 'utf8') })
    }
  }
  for (const root of SCAN_ROOTS) await walk(join(ROOT, root))
  targets.push({ file: 'index.html', source: await readFile(join(ROOT, 'index.html'), 'utf8') })
  return targets
}

test('packages/web reads only semantic tokens and holds no colour literal', async () => {
  const targets = await collect()
  expect(targets.length).toBeGreaterThan(10)
  const violations = targets.flatMap(({ file, source }) => scanSource(file, source))
  expect(formatViolations(violations)).toBe('')
})
