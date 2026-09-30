/**
 * The CI gate for architecture.md section 2 / decision D-002.
 *
 * Two layers on purpose:
 *   - `describe('scanner')` proves the scanner fires on real violations and stays quiet on
 *     look-alikes. A boundary check that cannot fail is worse than no check, so the check
 *     itself is tested.
 *   - `describe('packages/core')` runs it over the actual source tree, and first asserts that
 *     the walk found files at all — otherwise an empty or moved `src/` would pass forever.
 */
import { describe, expect, test } from 'bun:test'
import { readFileSync } from 'node:fs'
import { dirname, join, relative } from 'node:path'
import {
  collectSourceFiles,
  FORBIDDEN_GLOBALS,
  FORBIDDEN_MODULES,
  formatViolations,
  scanSource,
  stripSource,
} from './platformBoundary'

const packageRoot = dirname(import.meta.dir)
const sourceRoot = join(packageRoot, 'src')

const tokensOf = (source: string) => scanSource('fixture.ts', source).map((v) => `${v.rule}:${v.token}`)

describe('scanner', () => {
  test('flags a static import of a forbidden module', () => {
    expect(tokensOf("import { useState } from 'react'\n")).toEqual(['forbidden-module:react'])
  })

  test('flags a type-only import, which still couples core to React types', () => {
    expect(tokensOf("import type { ReactNode } from 'react'\n")).toEqual(['forbidden-module:react'])
  })

  test('flags a subpath of a forbidden module', () => {
    expect(tokensOf("import 'react-dom/client'\n")).toEqual(['forbidden-module:react-dom/client'])
  })

  test('flags a re-export from a forbidden module', () => {
    expect(tokensOf("export { Button } from '@tg/ui'\n")).toEqual(['forbidden-module:@tg/ui'])
  })

  test('flags a dynamic import and a require', () => {
    expect(tokensOf("await import('@tg/web')\nrequire('react')\n")).toEqual([
      'forbidden-module:@tg/web',
      'forbidden-module:react',
    ])
  })

  test('flags every forbidden global, including via globalThis', () => {
    const source = [
      'export const probe = () => {',
      '  const a = window.location.href',
      '  const b = globalThis.document.title',
      '  const c = globalThis?.localStorage.getItem("k")',
      '  const d = navigator.onLine',
      '  return [a, b, c, d]',
      '}',
    ].join('\n')
    expect(tokensOf(source)).toEqual([
      'forbidden-global:window',
      'forbidden-global:document',
      'forbidden-global:localStorage',
      'forbidden-global:navigator',
    ])
  })

  test('flags a global reached through a template interpolation', () => {
    expect(tokensOf('export const origin = `${window.origin}/api`\n')).toEqual(['forbidden-global:window'])
  })

  test('reports the line number and the source line as evidence', () => {
    const source = ['export const ok = 1', '', 'export const bad = document.title', ''].join('\n')
    const [violation] = scanSource('a/b.ts', source)
    expect(violation).toMatchObject({
      file: 'a/b.ts',
      line: 3,
      rule: 'forbidden-global',
      token: 'document',
      evidence: 'export const bad = document.title',
    })
  })

  test('ignores forbidden names inside comments', () => {
    const source = [
      "// never import { x } from 'react' here",
      '/* window, document, localStorage and navigator are banned in this package */',
      'export const safe = 1',
    ].join('\n')
    expect(tokensOf(source)).toEqual([])
  })

  test('ignores forbidden names inside string and template literals', () => {
    const source = [
      "export const label = 'window'",
      'export const note = `document and localStorage`',
      "export const escaped = 'it\\'s navigator'",
    ].join('\n')
    expect(tokensOf(source)).toEqual([])
  })

  test('ignores a property access on something that is not the global', () => {
    expect(tokensOf('export const title = (state: HostState) => state.document\n')).toEqual([])
  })

  // Documented over-strictness, not a bug: a declaration site is a bare identifier, so a
  // member literally named `document` trips the rule. Narrowing this would mean excluding
  // `name:` occurrences, which would also hide `cond ? window : other` — a real read. Being
  // over-strict can only cost a rename; being under-strict silently reopens the hole.
  test('is deliberately over-strict about members named after a forbidden global', () => {
    expect(tokensOf('export interface Host {\n  document: unknown\n}\n')).toEqual(['forbidden-global:document'])
  })

  test('ignores identifiers that merely start with a forbidden global', () => {
    expect(tokensOf('export const documentTitle = 1\nexport const windowing = 2\n')).toEqual([])
  })

  test('ignores module specifiers that merely start with a forbidden module name', () => {
    const source = ["import { View } from 'react-native'", "import { r } from 'reactive-kit'"].join('\n')
    expect(tokensOf(source)).toEqual([])
  })

  test('strips comments and string bodies without moving any line', () => {
    const source = "const a = 'x' // c\nconst b = `t${1}`\n"
    const { withStrings, withoutStrings } = stripSource(source)
    expect(withStrings).toHaveLength(source.length)
    expect(withoutStrings).toHaveLength(source.length)
    expect(withStrings.split('\n')).toHaveLength(source.split('\n').length)
    expect(withoutStrings.split('\n')).toHaveLength(source.split('\n').length)
  })
})

describe('packages/core', () => {
  const files = collectSourceFiles(sourceRoot)

  test('the scan actually reaches the source tree', () => {
    expect(files.length).toBeGreaterThan(0)
  })

  test('imports no react, react-dom, @tg/ui or @tg/web and touches no DOM global', () => {
    const violations = files.flatMap((file) => scanSource(relative(packageRoot, file), readFileSync(file, 'utf8')))
    expect(formatViolations(violations)).toBe('')
  })

  test('declares no forbidden module as a dependency', () => {
    const manifest = JSON.parse(readFileSync(join(packageRoot, 'package.json'), 'utf8')) as Record<
      string,
      Record<string, string> | undefined
    >
    const declared = ['dependencies', 'devDependencies', 'peerDependencies'].flatMap((field) =>
      Object.keys(manifest[field] ?? {}),
    )
    expect(declared.filter((name) => FORBIDDEN_MODULES.some((forbidden) => name === forbidden))).toEqual([])
  })

  test('the rule under enforcement is still the one architecture.md section 2 specifies', () => {
    expect([...FORBIDDEN_MODULES]).toEqual(['react', 'react-dom', '@tg/ui', '@tg/web'])
    expect([...FORBIDDEN_GLOBALS]).toEqual(['window', 'document', 'localStorage', 'navigator'])
  })
})
