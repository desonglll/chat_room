/**
 * State of the invite-links page for one chat: the list (loaded once, reloaded after every
 * write so usage counts and states come from the server), the write actions, and the per-link
 * joined / pending lists. The server re-authorizes every call; `forbidden` hides the page.
 */
import { useCallback, useEffect, useState } from 'react'
import type { InviteLink, InviteLinkInput, InviteLinkMember, InviteLinksView } from '@tg/core'
import { ApiError } from '@tg/core'
import type { InviteLinksService } from './inviteLinksApi'

export interface InviteLinksState {
  view: InviteLinksView | null
  loading: boolean
  busy: boolean
  forbidden: boolean
  error: string | null
}

export interface LinkPeople {
  joined: InviteLinkMember[]
  pending: InviteLinkMember[]
}

const WRITE_ERRORS: Record<number, string> = {
  400: '设置无效，请检查有效期与人数上限',
  403: '你没有管理这个链接的权限',
  404: '链接不存在',
  409: '链接已撤销或是主链接，无法这样修改',
}

function describe(error: unknown): string {
  if (error instanceof ApiError) return WRITE_ERRORS[error.status] ?? `请求失败（${error.status}）`
  return '网络异常，请稍后再试'
}

export function useInviteLinks(api: InviteLinksService, chatId: string, initial?: Partial<InviteLinksState>) {
  const [state, setState] = useState<InviteLinksState>({
    view: null,
    loading: !initial?.view,
    busy: false,
    forbidden: false,
    error: null,
    ...initial,
  })

  const reload = useCallback(async () => {
    try {
      const view = await api.list(chatId)
      setState((current) => ({ ...current, view, loading: false, forbidden: false }))
    } catch (error) {
      const forbidden = error instanceof ApiError && (error.status === 403 || error.status === 404)
      setState((current) => ({ ...current, loading: false, forbidden, error: forbidden ? null : describe(error) }))
    }
  }, [api, chatId])

  const seeded = Boolean(initial?.view)
  useEffect(() => {
    if (!seeded) void reload()
  }, [reload, seeded])

  /** Run one write; reload the list afterwards. Resolves to the write's result, or null. */
  const write = useCallback(
    async <T>(action: () => Promise<T>): Promise<T | null> => {
      setState((current) => ({ ...current, busy: true, error: null }))
      try {
        const result = await action()
        await reload()
        setState((current) => ({ ...current, busy: false }))
        return result
      } catch (error) {
        setState((current) => ({ ...current, busy: false, error: describe(error) }))
        return null
      }
    },
    [reload],
  )

  const people = useCallback(
    async (linkId: string): Promise<LinkPeople> => {
      const [joined, pending] = await Promise.all([api.members(chatId, linkId), api.requests(chatId, linkId)])
      return { joined, pending }
    },
    [api, chatId],
  )

  return {
    ...state,
    reload,
    create: (input: InviteLinkInput) => write<InviteLink>(() => api.create(chatId, input)),
    edit: (linkId: string, input: InviteLinkInput) => write<InviteLink>(() => api.edit(chatId, linkId, input)),
    revoke: (linkId: string) => write<InviteLink>(() => api.revoke(chatId, linkId)),
    remove: (linkId: string) => write(() => api.remove(chatId, linkId).then(() => true)),
    replacePrimary: () => write<InviteLink>(() => api.replacePrimary(chatId)),
    settle: (userId: string, approve: boolean) =>
      write(() => api.settleRequest(chatId, userId, approve).then(() => true)),
    people,
  }
}

export type InviteLinksController = ReturnType<typeof useInviteLinks>
