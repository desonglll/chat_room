/**
 * «新建频道» from the sidebar's main menu: name, optional description, author signatures.
 * The creator becomes the channel's owner and lands in it.
 */
import type { FormEvent } from 'react'
import { useState } from 'react'
import type { Chat, ChannelApi } from '@tg/core'
import { ApiError, chatListStore } from '@tg/core'
import { Button, Modal, TextField, Toggle } from '@tg/ui'
import { loadConversations } from '../chatList/chatListController'
import { apiClient } from '../../app/client'
import { t } from '../../i18n/index'
import './channel.css'

export interface CreateChannelDialogProps {
  open: boolean
  onClose: () => void
  token: string
  api: ChannelApi
  onCreated: (chat: Chat) => void
}

export function createChannelError(failure: unknown): string {
  if (failure instanceof ApiError && failure.status === 409) return t('w.channel.448caf')
  if (failure instanceof ApiError && failure.status === 400) return t('w.channel.9bfe6d')
  return t('w.channel.63598d')
}

export function CreateChannelDialog({ open, onClose, token, api, onCreated }: CreateChannelDialogProps) {
  const [title, setTitle] = useState('')
  const [description, setDescription] = useState('')
  const [signatures, setSignatures] = useState(false)
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)

  async function submit(event: FormEvent) {
    event.preventDefault()
    if (!title.trim() || busy) return
    setBusy(true)
    setError('')
    try {
      const chat = await api.create({ title, description, signaturesEnabled: signatures })
      chatListStore.getState().upsertChat(chat)
      void loadConversations({ client: apiClient, token, store: chatListStore })
      setTitle('')
      setDescription('')
      setSignatures(false)
      onCreated(chat)
    } catch (failure) {
      setError(createChannelError(failure))
    } finally {
      setBusy(false)
    }
  }

  return (
    <Modal open={open} onClose={onClose} title={t('w.channel.b0e811')} size="sm">
      <form className="tg-new-channel" onSubmit={submit}>
        <TextField
          label={t('w.channel.9c38b0')}
          value={title}
          onChange={(event) => setTitle(event.target.value)}
          error={error || undefined}
          maxLength={80}
          required
          fullWidth
          autoFocus
        />
        <TextField
          label={t('w.channel.f859bf')}
          value={description}
          onChange={(event) => setDescription(event.target.value)}
          maxLength={300}
          multiline
          rows={3}
          fullWidth
        />
        <Toggle
          checked={signatures}
          onCheckedChange={setSignatures}
          label={t('w.channel.b90326')}
          description={t('w.channel.f844d0')}
        />
        <Button type="submit" loading={busy} fullWidth>
          {t('w.channel.432320')}
        </Button>
      </form>
    </Modal>
  )
}
