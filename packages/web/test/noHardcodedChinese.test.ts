/**
 * TG-510 gate: interface copy lives in the i18n catalogs, never in components. Scans every
 * web and core source (not tests, fixtures, the benchmark harness, documentation pages or the
 * catalogs themselves) with the TypeScript parser, so comments are ignored and only real string,
 * template and JSX text count.
 */
import { expect, test } from 'bun:test'
import { readdirSync, readFileSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'
import ts from 'typescript'

const HAN = /\p{Script=Han}/u
const ROOTS = [join(import.meta.dir, '../src'), join(import.meta.dir, '../../core/src')]
const SKIP_DIRS = new Set(['test', 'fixtures', 'bench', 'docs', 'i18n', 'node_modules'])

function sources(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const full = join(dir, name)
    if (statSync(full).isDirectory()) return SKIP_DIRS.has(name) ? [] : sources(full)
    return /\.tsx?$/.test(name) && !/\.test\.tsx?$/.test(name) ? [full] : []
  })
}

export function hardcodedChinese(file: string, text: string): string[] {
  const sf = ts.createSourceFile(
    file,
    text,
    ts.ScriptTarget.Latest,
    true,
    file.endsWith('x') ? ts.ScriptKind.TSX : ts.ScriptKind.TS,
  )
  const found: string[] = []
  const visit = (node: ts.Node) => {
    const literal =
      ts.isStringLiteral(node) ||
      ts.isNoSubstitutionTemplateLiteral(node) ||
      ts.isTemplateHead(node) ||
      ts.isTemplateMiddle(node) ||
      ts.isTemplateTail(node) ||
      ts.isJsxText(node)
    if (literal && HAN.test(node.text)) {
      const line = sf.getLineAndCharacterOfPosition(node.getStart(sf)).line + 1
      found.push(`${file}:${line}: ${node.text.trim().slice(0, 40)}`)
    }
    ts.forEachChild(node, visit)
  }
  visit(sf)
  return found
}

test('no Chinese copy is hard-coded outside the i18n catalogs', () => {
  const files = ROOTS.flatMap(sources)
  expect(files.length).toBeGreaterThan(100)
  const found = files.flatMap((file) =>
    hardcodedChinese(relative(join(import.meta.dir, '../..'), file), readFileSync(file, 'utf8')),
  )
  expect(found.join('\n')).toBe('')
})

test('the gate ignores comments and catches literals, templates and JSX text', () => {
  expect(hardcodedChinese('a.tsx', '// 注释\n/* 注释 */ const a = 1')).toEqual([])
  expect(hardcodedChinese('a.tsx', "const a = '中文'")).toHaveLength(1)
  expect(hardcodedChinese('a.tsx', 'const a = `共 ${n} 条`').length).toBeGreaterThan(0)
  expect(hardcodedChinese('a.tsx', 'const a = <p>你好</p>')).toHaveLength(1)
})
