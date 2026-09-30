/**
 * The admin panel's data: the viewer's permission view, the administrator and restricted
 * lists, and the keyset-paged roster. Every mutation re-reads what it changed from the
 * server's answer instead of guessing — the server is the only authority on permissions.
 */
import { useCallback, useEffect, useRef, useState } from 'react'
import type { ChatAdminApi, ChatMemberEntry, ChatPermissionsView } from '@tg/core'
import { ApiError } from '@tg/core'

export const ROSTER_PAGE_SIZE = 50

export interface ChatAdminState {
  view: ChatPermissionsView | null
  admins: ChatMemberEntry[]
  restricted: ChatMemberEntry[]
  members: ChatMemberEntry[]
  membersCursor: string | null
  membersDone: boolean
  loading: boolean
  busy: boolean
  error: string
}

export interface ChatAdminActions {
  reload(): Promise<void>
  loadMoreMembers(): Promise<void>
  setDefaults(permissions: readonly string[]): Promise<boolean>
  appoint(userId: string, permissions: readonly string[], title: string): Promise<boolean>
  dismiss(userId: string): Promise<boolean>
  restrict(userId: string, denied: readonly string[], until: string | null): Promise<boolean>
}

const EMPTY: ChatAdminState = {
  view: null,
  admins: [],
  restricted: [],
  members: [],
  membersCursor: null,
  membersDone: false,
  loading: true,
  busy: false,
  error: '',
}

function message(error: unknown): string {
  if (error instanceof ApiError && error.status === 403) return '没有权限执行此操作'
  if (error instanceof ApiError && error.status === 409) return '不能对所有者、管理员或自己执行此操作'
  return '操作失败，请重试'
}

/** Replace `entry` in a list by user id, or drop it when `keep` says so. */
function upsert(list: ChatMemberEntry[], entry: ChatMemberEntry, keep: boolean): ChatMemberEntry[] {
  const without = list.filter((item) => item.user_id !== entry.user_id)
  return keep ? [...without, entry] : without
}

export function useChatAdmin(
  api: ChatAdminApi,
  chatId: string,
  initial?: Partial<ChatAdminState>,
): ChatAdminState & ChatAdminActions {
  const [state, setState] = useState<ChatAdminState>({ ...EMPTY, ...initial })
  const loadingMore = useRef(false)

  const reload = useCallback(async () => {
    setState((current) => ({ ...current, loading: true, error: '' }))
    try {
      const [view, admins, restricted, first] = await Promise.all([
        api.permissions(chatId),
        api.memberPage(chatId, { filter: 'admins' }),
        api.memberPage(chatId, { filter: 'restricted' }),
        api.memberPage(chatId, { limit: ROSTER_PAGE_SIZE }),
      ])
      setState((current) => ({
        ...current,
        view,
        admins: admins.items,
        restricted: restricted.items,
        members: first.items,
        membersCursor: first.next_cursor,
        membersDone: first.next_cursor === null,
        loading: false,
      }))
    } catch (error) {
      setState((current) => ({ ...current, loading: false, error: message(error) }))
    }
  }, [api, chatId])

  useEffect(() => {
    if (initial?.view) return
    void reload()
    // `initial` is a test/screenshot seed, read once on mount.
  }, [reload])

  const loadMoreMembers = useCallback(async () => {
    if (loadingMore.current || state.membersDone) return
    loadingMore.current = true
    try {
      const page = await api.memberPage(chatId, { cursor: state.membersCursor, limit: ROSTER_PAGE_SIZE })
      setState((current) => ({
        ...current,
        members: [
          ...current.members,
          ...page.items.filter((item) => !current.members.some((m) => m.user_id === item.user_id)),
        ],
        membersCursor: page.next_cursor,
        membersDone: page.next_cursor === null,
      }))
    } catch (error) {
      setState((current) => ({ ...current, error: message(error) }))
    } finally {
      loadingMore.current = false
    }
  }, [api, chatId, state.membersCursor, state.membersDone])

  /** Run one write; on success apply `after`, then refresh the viewer's own permissions. */
  const write = useCallback(
    async <T>(run: () => Promise<T>, after: (result: T, current: ChatAdminState) => ChatAdminState) => {
      setState((current) => ({ ...current, busy: true, error: '' }))
      try {
        const result = await run()
        const view = await api.permissions(chatId)
        setState((current) => ({ ...after(result, current), view, busy: false }))
        return true
      } catch (error) {
        setState((current) => ({ ...current, busy: false, error: message(error) }))
        return false
      }
    },
    [api, chatId],
  )

  const replaceEverywhere = (current: ChatAdminState, entry: ChatMemberEntry): ChatAdminState => ({
    ...current,
    admins: upsert(current.admins, entry, entry.role !== 'member'),
    restricted: upsert(current.restricted, entry, (entry.restrictions ?? []).length > 0),
    members: current.members.map((item) => (item.user_id === entry.user_id ? entry : item)),
  })

  return {
    ...state,
    reload,
    loadMoreMembers,
    setDefaults: (permissions) =>
      write(
        () => api.setDefaultPermissions(chatId, permissions),
        (view, current) => ({ ...current, view }),
      ),
    appoint: (userId, permissions, title) =>
      write(
        () => api.appointAdmin(chatId, userId, permissions, title),
        (entry, current) => replaceEverywhere(current, entry),
      ),
    dismiss: (userId) =>
      write(
        () => api.dismissAdmin(chatId, userId),
        (entry, current) => replaceEverywhere(current, entry),
      ),
    restrict: (userId, denied, until) =>
      write(
        () => api.restrictMember(chatId, userId, denied, until),
        (entry, current) => replaceEverywhere(current, entry),
      ),
  }
}
