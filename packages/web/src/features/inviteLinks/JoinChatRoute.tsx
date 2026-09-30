/**
 * `/joinchat/:token` — the landing page of an invite link, Telegram's `t.me/+<hash>`: the
 * chat's preview (avatar, title, type, member count, description) and one button — join, or
 * request to join for an approval link, or open the chat for a member. A refused link says
 * why. It renders in the workspace's middle pane.
 */
import { useEffect, useState } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import type { InviteJoinResult, InvitePreview, InviteRefusal } from '@tg/core'
import { authStore, chatListStore, inviteRefusal, selectToken } from '@tg/core'
import { apiClient } from '../../app/client'
import { loadChatList } from '../chatList/chatListController'
import { inviteLinksApi } from './inviteLinksApi'
import { JoinChatCard, type JoinCardState } from './JoinChatCard'
import './inviteLinks.css'

export interface JoinChatRouteDeps {
  preview(token: string): Promise<InvitePreview>
  join(token: string): Promise<InviteJoinResult>
  /** Refresh the chat list after a join, so the sidebar shows the new chat. */
  refreshChats(): Promise<void>
}

const defaultDeps: JoinChatRouteDeps = {
  preview: (token) => inviteLinksApi.preview(token),
  join: (token) => inviteLinksApi.join(token),
  refreshChats: () =>
    loadChatList({ client: apiClient, token: selectToken(authStore.getState()), store: chatListStore }),
}

export function JoinChatRoute({ deps = defaultDeps }: { deps?: JoinChatRouteDeps }) {
  const { token = '' } = useParams()
  const navigate = useNavigate()
  const [state, setState] = useState<JoinCardState>({ kind: 'loading' })

  useEffect(() => {
    let alive = true
    setState({ kind: 'loading' })
    deps.preview(token).then(
      (preview) => alive && setState({ kind: 'preview', preview, busy: false }),
      (error: unknown) => alive && setState({ kind: 'refused', reason: inviteRefusal(error) }),
    )
    return () => {
      alive = false
    }
  }, [deps, token])

  const join = async (preview: InvitePreview) => {
    if (preview.chat_id && preview.membership_status === 'active') {
      navigate(`/chat/${preview.chat_id}`, { replace: true })
      return
    }
    setState({ kind: 'preview', preview, busy: true })
    try {
      const result = await deps.join(token)
      if (result.status === 'active' && result.chat_id) {
        await deps.refreshChats().catch(() => undefined)
        navigate(`/chat/${result.chat_id}`, { replace: true })
      } else {
        setState({ kind: 'preview', preview: { ...preview, membership_status: 'pending' }, busy: false })
      }
    } catch (error) {
      const reason: InviteRefusal = inviteRefusal(error)
      setState({ kind: 'refused', reason })
    }
  }

  return (
    <JoinChatCard
      state={state}
      onJoin={(preview) => void join(preview)}
      onDismiss={() => navigate('/', { replace: true })}
    />
  )
}
