/**
 * TG-902: Telegram Web A's round pencil button at the bottom-right of the chat list. It opens
 * the same three entries Telegram offers — new channel, new group, new private message (the
 * contacts list) — reusing the dialogs the main menu already opens.
 */
import { useRef, useState } from 'react'
import { Menu, type MenuItem } from '@tg/ui'
import { t } from '../../i18n/index'
import './newChatFab.css'

const glyph = (path: string) => (
  <svg
    width="20"
    height="20"
    viewBox="0 0 24 24"
    aria-hidden="true"
    fill="none"
    stroke="currentColor"
    strokeWidth="1.8"
    strokeLinecap="round"
    strokeLinejoin="round"
  >
    <path d={path} />
  </svg>
)
const CHANNEL = 'M4 10v4h3l6 4V6l-6 4H4Zm13-1a4 4 0 0 1 0 6'
const GROUP = 'M9 11a3 3 0 1 0 0-6 3 3 0 0 0 0 6Zm-6 8a6 6 0 0 1 12 0M16 5a3 3 0 0 1 0 6m2 8a6 6 0 0 0-3-5.2'
const PERSON = 'M12 12a4 4 0 1 0 0-8 4 4 0 0 0 0 8Zm-7 8a7 7 0 0 1 14 0'

export function NewChatFab({
  onNewGroup,
  onNewChannel,
  onNewMessage,
}: {
  onNewGroup: () => void
  onNewChannel: () => void
  onNewMessage: () => void
}) {
  const button = useRef<HTMLButtonElement>(null)
  const [open, setOpen] = useState(false)
  const items: MenuItem[] = [
    { id: 'new-channel', label: t('w.chatList.b0e811'), icon: glyph(CHANNEL), onSelect: onNewChannel },
    { id: 'new-group', label: t('w.chatList.07285a'), icon: glyph(GROUP), onSelect: onNewGroup },
    { id: 'new-message', label: t('w.chatList.newMessage'), icon: glyph(PERSON), onSelect: onNewMessage },
  ]
  return (
    <>
      <button
        ref={button}
        type="button"
        className="tg-chatlist__fab"
        data-open={open ? '' : undefined}
        aria-label={t('w.chatList.newChat')}
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => setOpen((value) => !value)}
      >
        <svg
          width="24"
          height="24"
          viewBox="0 0 24 24"
          aria-hidden="true"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
        >
          <path d="M4 20h4L19 9l-4-4L4 16v4Z" />
          <path d="m13.5 6.5 4 4" />
        </svg>
      </button>
      <Menu
        open={open}
        onClose={() => setOpen(false)}
        anchor={button}
        items={items}
        placement="top-end"
        aria-label={t('w.chatList.newChat')}
        triggerRef={button}
      />
    </>
  )
}
