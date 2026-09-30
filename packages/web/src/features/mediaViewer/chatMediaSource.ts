/**
 * Where the viewer's media list comes from: the loaded timeline (seed) and the Chat's
 * `/files` listing (pages). The listing is authorised server-side at read time
 * (`src/attachments/file_handlers.rs`: active member with `message.send`).
 */
import type { ApiClient, ChatFilePage } from '@tg/core'
import { listChatFiles } from '@tg/core'
import type { MediaItem } from './mediaItem'
import { mediaFromFileItem } from './mediaItem'
import type { FetchMediaPage, MediaPage } from './mediaPager'

/** The server's maximum page size; most rows of a mixed page are not media. */
export const MEDIA_PAGE_SIZE = 100

/** A `/files` page reduced to what the pager needs: viewable media + raw-row cursor data. */
export function toMediaPage(page: ChatFilePage): MediaPage {
  return {
    items: page.items.map(mediaFromFileItem).filter((item): item is MediaItem => item !== null),
    nextBefore: page.next_before,
    oldestAt: page.items.at(-1)?.created_at ?? null,
  }
}

export interface ChatMediaSourceDeps {
  client: ApiClient
  token: () => string
  /** Chat password for password-protected Chats; empty when none. */
  password?: (() => string) | undefined
}

export function createChatMediaFetcher(chatId: string, deps: ChatMediaSourceDeps): FetchMediaPage {
  return async (before) => {
    const password = deps.password?.() ?? ''
    const page = await listChatFiles(
      deps.client,
      chatId,
      { token: deps.token(), ...(password ? { password } : {}) },
      'all',
      before ?? '',
      MEDIA_PAGE_SIZE,
    )
    return toMediaPage(page)
  }
}
