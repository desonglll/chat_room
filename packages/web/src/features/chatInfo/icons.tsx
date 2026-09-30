/** Inline stroke icons for the info panel; `currentColor`, always decorative. */
import type { ReactNode } from 'react'

function Glyph({ children, size = 24 }: { children: ReactNode; size?: number }) {
  return (
    <svg
      className="tg-chatinfo__glyph"
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.8}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
    >
      {children}
    </svg>
  )
}

export const CloseIcon = () => (
  <Glyph>
    <path d="M6 6l12 12M18 6L6 18" />
  </Glyph>
)

export const SearchIcon = () => (
  <Glyph>
    <circle cx="11" cy="11" r="6.5" />
    <path d="M16 16l4.5 4.5" />
  </Glyph>
)

export const BellIcon = () => (
  <Glyph>
    <path d="M6 16V11a6 6 0 0 1 12 0v5l1.5 2h-15z" />
    <path d="M10 20.5a2 2 0 0 0 4 0" />
  </Glyph>
)

export const InfoIcon = () => (
  <Glyph>
    <circle cx="12" cy="12" r="9" />
    <path d="M12 11v6M12 7.5v.01" />
  </Glyph>
)

export const AtIcon = () => (
  <Glyph>
    <circle cx="12" cy="12" r="3.5" />
    <path d="M15.5 12v1.5a2.5 2.5 0 0 0 5 0V12a8.5 8.5 0 1 0-3.4 6.8" />
  </Glyph>
)

export const FileIcon = () => (
  <Glyph>
    <path d="M7 3h7l5 5v13H7z" />
    <path d="M14 3v5h5" />
  </Glyph>
)

export const PlayIcon = () => (
  <Glyph size={20}>
    <path d="M8 5.5v13l10-6.5z" fill="currentColor" />
  </Glyph>
)

export const TimerIcon = () => (
  <Glyph>
    <circle cx="12" cy="13" r="8" />
    <path d="M12 9v4l2.5 2.5M9.5 2.5h5" />
  </Glyph>
)
