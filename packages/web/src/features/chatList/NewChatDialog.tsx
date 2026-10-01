/** Minimal "new group" modal: one title field, one action. Password/type UI is M1/M2. */
import type { FormEvent } from 'react'
import { useState } from 'react'
import type { Chat } from '@tg/core'
import { ApiError, chatListStore } from '@tg/core'
import { Button, Modal, TextField } from '@tg/ui'
import { apiClient } from '../../app/client'
import { createNewChat } from './chatListController'
import { t } from '../../i18n/index'

export interface NewChatDialogProps {
  open: boolean
  onClose: () => void
  token: string
  onCreated: (chat: Chat) => void
}

export function NewChatDialog({ open, onClose, token, onCreated }: NewChatDialogProps) {
  const [title, setTitle] = useState('')
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)

  async function submit(event: FormEvent) {
    event.preventDefault()
    const trimmed = title.trim()
    if (!trimmed || busy) return
    setBusy(true)
    setError('')
    try {
      const chat = await createNewChat({ client: apiClient, token, store: chatListStore }, trimmed)
      setTitle('')
      onCreated(chat)
    } catch (failure) {
      setError(failure instanceof ApiError && failure.status === 409 ? t('w.chatList.448caf') : t('w.chatList.63598d'))
    } finally {
      setBusy(false)
    }
  }

  return (
    <Modal open={open} onClose={onClose} title={t('w.chatList.07285a')} size="sm">
      <form className="tg-new-chat" onSubmit={submit}>
        <TextField
          label={t('w.chatList.acb902')}
          value={title}
          onChange={(event) => setTitle(event.target.value)}
          error={error || undefined}
          maxLength={120}
          required
          fullWidth
          autoFocus
        />
        <Button type="submit" loading={busy} fullWidth>
          {t('w.chatList.fcbd09')}
        </Button>
      </form>
    </Modal>
  )
}
