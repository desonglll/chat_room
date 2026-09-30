/**
 * The sticker library use cases the panel, the management page and the composer call.
 * HTTP goes through `StickersApi`; state lands in `stickerStore`; a sent sticker is merged
 * into `messageStore` straight from the POST response (the WS echo of the same message is
 * idempotent on `message_id`), so the sender sees it without waiting for the socket.
 *
 * `createStickerLibrary` is the seam (tests inject a fake API and fresh stores);
 * `stickerLibrary()` is the app-wide instance over the real client.
 */
import {
  authStore,
  createRandomUuid,
  createStickersApi,
  messageStore,
  selectToken,
  stickerStore,
  storedMessageToBroadcast,
  type InstalledStickerSets,
  type MessageStore,
  type Sticker,
  type StickerSet,
  type StickersApi,
  type StickerStore,
} from '@tg/core'
import { apiClient } from '../../app/client'
import { activeTopicId } from '../forum/activeTopic'

export interface StickerLibraryDeps {
  api: StickersApi
  store: StickerStore
  messages: MessageStore
  uuid?: () => string
}

export interface SendStickerInput {
  chatId: string
  sticker: Sticker
  replyTo?: string | null | undefined
}

export interface StickerLibrary {
  /** Loads sets, recents and favorites once; later calls share the first load. */
  ensureLoaded(): Promise<void>
  refresh(): Promise<void>
  install(setId: string): Promise<void>
  uninstall(setId: string): Promise<void>
  setArchived(setId: string, archived: boolean): Promise<void>
  /** Optimistic: the local order changes at once and is replaced by the server's answer. */
  reorder(setIds: readonly string[]): Promise<void>
  toggleFavorite(sticker: Sticker): Promise<void>
  removeRecent(stickerId: string): Promise<void>
  lookupSet(shortName: string): Promise<StickerSet | null>
  /** Resolves `true` once the server stored the message. Never throws. */
  send(input: SendStickerInput): Promise<boolean>
}

export function createStickerLibrary({
  api,
  store,
  messages,
  uuid = createRandomUuid,
}: StickerLibraryDeps): StickerLibrary {
  let loading: Promise<void> | null = null
  const state = () => store.getState()
  const apply = (result: InstalledStickerSets) => state().applyInstalled(result)

  async function load(): Promise<void> {
    state().setStatus('loading')
    try {
      const [installed, recent, favorites] = await Promise.all([api.installed(), api.recent(), api.favorites()])
      apply(installed)
      state().setRecent(recent)
      state().setFavorites(favorites)
      state().setStatus('ready')
    } catch (error) {
      state().setStatus('error')
      throw error
    }
  }

  /** Library writes: on failure the local copy may be stale, so re-read the truth. */
  async function write(run: () => Promise<InstalledStickerSets>): Promise<void> {
    try {
      apply(await run())
    } catch (error) {
      void api.installed().then(apply, () => undefined)
      throw error
    }
  }

  const library: StickerLibrary = {
    ensureLoaded() {
      if (state().status === 'ready') return Promise.resolve()
      if (!loading) {
        loading = load().finally(() => {
          loading = null
        })
      }
      return loading
    },
    refresh: () => load(),
    install: (setId) => write(() => api.install(setId)),
    uninstall: (setId) => write(() => api.uninstall(setId)),
    setArchived: (setId, archived) => write(() => api.setArchived(setId, archived)),
    reorder(setIds) {
      const position = new Map(setIds.map((id, index) => [id, index]))
      const rank = (set: StickerSet) => position.get(set.id) ?? setIds.length
      const sets = [...state().sets].sort((left, right) => rank(left) - rank(right))
      store.setState({ sets })
      return write(() => api.reorder(setIds))
    },
    async toggleFavorite(sticker) {
      const favorite = !state().favorites.some((entry) => entry.id === sticker.id)
      const before = state().favorites
      state().setFavorite(sticker, favorite)
      try {
        await api.setFavorite(sticker.id, favorite)
      } catch (error) {
        store.setState({ favorites: before })
        throw error
      }
    },
    async removeRecent(stickerId) {
      state().removeRecent(stickerId)
      await api.removeRecent(stickerId)
    },
    lookupSet: (shortName) => api.set(shortName),
    async send({ chatId, sticker, replyTo }) {
      try {
        const topicId = activeTopicId(chatId)
        const stored = await api.send(chatId, {
          sticker_id: sticker.id,
          reply_to: replyTo ?? undefined,
          client_message_id: uuid(),
          ...(topicId ? { topic_id: topicId } : {}),
        })
        messages.getState().applyBroadcast(chatId, storedMessageToBroadcast(stored), 'outgoing')
        state().pushRecent(sticker)
        return true
      } catch {
        return false
      }
    },
  }
  return library
}

let instance: StickerLibrary | null = null

export function stickerLibrary(): StickerLibrary {
  if (instance) return instance
  instance = createStickerLibrary({
    api: createStickersApi(apiClient, () => selectToken(authStore.getState()) || null),
    store: stickerStore,
    messages: messageStore,
  })
  // The library is per account: a sign-out or account switch must not show the last
  // account's packs, recents or favorites.
  authStore.subscribe((next, previous) => {
    if (selectToken(next) !== selectToken(previous)) stickerStore.getState().reset()
  })
  return instance
}
