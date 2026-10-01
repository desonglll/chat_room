/**
 * The chat list's small glyphs. The shared sprite has no pin / mute / tick / media
 * icons, so they are drawn here as inline strokes in `currentColor` (the token layer
 * colours them). Always decorative: the owning element carries the accessible name.
 */
import type { ReactNode } from 'react'

export type ChatListIconName =
  | 'menu'
  | 'search'
  | 'close'
  | 'back'
  | 'pin'
  | 'muted'
  | 'check'
  | 'checks'
  | 'archive'
  | 'unarchive'
  | 'photo'
  | 'video'
  | 'gif'
  | 'voice'
  | 'audio'
  | 'sticker'
  | 'file'
  | 'videoNote'
  | 'poll'
  | 'location'
  | 'liveLocation'
  | 'contact'
  | 'album'

const PATHS: Record<ChatListIconName, ReactNode> = {
  menu: <path d="M4 7h16M4 12h16M4 17h16" />,
  search: (
    <>
      <circle cx="11" cy="11" r="6.5" />
      <path d="m16 16 4 4" />
    </>
  ),
  close: <path d="M6 6l12 12M18 6 6 18" />,
  back: <path d="M19 12H5m6-6-6 6 6 6" />,
  pin: <path d="M9 4h6l-1 6 3 3H7l3-3-1-6Zm3 9v7" />,
  muted: (
    <>
      <path d="M6 16V11a6 6 0 0 1 9.3-5M18 11v5l1.5 2H7" />
      <path d="M10 20a2 2 0 0 0 4 0M4 4l16 16" />
    </>
  ),
  check: <path d="m5 12.5 4.5 4.5L19 7.5" />,
  checks: <path d="m2.5 12.5 4.5 4.5 9.5-9.5m-5 9.5 1 0 9.5-9.5" />,
  archive: (
    <>
      <rect x="3.5" y="4.5" width="17" height="4" rx="1" />
      <path d="M5 8.5v10a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-10M10 12.5h4" />
    </>
  ),
  unarchive: (
    <>
      <rect x="3.5" y="4.5" width="17" height="4" rx="1" />
      <path d="M5 8.5v10a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-10M12 17v-5.5m-2.5 2.5 2.5-2.5 2.5 2.5" />
    </>
  ),
  photo: (
    <>
      <rect x="3.5" y="5" width="17" height="14" rx="2" />
      <circle cx="9" cy="10" r="1.5" />
      <path d="m4 17 5-4.5 4 3.5 2.5-2 4.5 3.5" />
    </>
  ),
  video: (
    <>
      <rect x="3.5" y="6" width="12" height="12" rx="2" />
      <path d="m15.5 10.5 5-3v9l-5-3" />
    </>
  ),
  gif: (
    <>
      <rect x="3.5" y="5.5" width="17" height="13" rx="2" />
      <path d="M8 10.5H7a1.5 1.5 0 0 0 0 3h1v-1.5M12 10v4M15.5 14v-4h2M15.5 12h1.5" />
    </>
  ),
  voice: (
    <>
      <rect x="9" y="3.5" width="6" height="11" rx="3" />
      <path d="M6 11.5a6 6 0 0 0 12 0M12 17.5v3" />
    </>
  ),
  audio: (
    <>
      <path d="M9 17.5V6l10-2v11.5" />
      <circle cx="7" cy="17.5" r="2" />
      <circle cx="17" cy="15.5" r="2" />
    </>
  ),
  sticker: (
    <>
      <path d="M20 12a8 8 0 1 1-8-8h8v8Z" />
      <path d="M9 10h.01M15 10h.01M9 14.5a4 4 0 0 0 6 0" />
    </>
  ),
  file: (
    <>
      <path d="M14 3.5H7a1.5 1.5 0 0 0-1.5 1.5v14A1.5 1.5 0 0 0 7 20.5h10a1.5 1.5 0 0 0 1.5-1.5V8L14 3.5Z" />
      <path d="M14 3.5V8h4.5" />
    </>
  ),
  videoNote: (
    <>
      <circle cx="12" cy="12" r="8.5" />
      <path d="m10.5 9 4.5 3-4.5 3V9Z" />
    </>
  ),
  poll: <path d="M5 20V12M10 20V5M15 20v-9M20 20v-5" />,
  location: (
    <>
      <path d="M12 21s-6.5-5.5-6.5-11a6.5 6.5 0 0 1 13 0c0 5.5-6.5 11-6.5 11Z" />
      <circle cx="12" cy="10" r="2.2" />
    </>
  ),
  liveLocation: (
    <>
      <circle cx="12" cy="12" r="2.2" />
      <path d="M8 8a5.5 5.5 0 0 0 0 8M16 8a5.5 5.5 0 0 1 0 8M5 5a9.5 9.5 0 0 0 0 14M19 5a9.5 9.5 0 0 1 0 14" />
    </>
  ),
  contact: (
    <>
      <circle cx="12" cy="9" r="3.5" />
      <path d="M5 20a7 7 0 0 1 14 0" />
    </>
  ),
  album: (
    <>
      <rect x="7" y="7" width="13.5" height="12.5" rx="2" />
      <path d="M17 4H5.5A2 2 0 0 0 3.5 6v11" />
      <path d="m7.5 17 4-3.5 3 2.5 2-1.5 3.5 2.5" />
    </>
  ),
}

export function ChatListIcon({
  name,
  size = 20,
  className,
}: {
  name: ChatListIconName
  size?: number
  className?: string
}) {
  return (
    <svg
      className={className ? `tg-cl-icon ${className}` : 'tg-cl-icon'}
      width={size}
      height={size}
      viewBox="0 0 24 24"
      aria-hidden="true"
      focusable="false"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.8}
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      {PATHS[name]}
    </svg>
  )
}
