/** Minimal "new group" modal: one title field, one action. Password/type UI is M1/M2. */
import type { FormEvent } from 'react'
import { useState } from 'react'
import type { Chat } from '@tg/core'
import { ApiError, chatListStore } from '@tg/core'
import { Button, Modal, TextField } from '@tg/ui'
import { apiClient } from '../../app/client'
import { createNewChat } from './chatListController'

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
      setError(
        failure instanceof ApiError && failure.status === 409 ? '已有同名会话，换一个名字' : '创建失败，请稍后再试',
      )
    } finally {
      setBusy(false)
    }
  }

  return (
    <Modal open={open} onClose={onClose} title="新建群组" size="sm">
      <form className="tg-new-chat" onSubmit={submit}>
        <TextField
          label="群组名称"
          value={title}
          onChange={(event) => setTitle(event.target.value)}
          error={error || undefined}
          maxLength={120}
          required
          fullWidth
          autoFocus
        />
        <Button type="submit" loading={busy} fullWidth>
          创建
        </Button>
      </form>
    </Modal>
  )
}
