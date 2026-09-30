/**
 * The executable form of the two hard constraints in architecture.md section 2.
 *
 * `packages/core` must stay runnable on React Native, which means it may not import React or
 * the UI/app packages, and may not touch DOM globals. This module is the scanner; the gate
 * that runs it over `packages/core/src` is `platformBoundary.test.ts`.
 *
 * It is a lexical scanner, not a type-aware analysis. It strips comments and string contents
 * (so those may mention the forbidden names), scans template interpolations as code, and
 * deliberately scans regex literals as code too — over-strict is the safe direction here,
 * because a false negative silently reopens the exact hole this check exists to close.
 */
import { readdirSync, statSync } from 'node:fs'
import { join } from 'node:path'

export const FORBIDDEN_MODULES = ['react', 'react-dom', '@tg/ui', '@tg/web'] as const
export const FORBIDDEN_GLOBALS = ['window', 'document', 'localStorage', 'navigator'] as const

const SOURCE_EXTENSIONS = ['.ts', '.tsx', '.mts', '.cts']

export interface BoundaryViolation {
  file: string
  line: number
  rule: 'forbidden-module' | 'forbidden-global'
  token: string
  evidence: string
}

type Frame =
  | { kind: 'code'; fromTemplate: boolean; depth: number }
  | { kind: 'template' }
  | { kind: 'quote'; quote: string }

const blankOf = (text: string): string => [...text].map((ch) => (ch === '\n' ? '\n' : ' ')).join('')

/**
 * One pass, two outputs, both the same length as the input so offsets stay comparable:
 * `withStrings` blanks comments only (module specifiers survive), `withoutStrings` blanks
 * comments and string bodies (identifiers survive). Template interpolations stay as code in
 * both, because `` `${window.origin}` `` is a real way to reach a DOM global.
 */
export function stripSource(source: string): { withStrings: string; withoutStrings: string } {
  let withStrings = ''
  let withoutStrings = ''
  const stack: Frame[] = [{ kind: 'code', fromTemplate: false, depth: 0 }]
  let i = 0

  const both = (text: string) => {
    withStrings += text
    withoutStrings += text
  }
  const comment = (text: string) => {
    const blanked = blankOf(text)
    withStrings += blanked
    withoutStrings += blanked
  }
  const inString = (text: string) => {
    withStrings += text
    withoutStrings += blankOf(text)
  }

  while (i < source.length) {
    const frame = stack[stack.length - 1] as Frame
    const ch = source[i] as string

    if (frame.kind === 'quote') {
      if (ch === '\\') {
        inString(source.slice(i, i + 2))
        i += 2
      } else {
        if (ch === frame.quote) stack.pop()
        inString(ch)
        i += 1
      }
      continue
    }

    if (frame.kind === 'template') {
      if (ch === '\\') {
        inString(source.slice(i, i + 2))
        i += 2
      } else if (ch === '$' && source[i + 1] === '{') {
        inString('${')
        stack.push({ kind: 'code', fromTemplate: true, depth: 0 })
        i += 2
      } else {
        if (ch === '`') stack.pop()
        inString(ch)
        i += 1
      }
      continue
    }

    if (ch === '/' && source[i + 1] === '/') {
      const end = source.indexOf('\n', i)
      const stop = end === -1 ? source.length : end
      comment(source.slice(i, stop))
      i = stop
    } else if (ch === '/' && source[i + 1] === '*') {
      const end = source.indexOf('*/', i + 2)
      const stop = end === -1 ? source.length : end + 2
      comment(source.slice(i, stop))
      i = stop
    } else if (ch === '"' || ch === "'") {
      stack.push({ kind: 'quote', quote: ch })
      inString(ch)
      i += 1
    } else if (ch === '`') {
      stack.push({ kind: 'template' })
      inString(ch)
      i += 1
    } else if (ch === '{') {
      frame.depth += 1
      both(ch)
      i += 1
    } else if (ch === '}' && frame.depth === 0 && frame.fromTemplate) {
      stack.pop()
      inString(ch)
      i += 1
    } else {
      if (ch === '}') frame.depth = Math.max(0, frame.depth - 1)
      both(ch)
      i += 1
    }
  }

  return { withStrings, withoutStrings }
}

const SPECIFIER_PATTERNS = [
  /\bfrom\s*(['"])([^'"\n]*)\1/g,
  /\bimport\s*(['"])([^'"\n]*)\1/g,
  /\bimport\s*\(\s*(['"])([^'"\n]*)\1/g,
  /\brequire\s*\(\s*(['"])([^'"\n]*)\1/g,
]

const lineStarts = (source: string): number[] => {
  const starts = [0]
  for (let i = 0; i < source.length; i += 1) if (source[i] === '\n') starts.push(i + 1)
  return starts
}

const lineAt = (starts: number[], offset: number): number => {
  let line = 1
  for (let i = 0; i < starts.length; i += 1) if ((starts[i] as number) <= offset) line = i + 1
  return line
}

/** Scan one file's text. Pure, so the scanner itself is unit-testable on inline fixtures. */
export function scanSource(file: string, source: string): BoundaryViolation[] {
  const { withStrings, withoutStrings } = stripSource(source)
  const starts = lineStarts(source)
  const lines = source.split('\n')
  const violations: BoundaryViolation[] = []
  const evidence = (offset: number) => (lines[lineAt(starts, offset) - 1] ?? '').trim().slice(0, 120)

  for (const pattern of SPECIFIER_PATTERNS) {
    pattern.lastIndex = 0
    for (const match of withStrings.matchAll(pattern)) {
      const specifier = match[2] as string
      const forbidden = FORBIDDEN_MODULES.find((name) => specifier === name || specifier.startsWith(`${name}/`))
      if (!forbidden) continue
      const offset = match.index ?? 0
      violations.push({
        file,
        line: lineAt(starts, offset),
        rule: 'forbidden-module',
        token: specifier,
        evidence: evidence(offset),
      })
    }
  }

  // `globalThis.window` is the same access as `window`; flatten it so the bare-identifier
  // pattern below sees it, keeping the length identical so offsets still map to lines.
  const flattened = withoutStrings.replace(/globalThis[ \t]*\??[ \t]*\./g, (m) => 'globalThis'.padEnd(m.length, ' '))
  for (const token of FORBIDDEN_GLOBALS) {
    for (const match of flattened.matchAll(new RegExp(`(?<![\\w$.])${token}\\b`, 'g'))) {
      const offset = match.index ?? 0
      violations.push({
        file,
        line: lineAt(starts, offset),
        rule: 'forbidden-global',
        token,
        evidence: evidence(offset),
      })
    }
  }

  return violations.sort((a, b) => a.line - b.line || a.token.localeCompare(b.token))
}

/** Every TypeScript file under `root`, recursively. Returns absolute paths. */
export function collectSourceFiles(root: string): string[] {
  const found: string[] = []
  const walk = (dir: string) => {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const path = join(dir, entry.name)
      if (entry.isDirectory()) walk(path)
      else if (SOURCE_EXTENSIONS.some((ext) => entry.name.endsWith(ext))) found.push(path)
    }
  }
  if (statSync(root).isDirectory()) walk(root)
  return found.sort()
}

export const formatViolations = (violations: BoundaryViolation[]): string =>
  violations.map((v) => `${v.file}:${v.line}  ${v.rule}: ${v.token}\n    ${v.evidence}`).join('\n')
