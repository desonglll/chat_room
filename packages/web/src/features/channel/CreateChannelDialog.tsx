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
import './channel.css'

export interface CreateChannelDialogProps {
  open: boolean
  onClose: () => void
  token: string
  api: ChannelApi
  onCreated: (chat: Chat) => void
}

export function createChannelError(failure: unknown): string {
  if (failure instanceof ApiError && failure.status === 409) return '已有同名会话，换一个名字'
  if (failure instanceof ApiError && failure.status === 400) return '名称或简介不合要求'
  return '创建失败，请稍后再试'
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
    <Modal open={open} onClose={onClose} title="新建频道" size="sm">
      <form className="tg-new-channel" onSubmit={submit}>
        <TextField
          label="频道名称"
          value={title}
          onChange={(event) => setTitle(event.target.value)}
          error={error || undefined}
          maxLength={80}
          required
          fullWidth
          autoFocus
        />
        <TextField
          label="简介（可选）"
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
          label="消息署名"
          description="在频道消息下显示发布者的名字"
        />
        <Button type="submit" loading={busy} fullWidth>
          创建频道
        </Button>
      </form>
    </Modal>
  )
}
