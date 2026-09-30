/**
 * The handful of glyphs the primitives need in order to be usable on their own: a checkmark for
 * `Checkbox` and for a selected `Menu` row, a dash for the indeterminate checkbox, a cross for
 * `TextField`'s clear affordance, and a chevron for disclosure.
 *
 * `@tg/ui` deliberately ships no icon set — icon choice belongs to the application, and pulling in
 * an icon package for five shapes would be a dependency with one consumer. Every path is stroke
 * geometry on `currentColor`, so colour comes from the consuming component's token.
 */
const BOX = { viewBox: '0 0 24 24', 'aria-hidden': true, focusable: false } as const

const STROKE = {
  fill: 'none',
  stroke: 'currentColor',
  strokeWidth: 2,
  strokeLinecap: 'round',
  strokeLinejoin: 'round',
} as const

export function CheckGlyph({ className }: { className?: string | undefined }) {
  return (
    <svg {...BOX} className={className}>
      <path {...STROKE} d="M5 13l4.5 4.5L19 7" />
    </svg>
  )
}

export function DashGlyph({ className }: { className?: string | undefined }) {
  return (
    <svg {...BOX} className={className}>
      <path {...STROKE} d="M6 12h12" />
    </svg>
  )
}

export function CrossGlyph({ className }: { className?: string | undefined }) {
  return (
    <svg {...BOX} className={className}>
      <path {...STROKE} d="M6 6l12 12M18 6L6 18" />
    </svg>
  )
}

export function ChevronGlyph({ className }: { className?: string | undefined }) {
  return (
    <svg {...BOX} className={className}>
      <path {...STROKE} d="M9 6l6 6-6 6" />
    </svg>
  )
}
