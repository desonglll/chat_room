/**
 * TG-404: the calendar button Telegram shows in the input while the chat has scheduled
 * messages; it opens the scheduled-messages list (lazily loaded).
 */
import { Suspense, lazy, useState } from 'react'
import { IconButton } from '@tg/ui'
import { useScheduledMessages } from '../scheduled/useScheduled'
import { CalendarGlyph } from './icons'

const ScheduledMessagesDialog = lazy(() =>
  import('../scheduled/ScheduledMessagesDialog').then((module) => ({ default: module.ScheduledMessagesDialog })),
)

export function ScheduledEntry({ chatId }: { chatId: string }) {
  const items = useScheduledMessages(chatId)
  const [open, setOpen] = useState(false)
  return (
    <>
      {items.length > 0 ? (
        <IconButton
          label={`定时消息（${items.length}）`}
          className="tg-compose__tool"
          data-kind="scheduled"
          aria-haspopup="dialog"
          aria-expanded={open}
          onClick={() => setOpen(true)}
        >
          <CalendarGlyph size={24} />
        </IconButton>
      ) : null}
      {open ? (
        <Suspense fallback={null}>
          <ScheduledMessagesDialog open chatId={chatId} onClose={() => setOpen(false)} />
        </Suspense>
      ) : null}
    </>
  )
}
