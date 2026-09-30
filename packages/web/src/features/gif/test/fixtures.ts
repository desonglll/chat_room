import type { GifsApi, RecentGif, SavedGif, StoredMessage } from '@tg/core'

export function savedGif(id: string, overrides: Partial<SavedGif> = {}): SavedGif {
  return {
    id,
    mime_type: 'video/mp4',
    size_bytes: 1000,
    width: 480,
    height: 270,
    duration_ms: 2000,
    file_url: `/api/gifs/saved/${id}/file?key=k`,
    saved_at: '2026-10-01T10:00:00Z',
    used_at: '2026-10-01T10:00:00Z',
    ...overrides,
  }
}

export function recentGif(messageId: string, overrides: Partial<RecentGif> = {}): RecentGif {
  return {
    message_id: messageId,
    room_id: 'c1',
    mime_type: 'image/gif',
    size_bytes: 1000,
    width: null,
    height: null,
    duration_ms: null,
    file_url: `/api/attachments/a-${messageId}?key=k`,
    created_at: '2026-10-01T10:00:00Z',
    ...overrides,
  }
}

export function storedGif(id: string, chatId: string, mimeType = 'video/mp4'): StoredMessage {
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
      file_name: mimeType === 'image/gif' ? 'gif.gif' : 'gif.mp4',
      mime_type: mimeType,
      size_bytes: 1000,
      download_url: `/api/attachments/att-${id}?key=x`,
      is_sensitive: false,
    },
    reply_to: null,
    recalled_at: null,
    edited_at: null,
    created_at: '2026-10-01T10:00:00Z',
    favorite_id: null,
    forwarded_from: null,
    media_kind: 'gif',
  } as StoredMessage
}

export function fakeGifsApi(overrides: Partial<GifsApi> = {}) {
  const calls: string[] = []
  const log = (name: string, ...args: unknown[]) =>
    calls.push(`${name}(${args.map((arg) => JSON.stringify(arg)).join(',')})`)
  const api: GifsApi = {
    saved: async () => (log('saved'), [savedGif('g1')]),
    recent: async () => (log('recent'), [recentGif('m1')]),
    save: async (messageId) => (log('save', messageId), savedGif(`saved-${messageId}`)),
    remove: async (id) => void log('remove', id),
    send: async (chatId, source, options) => (log('send', chatId, source, options), storedGif('sent', chatId)),
    upload: async (chatId, _file, options) => (log('upload', chatId, options), storedGif('up', chatId)),
    ...overrides,
  }
  return { api, calls }
}
