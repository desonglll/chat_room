/** Round video message glyphs (24-unit viewBox, currentColor). */

/** The composer button in camera mode: a camcorder in a circle (Telegram's round video). */
export const CameraGlyph = () => (
  <svg
    width={24}
    height={24}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="1.8"
    strokeLinecap="round"
    strokeLinejoin="round"
    aria-hidden="true"
    focusable="false"
  >
    <circle cx="12" cy="12" r="9" />
    <rect x="7" y="9.25" width="6.5" height="5.5" rx="1.2" />
    <path d="m13.5 11.2 3.5-1.9v5.4l-3.5-1.9" />
  </svg>
)

export const SpeakerOffGlyph = () => (
  <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
    <path fill="currentColor" d="M4 9.5h3l4-3.5v12l-4-3.5H4z" />
    <path fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" d="m15 9.5 5 5m0-5-5 5" />
  </svg>
)
