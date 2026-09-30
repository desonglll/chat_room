/**
 * Settings glyphs: one per section plus the header's back / close / edit. Inline strokes in
 * `currentColor` (the shared sprite has none of these). Always decorative — the owning
 * control carries the accessible name.
 */
import type { ReactNode } from 'react'
import type { SettingsSectionId } from './settingsRegistry'

export type SettingsIconName = SettingsSectionId | 'back' | 'close' | 'edit' | 'chevron' | 'security'

const PATHS: Record<SettingsIconName, ReactNode> = {
  account: (
    <>
      <circle cx="12" cy="8" r="4" />
      <path d="M4.5 20a7.5 7.5 0 0 1 15 0" />
    </>
  ),
  notifications: (
    <>
      <path d="M6 16v-5a6 6 0 0 1 12 0v5l1.5 2h-15Z" />
      <path d="M10 20.5a2 2 0 0 0 4 0" />
    </>
  ),
  privacy: (
    <>
      <rect x="5" y="10.5" width="14" height="10" rx="2" />
      <path d="M8 10.5V8a4 4 0 0 1 8 0v2.5" />
    </>
  ),
  security: <path d="M12 3.5 5 6.5v5c0 4.2 3 7.6 7 9 4-1.4 7-4.8 7-9v-5Z" />,
  storage: (
    <>
      <ellipse cx="12" cy="6" rx="7" ry="2.5" />
      <path d="M5 6v12c0 1.4 3.1 2.5 7 2.5s7-1.1 7-2.5V6M5 12c0 1.4 3.1 2.5 7 2.5s7-1.1 7-2.5" />
    </>
  ),
  appearance: (
    <>
      <path d="M12 3.5a8.5 8.5 0 1 0 0 17c1.2 0 1.8-.9 1.4-2l-.4-1a1.6 1.6 0 0 1 1.5-2.2h2.2a3.3 3.3 0 0 0 3.3-3.3c0-4.7-3.8-8.5-8-8.5Z" />
      <circle cx="8" cy="11" r="1" />
      <circle cx="11.5" cy="7.5" r="1" />
      <circle cx="15.5" cy="9" r="1" />
    </>
  ),
  folders: <path d="M3.5 7.5a2 2 0 0 1 2-2h4l2 2.5h7a2 2 0 0 1 2 2v7.5a2 2 0 0 1-2 2h-13a2 2 0 0 1-2-2Z" />,
  language: (
    <>
      <circle cx="12" cy="12" r="8.5" />
      <path d="M3.5 12h17M12 3.5c2.5 2.6 3.5 5.4 3.5 8.5s-1 5.9-3.5 8.5c-2.5-2.6-3.5-5.4-3.5-8.5s1-5.9 3.5-8.5Z" />
    </>
  ),
  devices: (
    <>
      <rect x="4" y="5" width="16" height="11" rx="1.5" />
      <path d="M2.5 19h19" />
    </>
  ),
  back: <path d="M19 12H5m6-6-6 6 6 6" />,
  close: <path d="M6 6l12 12M18 6 6 18" />,
  edit: <path d="M4.5 19.5h4l10-10-4-4-10 10Zm9-13 4 4" />,
  chevron: <path d="m9 6 6 6-6 6" />,
}

export function SettingsIcon({ name, size = 22 }: { name: SettingsIconName; size?: number }) {
  return (
    <svg
      className="tg-settings-icon"
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
      {PATHS[name]}
    </svg>
  )
}
