/**
 * The bubble's own glyphs. Inline SVG rather than the shell's sprite because the ticks and
 * the tail must render in the markup tests and in the fixture page with no server behind
 * them. Always decorative: the owning control or the meta's label carries the name.
 */
import type { ReactNode } from 'react'

function Glyph({ children, size = 20, className }: { children: ReactNode; size?: number; className?: string }) {
  return (
    <svg
      className={className}
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

export const ReplyGlyph = () => (
  <Glyph>
    <path d="M10 8 5 12.5 10 17" />
    <path d="M5.5 12.5H14a5 5 0 0 1 5 5V19" />
  </Glyph>
)

export const ForwardGlyph = () => (
  <Glyph>
    <path d="M14 8l5 4.5-5 4.5" />
    <path d="M18.5 12.5H10a5 5 0 0 0-5 5V19" />
  </Glyph>
)

export const EditGlyph = () => (
  <Glyph>
    <path d="M4 20h4L19 9l-4-4L4 16z" />
  </Glyph>
)

export const CopyGlyph = () => (
  <Glyph>
    <rect x="8" y="8" width="12" height="12" rx="2" />
    <path d="M16 8V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h2" />
  </Glyph>
)

export const PinGlyph = () => (
  <Glyph>
    <path d="M9 4h6l-1 6 3 3H7l3-3z" />
    <path d="M12 16v5" />
  </Glyph>
)

/** TG-901: «保存到收藏夹» — Telegram's Saved Messages bookmark. */
export const BookmarkGlyph = () => (
  <Glyph>
    <path d="M7 4h10a1 1 0 0 1 1 1v15l-6-4-6 4V5a1 1 0 0 1 1-1z" />
  </Glyph>
)

export const SelectGlyph = () => (
  <Glyph>
    <circle cx="12" cy="12" r="8" />
    <path d="m8.5 12 2.5 2.5 4.5-5" />
  </Glyph>
)

export const DeleteGlyph = () => (
  <Glyph>
    <path d="M5 7h14M10 7V5h4v2M7 7l1 13h8l1-13" />
  </Glyph>
)

export const SmileGlyph = () => (
  <Glyph>
    <circle cx="12" cy="12" r="8" />
    <path d="M9 14.5a4 4 0 0 0 6 0" />
    <path d="M9.5 10h0M14.5 10h0" />
  </Glyph>
)

export const MoreGlyph = () => (
  <Glyph>
    <path d="M12 6h0M12 12h0M12 18h0" strokeWidth="3" />
  </Glyph>
)

export const FileGlyph = () => (
  <Glyph size={22}>
    <path d="M7 3h7l5 5v13H7z" />
    <path d="M14 3v5h5" />
  </Glyph>
)

export const PlayGlyph = () => (
  <svg width="22" height="22" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
    <path d="M8 5v14l11-7z" fill="currentColor" />
  </svg>
)

export const DeletedGlyph = () => (
  <Glyph size={16} className="tg-bubble__deleted-icon">
    <circle cx="12" cy="12" r="8" />
    <path d="M6.5 17.5 17.5 6.5" />
  </Glyph>
)

/** Telegram's ticks: one check = sent, two = read. Drawn in a 16×11 box like tweb's. */
export function TickGlyph({ double }: { double: boolean }) {
  return (
    <svg className="tg-bubble__tick" width="16" height="11" viewBox="0 0 16 11" aria-hidden="true" focusable="false">
      <path
        d={double ? 'M1 6l3 3 6-7M6.5 8.5 7.5 9.5l6-7.5' : 'M3 6l3 3 7-7.5'}
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  )
}

export const ClockGlyph = () => (
  <svg className="tg-bubble__tick" width="14" height="14" viewBox="0 0 14 14" aria-hidden="true" focusable="false">
    <circle cx="7" cy="7" r="5.5" fill="none" stroke="currentColor" strokeWidth="1.3" />
    <path d="M7 4v3.2l2 1.3" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" />
  </svg>
)

export const FailedGlyph = () => (
  <svg className="tg-bubble__tick" width="14" height="14" viewBox="0 0 14 14" aria-hidden="true" focusable="false">
    <circle cx="7" cy="7" r="6" fill="currentColor" />
    <path d="M7 3.8v3.8M7 9.8v.1" stroke="var(--tg-bubble-in)" strokeWidth="1.6" strokeLinecap="round" />
  </svg>
)

/**
 * Telegram's tail, incoming orientation (bubble to the right). The outgoing tail is the
 * same path mirrored in CSS. Filled with `currentColor`, which the CSS sets to the flat
 * bubble colour — never the gradient fill (tokens/semantic.css "Bubble geometry").
 */
export const BubbleTail = () => (
  <svg className="tg-bubble__tail" width="11" height="20" viewBox="0 0 11 20" aria-hidden="true" focusable="false">
    <path d="M11 2C11 10.5 7.6 16 1.6 18.2A1 1 0 0 0 2 20H11Z" fill="currentColor" />
  </svg>
)
