/** Poll glyphs: decorative inline SVG (the owning element carries the accessible name). */
import type { ReactNode } from 'react'

function Glyph({ children, size = 14 }: { children: ReactNode; size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="3"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
    >
      {children}
    </svg>
  )
}

export const CheckGlyph = () => (
  <Glyph>
    <path d="M5 12.5 10 17.5 19 7" />
  </Glyph>
)

export const CrossGlyph = () => (
  <Glyph>
    <path d="M6 6l12 12M18 6 6 18" />
  </Glyph>
)

export const BulbGlyph = () => (
  <Glyph size={16}>
    <path
      d="M9 18h6M10 21h4M12 3a6 6 0 0 0-3.5 10.9c.6.5 1 1.2 1 2.1h5c0-.9.4-1.6 1-2.1A6 6 0 0 0 12 3z"
      strokeWidth="2"
    />
  </Glyph>
)
