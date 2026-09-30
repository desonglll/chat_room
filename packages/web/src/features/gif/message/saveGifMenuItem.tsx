/**
 * The "保存 GIF" row of the message context menu. The row is offered on every delivered
 * GIF message; the server decides whether the viewer can read it (a refusal is silent,
 * like Telegram's). Saving loads the library lazily, so the bubble stays light.
 */
import type { BroadcastMessage } from '@tg/core'
import type { MenuItem } from '@tg/ui'
import { isGifMessage } from '../gifModel'

function SaveGifGlyph() {
  return (
    <svg
      viewBox="0 0 24 24"
      width="20"
      height="20"
      aria-hidden="true"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.8"
    >
      <rect x="3.5" y="5" width="17" height="14" rx="3" />
      <path d="M8 12h3m-1.5-1.5v3M14 10.5v3m2-3h-2m0 1.5h1.5" strokeLinecap="round" />
    </svg>
  )
}

export function saveGifMenuItem(message: BroadcastMessage): MenuItem | null {
  if (!isGifMessage(message)) return null
  return {
    id: 'save-gif',
    label: '保存 GIF',
    icon: <SaveGifGlyph />,
    onSelect: () => {
      void import('../gifLibrary').then(({ gifLibrary }) => gifLibrary().save(message.message_id))
    },
  }
}
