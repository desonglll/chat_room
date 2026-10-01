/**
 * The GIF use cases the panel, the bubble menu and the upload button call. HTTP goes
 * through `GifsApi`; state lands in `gifStore` (this feature's own store: saved GIFs,
 * recent GIFs from chats, and aspect ratios the media reported); a sent GIF is merged into
 * `messageStore` from the POST response — the WS echo of the same id is idempotent.
 *
 * `createGifLibrary` is the seam (tests inject a fake API and fresh stores); `gifLibrary()`
 * is the app-wide instance over the real client.
 */
import {
  authStore,
  createGifsApi,
  createRandomUuid,
  messageStore,
  selectToken,
  storedMessageToBroadcast,
  type GifsApi,
  type GifUploadBody,
  type MessageStore,
  type RecentGif,
  type SavedGif,
  type SendGifSource,
} from '@tg/core'
import { createStore, type StoreApi } from 'zustand/vanilla'
import { apiClient } from '../../app/client'
import { browserFetch } from '../../app/platform'
import { activeTopicId } from '../forum/activeTopic'
import { t } from '../../i18n/index'

export interface GifState {
  status: 'idle' | 'loading' | 'ready' | 'error'
  saved: SavedGif[]
  recent: RecentGif[]
  /** Width / height reported by loaded media, keyed by file URL, for unknown geometry. */
  aspects: Record<string, number>
}

export type GifStore = StoreApi<GifState>

export function createGifStore(): GifStore {
  return createStore<GifState>()(() => ({ status: 'idle', saved: [], recent: [], aspects: {} }))
}

export const gifStore = createGifStore()

export interface GifLibraryDeps {
  api: GifsApi
  store: GifStore
  messages: MessageStore
  uuid?: () => string
}

export type GifSendResult = { ok: true } | { ok: false; code: string }

export interface GifLibrary {
  /** Loads saved and recent GIFs once; later calls share the first load. */
  ensureLoaded(): Promise<void>
  refresh(): Promise<void>
  /** Save the GIF of a message; the entry moves to the front. Never throws. */
  save(messageId: string): Promise<boolean>
  /** Optimistic; restores the entry when the server refuses. */
  remove(savedGifId: string): Promise<void>
  send(chatId: string, source: SendGifSource, replyTo?: string | null): Promise<GifSendResult>
  upload(chatId: string, file: GifUploadBody, replyTo?: string | null): Promise<GifSendResult>
  reportAspect(fileUrl: string, aspect: number): void
  /** Forget everything (account change); the next `ensureLoaded` reloads. */
  reset(): void
}

function errorCode(error: unknown): string {
  const code = (error as { serverMessage?: unknown }).serverMessage
  return typeof code === 'string' && code !== '' ? code : 'failed'
}

export function createGifLibrary({ api, store, messages, uuid = createRandomUuid }: GifLibraryDeps): GifLibrary {
  let loading: Promise<void> | null = null

  async function load(): Promise<void> {
    store.setState({ status: 'loading' })
    try {
      const [saved, recent] = await Promise.all([api.saved(), api.recent()])
      store.setState({ saved, recent, status: 'ready' })
    } catch (error) {
      store.setState({ status: 'error' })
      throw error
    }
  }

  function merge(chatId: string, stored: Parameters<typeof storedMessageToBroadcast>[0]) {
    messages.getState().applyBroadcast(chatId, storedMessageToBroadcast(stored), 'outgoing')
  }

  return {
    ensureLoaded() {
      loading ??= load().catch((error: unknown) => {
        loading = null
        throw error
      })
      return loading
    },
    async refresh() {
      loading = load()
      await loading
    },
    async save(messageId) {
      try {
        const saved = await api.save(messageId)
        store.setState((state) => ({ saved: [saved, ...state.saved.filter((entry) => entry.id !== saved.id)] }))
        return true
      } catch {
        return false
      }
    },
    async remove(savedGifId) {
      const before = store.getState().saved
      store.setState({ saved: before.filter((entry) => entry.id !== savedGifId) })
      try {
        await api.remove(savedGifId)
      } catch {
        store.setState({ saved: before })
      }
    },
    async send(chatId, source, replyTo) {
      try {
        const stored = await api.send(chatId, source, {
          reply_to: replyTo ?? undefined,
          topic_id: activeTopicId(chatId) ?? undefined,
          client_message_id: uuid(),
        })
        merge(chatId, stored)
        if ('saved_gif_id' in source) {
          // Sending moves a saved GIF to the front, as the server just did.
          store.setState((state) => {
            const sent = state.saved.find((entry) => entry.id === source.saved_gif_id)
            return sent ? { saved: [sent, ...state.saved.filter((entry) => entry !== sent)] } : {}
          })
        }
        return { ok: true }
      } catch (error) {
        return { ok: false, code: errorCode(error) }
      }
    },
    async upload(chatId, file, replyTo) {
      try {
        const stored = await api.upload(chatId, file, {
          reply_to: replyTo ?? undefined,
          topic_id: activeTopicId(chatId) ?? undefined,
          client_message_id: uuid(),
        })
        merge(chatId, stored)
        return { ok: true }
      } catch (error) {
        return { ok: false, code: errorCode(error) }
      }
    },
    reset() {
      loading = null
      store.setState({ status: 'idle', saved: [], recent: [], aspects: {} })
    },
    reportAspect(fileUrl, aspect) {
      if (!Number.isFinite(aspect) || aspect <= 0 || store.getState().aspects[fileUrl] === aspect) return
      store.setState((state) => ({ aspects: { ...state.aspects, [fileUrl]: aspect } }))
    },
  }
}

let instance: GifLibrary | null = null

export function gifLibrary(): GifLibrary {
  if (instance) return instance
  const token = () => selectToken(authStore.getState()) || null
  instance = createGifLibrary({
    api: createGifsApi(apiClient, token, browserFetch),
    store: gifStore,
    messages: messageStore,
  })
  // Saved GIFs are per account: a sign-out or account switch must not show the last one's.
  const library = instance
  authStore.subscribe((next, previous) => {
    if (selectToken(next) !== selectToken(previous)) library.reset()
  })
  return library
}

/** Upload refusals, in the client's copy. */
export function gifErrorMessage(code: string): string {
  switch (code) {
    case 'audio_not_allowed':
      return t('w.gif.9ab82e')
    case 'unsupported_format':
    case 'unsupported_codec':
      return t('w.gif.6d46b9')
    case 'file_too_large':
    case 'Payload Too Large':
      return t('w.gif.26e72c')
    case 'too_long':
      return t('w.gif.b8ebca')
    case 'forbidden':
      return t('w.gif.71935a')
    default:
      return t('w.gif.178dfa')
  }
}
