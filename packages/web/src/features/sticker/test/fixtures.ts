/** Test-only sticker, set and fake-API factories for this feature's tests. */
import type { InstalledStickerSets, Sticker, StickerSet, StickersApi, StoredMessage } from '@tg/core'

export { FakeClock } from '../../presence/testClock'

export function makeSticker(id: string, emojis: string[] = ['😀'], setId = 'set-1'): Sticker {
  return {
    id,
    set_id: setId,
    emoji: emojis[0] ?? '',
    emojis,
    format: 'tgs',
    mime_type: 'application/x-tgsticker',
    width: 512,
    height: 512,
    duration_ms: 3000,
    size_bytes: 1000,
    file_url: `/api/stickers/${id}/file?key=k`,
  }
}

export function makeSet(id: string, stickers: Sticker[], extra: Partial<StickerSet> = {}): StickerSet {
  return {
    id,
    short_name: id.replace(/-/g, '_'),
    title: `Set ${id}`,
    set_type: 'regular',
    owner_id: null,
    stickers,
    installed: true,
    archived: false,
    created_at: '2026-10-01T00:00:00Z',
    updated_at: '2026-10-01T00:00:00Z',
    ...extra,
  }
}

export function storedSticker(id: string, chatId: string, sticker: Sticker): StoredMessage {
  return {
    id,
    room_id: chatId,
    client_message_id: null,
    sender_id: 'me',
    sender: 'Me',
    sender_avatar: '',
    content: '',
    attachment: {
      id: `att-${id}`,
      file_name: 'sticker.tgs',
      mime_type: sticker.mime_type,
      size_bytes: sticker.size_bytes,
      download_url: `/api/attachments/att-${id}?key=x`,
      is_sensitive: false,
    },
    reply_to: null,
    recalled_at: null,
    edited_at: null,
    created_at: '2026-10-01T10:00:00Z',
    favorite_id: null,
    forwarded_from: null,
    reactions: [],
    media_kind: 'sticker',
    sticker: {
      sticker_id: sticker.id,
      set_id: sticker.set_id,
      set_short_name: 'cats',
      emoji: sticker.emoji,
      format: sticker.format,
      width: 512,
      height: 512,
    },
  }
}

/** A scripted `StickersApi`: every call is recorded; overrides replace single methods. */
export function fakeApi(overrides: Partial<StickersApi> = {}) {
  const calls: string[] = []
  let revision = 1
  const library = (): InstalledStickerSets => ({ revision: revision++, sets: [] })
  const record =
    <A extends unknown[], R>(name: string, result: (...args: A) => R) =>
    (...args: A): R => {
      calls.push(`${name}(${args.map((arg) => JSON.stringify(arg)).join(',')})`)
      return result(...args)
    }
  const api: StickersApi = {
    installed: record('installed', async () => library()),
    install: record('install', async () => library()),
    uninstall: record('uninstall', async () => library()),
    setArchived: record('setArchived', async () => library()),
    reorder: record('reorder', async () => library()),
    set: record('set', async () => null),
    recent: record('recent', async () => []),
    removeRecent: record('removeRecent', async () => undefined),
    favorites: record('favorites', async () => []),
    setFavorite: record('setFavorite', async () => undefined),
    search: record('search', async () => []),
    send: record('send', async (chatId: string) => storedSticker('m1', chatId, makeSticker('s'))),
    ...overrides,
  }
  return { api, calls }
}
