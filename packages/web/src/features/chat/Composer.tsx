/**
 * The minimal composer: one auto-growing textarea, one send button. TG-104 owns the
 * full behaviour (emoji, attachments, reply/edit bars, mentions). The draft text lives
 * in `composerStore` (so a `draft_updated` from another device shows up here), and
 * every keystroke flows through the session's `setDraftText` — store + debounced cloud
 * save + typing preview in one call.
 */
import type { KeyboardEvent } from 'react'
import { composerStore, selectDraft, settingsStore } from '@tg/core'
import { useStore } from 'zustand/react'
import { IconButton } from '@tg/ui'
import { SpriteIcon } from '../shell/SpriteIcon'

export interface ComposerProps {
  chatId: string
  onSend: (text: string) => boolean
  onDraftChange: (text: string) => void
}

export function Composer({ chatId, onSend, onDraftChange }: ComposerProps) {
  const text = useStore(composerStore, (state) => selectDraft(chatId)(state).text)
  const sendShortcut = useStore(settingsStore, (state) => state.sendShortcut)

  function submit() {
    if (text.trim()) onSend(text)
  }

  function handleKeyDown(event: KeyboardEvent<HTMLTextAreaElement>) {
    if (event.key !== 'Enter' || event.nativeEvent.isComposing) return
    const plainEnterSends = sendShortcut === 'enter' && !event.shiftKey
    const shiftEnterSends = sendShortcut === 'shift-enter' && event.shiftKey
    if (plainEnterSends || shiftEnterSends) {
      event.preventDefault()
      submit()
    }
  }

  return (
    <footer className="tg-composer">
      <div className="tg-composer__field">
        <textarea
          className="tg-composer__input"
          value={text}
          placeholder="写消息…"
          aria-label="消息内容"
          rows={1}
          onChange={(event) => onDraftChange(event.target.value)}
          onKeyDown={handleKeyDown}
        />
      </div>
      <IconButton label="发送" variant="filled" size="lg" onClick={submit} disabled={!text.trim()}>
        <SpriteIcon name="send" size={22} />
      </IconButton>
    </footer>
  )
}
