/**
 * The viewer's glyphs, inline for the same reason as the bubble's (markup tests and no server
 * dependency). Decorative: the owning button carries the accessible name.
 */
import type { ReactNode } from 'react'

function Glyph({ children, size = 24 }: { children: ReactNode; size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
    >
      {children}
    </svg>
  )
}

export const DownloadGlyph = () => (
  <Glyph>
    <path d="M12 4v11" />
    <path d="m7 10.5 5 5 5-5" />
    <path d="M5 19.5h14" />
  </Glyph>
)

export const ForwardGlyph = () => (
  <Glyph>
    <path d="M14 7l5 5-5 5" />
    <path d="M18.5 12H10a5 5 0 0 0-5 5v1.5" />
  </Glyph>
)

export const DeleteGlyph = () => (
  <Glyph>
    <path d="M5 7h14" />
    <path d="M10 7V5h4v2" />
    <path d="M7 7l1 12h8l1-12" />
  </Glyph>
)

export const CloseGlyph = () => (
  <Glyph>
    <path d="M6 6l12 12M18 6 6 18" />
  </Glyph>
)

export const ChevronGlyph = ({ direction }: { direction: 'left' | 'right' }) => (
  <Glyph size={32}>{direction === 'left' ? <path d="M15 5l-7 7 7 7" /> : <path d="M9 5l7 7-7 7" />}</Glyph>
)

export const PlayGlyph = () => (
  <Glyph size={16}>
    <path d="M8 5.5v13l10-6.5z" fill="currentColor" stroke="none" />
  </Glyph>
)
