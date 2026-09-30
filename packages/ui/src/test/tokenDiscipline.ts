/**
 * The token-discipline scanner.
 *
 * `docs/tg/architecture.md` section 3 and `agent-protocol.md` section 5 both say components consume
 * only the semantic layer. TG-009 prefixed the primitive layer `--tg-raw-` specifically so that
 * the rule is one grep; TG-002 proved that a rule of this kind is only worth having when it is an
 * executable test. This file is the grep, as a pure function over source text, and
 * `tokenDiscipline.test.ts` is the gate plus the fixtures that prove the gate fires.
 *
 * Four rules. The first two are the ones the task mandates; the last two exist because the first
 * two alone can be walked around without meaning to.
 *
 *  1 `raw-token`            `--tg-raw-` anywhere in a scanned file.
 *  2 `colour-literal`       a hex colour or a colour function: rgb/rgba/hsl/hsla/hwb/lab/lch/
 *                           oklab/oklch/color/color-mix.
 *  3 `named-colour`         a CSS named colour as a whole word, in a `.css` file. Restricted to CSS
 *                           because in TypeScript a bare word like `pink` or `green` is far more
 *                           likely to be an accent NAME (`data-tg-accent="pink"`) than a colour,
 *                           and a check with false positives gets disabled.
 *  4 `inline-colour-style`  in `.ts`/`.tsx`, a colour-ish CSS property assigned a string literal
 *                           that is neither a theme-neutral keyword nor a `var(--tg-*)` reference.
 *                           This is the hole rules 1-3 leave: `style={{ color: 'firebrick' }}`.
 *
 * Deliberate, recorded limits:
 *  - `transparent`, `currentColor`, `inherit`, `initial`, `unset`, `revert` and `none` are allowed
 *    everywhere. None of them carries a theme decision, and `transparent` in particular is
 *    unavoidable (a gradient stop, a reset border) and has no semantic-token equivalent.
 *  - comments are stripped, so provenance notes may name a hex value. String CONTENTS are kept, so
 *    a hex hidden in a string literal is still caught.
 *  - a computed value (`backgroundColor: props.tint`) cannot be judged from source text and is not
 *    flagged. The `.css` side is where all of this package's colour lives, and that side is checked
 *    completely.
 */
export type Rule = 'raw-token' | 'colour-literal' | 'named-colour' | 'inline-colour-style'

export interface Violation {
  file: string
  line: number
  rule: Rule
  evidence: string
}

/** CSS Color 4 named colours, plus the two legacy system keywords still in wide use. */
const NAMED_COLOURS = new Set(
  `aliceblue antiquewhite aqua aquamarine azure beige bisque black blanchedalmond blue blueviolet
   brown burlywood cadetblue chartreuse chocolate coral cornflowerblue cornsilk crimson cyan
   darkblue darkcyan darkgoldenrod darkgray darkgreen darkgrey darkkhaki darkmagenta darkolivegreen
   darkorange darkorchid darkred darksalmon darkseagreen darkslateblue darkslategray darkslategrey
   darkturquoise darkviolet deeppink deepskyblue dimgray dimgrey dodgerblue firebrick floralwhite
   forestgreen fuchsia gainsboro ghostwhite gold goldenrod gray green greenyellow grey honeydew
   hotpink indianred indigo ivory khaki lavender lavenderblush lawngreen lemonchiffon lightblue
   lightcoral lightcyan lightgoldenrodyellow lightgray lightgreen lightgrey lightpink lightsalmon
   lightseagreen lightskyblue lightslategray lightslategrey lightsteelblue lightyellow lime
   limegreen linen magenta maroon mediumaquamarine mediumblue mediumorchid mediumpurple
   mediumseagreen mediumslateblue mediumspringgreen mediumturquoise mediumvioletred midnightblue
   mintcream mistyrose moccasin navajowhite navy oldlace olive olivedrab orange orangered orchid
   palegoldenrod palegreen paleturquoise palevioletred papayawhip peachpuff peru pink plum
   powderblue purple rebeccapurple red rosybrown royalblue saddlebrown salmon sandybrown seagreen
   seashell sienna silver skyblue slateblue slategray slategrey snow springgreen steelblue tan teal
   thistle tomato turquoise violet wheat white whitesmoke yellow yellowgreen
   buttonface canvastext`
    .split(/\s+/u)
    .filter(Boolean),
)

/** Values that carry no theme decision and are therefore never violations. */
const NEUTRAL_VALUES = new Set(['transparent', 'currentcolor', 'inherit', 'initial', 'unset', 'revert', 'none', ''])

const COLOUR_PROPERTIES = [
  'color',
  'background',
  'backgroundColor',
  'backgroundImage',
  'borderColor',
  'borderBlockColor',
  'borderInlineColor',
  'borderTopColor',
  'borderRightColor',
  'borderBottomColor',
  'borderLeftColor',
  'outlineColor',
  'caretColor',
  'accentColor',
  'columnRuleColor',
  'textDecorationColor',
  'textEmphasisColor',
  'boxShadow',
  'textShadow',
  'fill',
  'stroke',
  'floodColor',
  'stopColor',
]

const RAW_TOKEN = /--tg-raw-[\w-]*/g
const HEX_COLOUR = /(?<![\w&])#(?:[0-9a-fA-F]{8}|[0-9a-fA-F]{6}|[0-9a-fA-F]{4}|[0-9a-fA-F]{3})(?![0-9a-zA-Z_-])/g
const COLOUR_FUNCTION = /\b(?:rgba?|hsla?|hwb|lab|lch|oklab|oklch|color|color-mix)\s*\(/g
const CSS_DECLARATION_VALUE = /:[^;{}]*/g

// Built rather than written as a literal: a character class containing a backtick cannot appear
// inside a template literal, and writing the class with escapes is less readable than this.
const BACKTICK = String.fromCharCode(96)
const QUOTE_CLASS = `['"${BACKTICK}]`
const NOT_QUOTE = `[^'"${BACKTICK}]`
const INLINE_COLOUR_STYLE = new RegExp(
  `\\b(${COLOUR_PROPERTIES.join('|')})\\s*:\\s*${QUOTE_CLASS}${NOT_QUOTE}*${QUOTE_CLASS}`,
  'g',
)
const INLINE_COLOUR_VALUE = new RegExp(`:\\s*${QUOTE_CLASS}(${NOT_QUOTE}*)${QUOTE_CLASS}`, 'u')

type Dialect = 'css' | 'script' | 'html'

function dialectOf(file: string): Dialect {
  if (file.endsWith('.css')) return 'css'
  if (file.endsWith('.html')) return 'html'
  return 'script'
}

/**
 * Replaces comment bodies with spaces, preserving line numbers and every other offset.
 *
 * `//` is only a comment in a script: in HTML it is the middle of every `https://` URL, and in CSS
 * it is not a comment at all.
 */
function blankComments(source: string, dialect: Dialect): string {
  const openers: [string, string][] = dialect === 'html' ? [['<!--', '-->']] : [['/*', '*/']]
  let out = ''
  let index = 0
  outer: while (index < source.length) {
    for (const [open, close] of openers) {
      if (source.startsWith(open, index)) {
        const end = source.indexOf(close, index + open.length)
        const stop = end === -1 ? source.length : end + close.length
        out += source.slice(index, stop).replace(/[^\n]/gu, ' ')
        index = stop
        continue outer
      }
    }
    if (dialect === 'script' && source.startsWith('//', index)) {
      const end = source.indexOf('\n', index)
      const stop = end === -1 ? source.length : end
      out += ' '.repeat(stop - index)
      index = stop
      continue
    }
    out += source[index]
    index += 1
  }
  return out
}

function lineOf(source: string, offset: number): number {
  let line = 1
  for (let at = 0; at < offset; at += 1) {
    if (source[at] === '\n') line += 1
  }
  return line
}

function matchAll(source: string, pattern: RegExp): { index: number; text: string }[] {
  const found: { index: number; text: string }[] = []
  const scanner = new RegExp(pattern.source, pattern.flags.includes('g') ? pattern.flags : `${pattern.flags}g`)
  let match = scanner.exec(source)
  while (match !== null) {
    found.push({ index: match.index, text: match[0] })
    match = scanner.exec(source)
  }
  return found
}

function isCss(file: string): boolean {
  return file.endsWith('.css')
}

function isScript(file: string): boolean {
  return file.endsWith('.ts') || file.endsWith('.tsx')
}

export function scanSource(file: string, source: string): Violation[] {
  const code = blankComments(source, dialectOf(file))
  const violations: Violation[] = []
  const add = (rule: Rule, index: number, evidence: string) => {
    violations.push({ file, line: lineOf(code, index), rule, evidence })
  }

  for (const hit of matchAll(code, RAW_TOKEN)) add('raw-token', hit.index, hit.text)
  for (const hit of matchAll(code, HEX_COLOUR)) add('colour-literal', hit.index, hit.text)
  for (const hit of matchAll(code, COLOUR_FUNCTION)) add('colour-literal', hit.index, `${hit.text}...)`)

  if (isCss(file)) {
    // Only declaration values, so a class called `.tg-menu__slot` or a keyframe named `snow` is
    // not mistaken for a colour.
    for (const declaration of matchAll(code, CSS_DECLARATION_VALUE)) {
      for (const word of matchAll(declaration.text, /[a-zA-Z][a-zA-Z-]*/g)) {
        if (NAMED_COLOURS.has(word.text.toLowerCase())) {
          add('named-colour', declaration.index + word.index, word.text)
        }
      }
    }
  }

  if (isScript(file)) {
    for (const hit of matchAll(code, INLINE_COLOUR_STYLE)) {
      const value = INLINE_COLOUR_VALUE.exec(hit.text)?.[1] ?? ''
      const normalised = value.trim().toLowerCase()
      if (NEUTRAL_VALUES.has(normalised)) continue
      if (normalised.startsWith('var(--tg-') && !normalised.includes('--tg-raw-')) continue
      add('inline-colour-style', hit.index, hit.text)
    }
  }

  return violations
}

export function formatViolations(violations: readonly Violation[]): string {
  return violations.map((v) => `${v.file}:${v.line}  ${v.rule}: ${v.evidence}`).join('\n')
}

/**
 * What the gate scans, and what it does not.
 *
 * Scanned: everything this task wrote that ends up in front of a user — the components, their CSS,
 * the shared internals, the documentation pages, and the stylesheet entry.
 *
 * Not scanned:
 *  - `src/tokens/**` is TG-009's and is the ONE place allowed to dereference `--tg-raw-` and to
 *    hold colour literals. Scanning it would make the rule self-contradictory.
 *  - `src/test/**` is this scanner, which necessarily contains the patterns it looks for.
 */
const SCAN_ROOTS = ['primitives', 'internal', 'docs']
const SCAN_FILES = ['styles.css']
const SCAN_SUFFIXES = ['.ts', '.tsx', '.css', '.html']

export interface ScanTarget {
  /** Path relative to `packages/ui/src`, for readable failure output. */
  file: string
  source: string
}

export async function collectScanTargets(srcDir: string): Promise<ScanTarget[]> {
  const { readdir, readFile } = await import('node:fs/promises')
  const { join, relative, sep } = await import('node:path')

  const targets: ScanTarget[] = []
  const walk = async (dir: string): Promise<void> => {
    for (const entry of await readdir(dir, { withFileTypes: true })) {
      const full = join(dir, entry.name)
      if (entry.isDirectory()) {
        await walk(full)
        continue
      }
      if (!SCAN_SUFFIXES.some((suffix) => entry.name.endsWith(suffix))) continue
      if (entry.name.endsWith('.test.ts') || entry.name.endsWith('.test.tsx')) continue
      targets.push({ file: relative(srcDir, full).split(sep).join('/'), source: await readFile(full, 'utf8') })
    }
  }

  for (const root of SCAN_ROOTS) await walk(join(srcDir, root))
  for (const name of SCAN_FILES) {
    targets.push({ file: name, source: await readFile(join(srcDir, name), 'utf8') })
  }
  return targets.sort((a, b) => a.file.localeCompare(b.file))
}
