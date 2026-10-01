/** TG-702: friends, both request queues and the blocklist, reloaded after every change. */
import { useCallback, useEffect, useState } from 'react'
import { useStore } from 'zustand'
import type { FriendRequestView, SocialApi, SocialUser } from '@tg/core'
import { notificationsStore } from '../notifications/notificationsStore'

export interface ContactsData {
  friends: SocialUser[]
  incoming: FriendRequestView[]
  outgoing: FriendRequestView[]
  blocked: SocialUser[]
}

export const EMPTY_CONTACTS: ContactsData = { friends: [], incoming: [], outgoing: [], blocked: [] }

export function useContacts(api: SocialApi) {
  const [data, setData] = useState<ContactsData>(EMPTY_CONTACTS)
  const [loaded, setLoaded] = useState(false)
  const [failed, setFailed] = useState(false)
  const reload = useCallback(
    () =>
      Promise.all([api.friends(), api.requests('incoming'), api.requests('outgoing'), api.blocks()]).then(
        ([friends, incoming, outgoing, blocked]) => {
          setData({ friends, incoming, outgoing, blocked })
          setLoaded(true)
          setFailed(false)
        },
        () => {
          setLoaded(true)
          setFailed(true)
        },
      ),
    [api],
  )
  // TG-801: the account socket's `social_changed` (a request arrived, was accepted, a friend
  // removed you) reloads the page live instead of waiting for a manual refresh.
  const socialRevision = useStore(notificationsStore, (state) => state.socialRevision)
  useEffect(() => void reload(), [reload, socialRevision])
  /** Run a change, then reload; resolves false when the server refused it. */
  const act = useCallback(
    (change: () => Promise<unknown>) =>
      change().then(
        () => reload().then(() => true),
        () => false,
      ),
    [reload],
  )
  return { data, loaded, failed, reload, act }
}
