/** Voice feature glyphs (24-unit viewBox, currentColor). */

export const PlayGlyph = () => (
  <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
    <path fill="currentColor" d="M8.5 5.6v12.8c0 .8.9 1.3 1.6.8l10-6.4c.6-.4.6-1.3 0-1.7l-10-6.3c-.7-.5-1.6 0-1.6.8Z" />
  </svg>
)

export const PauseGlyph = () => (
  <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
    <rect fill="currentColor" x="6.5" y="5" width="4" height="14" rx="1.2" />
    <rect fill="currentColor" x="13.5" y="5" width="4" height="14" rx="1.2" />
  </svg>
)

export const LockGlyph = ({ open = false }: { open?: boolean }) => (
  <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
    <path
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      d={open ? 'M8 11V8a4 4 0 0 1 7.6-1.7' : 'M8 11V8a4 4 0 0 1 8 0v3'}
    />
    <rect fill="currentColor" x="5.5" y="11" width="13" height="9" rx="2.5" />
  </svg>
)

export const ChevronLeftGlyph = () => (
  <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
    <path
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      d="m14 6-6 6 6 6"
    />
  </svg>
)

export const TrashGlyph = () => (
  <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
    <path
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      d="M5 7h14M10 7V5h4v2m-7 0 1 12h8l1-12"
    />
  </svg>
)
