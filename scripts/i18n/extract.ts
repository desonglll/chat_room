/**
 * TG-510 one-shot codemod: moves every user-visible Han string in a package's sources into a
 * catalog and replaces it with a `t()` call. Usage:
 *
 *   bun scripts/i18n/extract.ts <srcDir> <i18nModule> <catalogOut> <prefix>
 *
 * - string / template literals and JSX text and attributes inside functions → `t('key', …args)`;
 * - module-scope object properties → getters (`get label() { return t('key') }`), so they
 *   follow a language switch;
 * - anything else at module scope is reported, not changed.
 * Templates become `{0}`-slotted messages. Keys are `<prefix>.<area>.<hash>`; equal texts in one
 * area share a key. Test files, fixtures and documentation pages are left alone.
 */
import ts from 'typescript'
import { createHash } from 'node:crypto'
import { readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs'
import { dirname, join, relative, sep } from 'node:path'

const [srcDir, i18nModule, catalogOut, prefix] = process.argv.slice(2) as [string, string, string, string]
const HAN = /\p{Script=Han}/u
const catalog = new Map<string, string>()
const reports: string[] = []

function walk(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const full = join(dir, name)
    if (statSync(full).isDirectory()) {
      return ['test', 'fixtures', 'docs', 'i18n', 'node_modules'].includes(name) ? [] : walk(full)
    }
    return /\.(ts|tsx)$/.test(name) && !/\.test\.tsx?$/.test(name) && !name.endsWith('.d.ts') ? [full] : []
  })
}

function areaOf(file: string): string {
  const parts = relative(srcDir, file).split(sep)
  const at = parts.indexOf('features')
  const area = at >= 0 ? parts[at + 1] : parts.length > 1 ? parts[0] : parts[0]!.replace(/\.tsx?$/, '')
  return (area ?? 'app').replace(/[^A-Za-z0-9]/g, '')
}

function keyFor(area: string, text: string): string {
  const hash = createHash('sha1').update(text).digest('hex').slice(0, 6)
  const key = `${prefix}.${area}.${hash}`
  const existing = catalog.get(key)
  if (existing !== undefined && existing !== text) throw new Error(`hash collision ${key}`)
  catalog.set(key, text)
  return key
}

const isFunctionLike = (node: ts.Node) =>
  ts.isFunctionDeclaration(node) ||
  ts.isFunctionExpression(node) ||
  ts.isArrowFunction(node) ||
  ts.isMethodDeclaration(node) ||
  ts.isGetAccessorDeclaration(node) ||
  ts.isSetAccessorDeclaration(node) ||
  ts.isConstructorDeclaration(node)

function inFunction(node: ts.Node): boolean {
  for (let at = node.parent; at; at = at.parent) if (isFunctionLike(at)) return true
  return false
}

/** JSX text as React renders it (lines trimmed, joined by a space). */
function jsxRendered(raw: string): string {
  const lines = raw.split(/\r\n|\n|\r/)
  return lines
    .map((line, index) => {
      let value = line
      if (index > 0) value = value.trimStart()
      if (index < lines.length - 1) value = value.trimEnd()
      return value
    })
    .filter((line, index, all) => line !== '' || (all.length === 1 && index === 0))
    .join(' ')
}

const escape = (key: string) => `'${key}'`

for (const file of walk(srcDir)) {
  const source = readFileSync(file, 'utf8')
  if (!HAN.test(source)) continue
  const sf = ts.createSourceFile(file, source, ts.ScriptTarget.Latest, true, file.endsWith('x') ? ts.ScriptKind.TSX : ts.ScriptKind.TS)
  const area = areaOf(file)
  let shadowed = false
  const edits: { start: number; end: number; text: string }[] = []
  const fn = () => (shadowed ? 'translate' : 't')

  const scan = (node: ts.Node) => {
    if ((ts.isParameter(node) || ts.isVariableDeclaration(node) || ts.isBindingElement(node)) && ts.isIdentifier(node.name) && node.name.text === 't') {
      shadowed = true
    }
    ts.forEachChild(node, scan)
  }
  scan(sf)

  const replaceLiteral = (node: ts.Node, message: string, args: string[]) => {
    const call = `${fn()}(${[escape(keyFor(area, message)), ...args].join(', ')})`
    const parent = node.parent
    if (ts.isImportDeclaration(parent) || ts.isExportDeclaration(parent) || ts.isLiteralTypeNode(parent)) {
      reports.push(`${relative(srcDir, file)}: skipped (type/import) ${message}`)
      return
    }
    if (ts.isPropertyAssignment(parent) && parent.name === node) return
    if (ts.isJsxAttribute(parent)) {
      edits.push({ start: node.getStart(sf), end: node.getEnd(), text: `{${call}}` })
      return
    }
    if (inFunction(node)) {
      edits.push({ start: node.getStart(sf), end: node.getEnd(), text: call })
      return
    }
    if (ts.isPropertyAssignment(parent) && parent.initializer === node && ts.isObjectLiteralExpression(parent.parent)) {
      edits.push({ start: parent.getStart(sf), end: parent.getEnd(), text: `get ${parent.name.getText(sf)}() {\n    return ${call}\n  }` })
      return
    }
    reports.push(`${relative(srcDir, file)}:${sf.getLineAndCharacterOfPosition(node.getStart(sf)).line + 1}: module scope, not changed: ${message}`)
  }

  const visit = (node: ts.Node) => {
    if ((ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)) && HAN.test(node.text)) {
      replaceLiteral(node, node.text, [])
      return
    }
    if (ts.isTemplateExpression(node)) {
      const texts = [node.head.text, ...node.templateSpans.map((span) => span.literal.text)]
      if (texts.some((text) => HAN.test(text))) {
        let message = node.head.text
        const args: string[] = []
        node.templateSpans.forEach((span, index) => {
          message += `{${index}}${span.literal.text}`
          args.push(span.expression.getText(sf))
        })
        replaceLiteral(node, message, args)
        return
      }
    }
    if (ts.isJsxText(node) && HAN.test(node.text)) {
      const rendered = jsxRendered(node.getFullText(sf))
      const message = rendered.trim()
      if (message) {
        const lead = /^\s/.test(rendered) ? "{' '}" : ''
        const trail = /\s$/.test(rendered) ? "{' '}" : ''
        // The whole node (`pos`, not `getStart`): JSX text has no trivia, its spaces are content.
        edits.push({ start: node.pos, end: node.end, text: `${lead}{${fn()}(${escape(keyFor(area, message))})}${trail}` })
      }
      return
    }
    ts.forEachChild(node, visit)
  }
  visit(sf)
  if (edits.length === 0) continue

  let out = source
  for (const edit of edits.sort((a, b) => b.start - a.start)) out = out.slice(0, edit.start) + edit.text + out.slice(edit.end)
  let spec = relative(dirname(file), i18nModule).split(sep).join('/')
  if (!spec.startsWith('.')) spec = `./${spec}`
  const importLine = shadowed ? `import { t as translate } from '${spec}'\n` : `import { t } from '${spec}'\n`
  const imports = [...out.matchAll(/^import [\s\S]*?from ['"][^'"]+['"]\n/gm)]
  const last = imports[imports.length - 1]
  const at = last ? last.index! + last[0].length : 0
  out = out.slice(0, at) + importLine + out.slice(at)
  writeFileSync(file, out)
}

const sorted = [...catalog.entries()].sort(([a], [b]) => a.localeCompare(b))
const body = sorted.map(([key, text]) => `  ${JSON.stringify(key)}: ${JSON.stringify(text)},`).join('\n')
writeFileSync(
  catalogOut,
  `/** TG-510: source (zh-CN) messages, extracted by scripts/i18n/extract.ts. Edit freely; keep keys. */\nimport type { Catalog } from '${prefix === 'c' ? './runtime' : '@tg/core'}'\n\nexport const zh: Catalog = {\n${body}\n}\n`,
)
console.log(`${catalog.size} messages`)
for (const line of reports) console.log(line)
