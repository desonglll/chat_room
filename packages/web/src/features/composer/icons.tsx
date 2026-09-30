/**
 * The composer's glyphs, inline for the same reason as `features/message/icons.tsx`:
 * they must render in markup tests with no sprite server. Always decorative — the
 * owning control carries the accessible name.
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
      strokeWidth="1.8"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
    >
      {children}
    </svg>
  )
}

export const SmileGlyph = () => (
  <Glyph>
    <circle cx="12" cy="12" r="9" />
    <path d="M8.5 14.5a4.5 4.5 0 0 0 7 0" />
    <path d="M9 9.5h.01M15 9.5h.01" strokeWidth="2.6" />
  </Glyph>
)

export const PaperclipGlyph = () => (
  <Glyph>
    <path d="m20 11.5-7.8 7.8a5 5 0 0 1-7.1-7.1l8.1-8.1a3.3 3.3 0 0 1 4.7 4.7l-8 8a1.7 1.7 0 0 1-2.4-2.4l7.3-7.3" />
  </Glyph>
)

export const SendGlyph = () => (
  <Glyph>
    <path d="M4 12 20 4l-4 16-4-7-8-1z" fill="currentColor" stroke="none" />
  </Glyph>
)

export const MicGlyph = () => (
  <Glyph>
    <rect x="9" y="3" width="6" height="11" rx="3" />
    <path d="M5.5 11a6.5 6.5 0 0 0 13 0M12 17.5V21" />
  </Glyph>
)

export const CheckGlyph = () => (
  <Glyph>
    <path d="m5 12.5 4.5 4.5L19 7.5" strokeWidth="2.4" />
  </Glyph>
)

export const CloseGlyph = ({ size = 20 }: { size?: number }) => (
  <Glyph size={size}>
    <path d="M6 6l12 12M18 6 6 18" />
  </Glyph>
)

export const ReplyBarGlyph = () => (
  <Glyph>
    <path d="M10 8 5 12.5 10 17" />
    <path d="M5.5 12.5H14a5 5 0 0 1 5 5V19" />
  </Glyph>
)

export const EditBarGlyph = () => (
  <Glyph>
    <path d="M4 20h4L19 9l-4-4L4 16z" />
    <path d="m13.5 6.5 4 4" />
  </Glyph>
)

export const ForwardBarGlyph = () => (
  <Glyph>
    <path d="M14 8l5 4.5-5 4.5" />
    <path d="M18.5 12.5H10a5 5 0 0 0-5 5V19" />
  </Glyph>
)

export const PhotoGlyph = () => (
  <Glyph size={20}>
    <rect x="3" y="4" width="18" height="16" rx="3" />
    <circle cx="9" cy="10" r="1.8" />
    <path d="m4 18 5-5 4 4 3-3 4 4" />
  </Glyph>
)

export const FileGlyph = () => (
  <Glyph size={20}>
    <path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z" />
    <path d="M14 3v5h5" />
  </Glyph>
)

export const PollGlyph = () => (
  <Glyph size={20}>
    <path d="M6 20V11M12 20V4M18 20v-6" strokeWidth="2.2" />
  </Glyph>
)

export const LocationGlyph = () => (
  <Glyph size={20}>
    <path d="M12 21s-7-6.2-7-11.5a7 7 0 0 1 14 0C19 14.8 12 21 12 21z" />
    <circle cx="12" cy="9.5" r="2.5" />
  </Glyph>
)

export const ContactGlyph = () => (
  <Glyph size={20}>
    <circle cx="12" cy="9" r="3.5" />
    <path d="M5 20a7 7 0 0 1 14 0" />
  </Glyph>
)

export const UploadGlyph = () => (
  <Glyph size={48}>
    <path d="M12 16V4M7 9l5-5 5 5" />
    <path d="M4 16v3a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-3" />
  </Glyph>
)
