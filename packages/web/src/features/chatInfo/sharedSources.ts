/**
 * Where each shared-content tab reads its pages from. Every source is an authorised
 * chat-scoped endpoint (active member, re-checked per request server-side), classified and
 * paged on the server (TG-803), so every page is a full page of that tab:
 *
 *   media  `/api/chats/:id/files?kind=media`     photos + videos (no GIFs, stickers, round videos)
 *   files  `/api/chats/:id/files?kind=document`  everything else that is not voice/GIF/sticker
 *   voice  `/api/chats/:id/files?kind=voice`     voice + round video messages
 *   gif    `/api/chats/:id/files?kind=gif`
 *   links  `/api/chats/:id/links`                the server's link index (text + hidden links)
 */
import type { ApiClient, Attachment, ChatFileItem } from '@tg/core'
import { listChatFiles } from '@tg/core'
import { listChatLinks } from './chatInfoApi'
import { extractLinks } from './linkExtract'
import type { FetchSharedPage, SharedPage } from './sharedPager'
import { t } from '../../i18n/index'

export type SharedTabId = 'media' | 'files' | 'links' | 'voice' | 'gif'

export const SHARED_TABS: ReadonlyArray<{ id: SharedTabId; label: string; empty: string }> = [
  {
    id: 'media',
    get label() {
      return t('w.chatInfo.fe3330')
    },
    get empty() {
      return t('w.chatInfo.8eecbb')
    },
  },
  {
    id: 'files',
    get label() {
      return t('w.chatInfo.49deaf')
    },
    get empty() {
      return t('w.chatInfo.2bcb83')
    },
  },
  {
    id: 'links',
    get label() {
      return t('w.chatInfo.715022')
    },
    get empty() {
      return t('w.chatInfo.3cbf8a')
    },
  },
  {
    id: 'voice',
    get label() {
      return t('w.chatInfo.7a73e1')
    },
    get empty() {
      return t('w.chatInfo.6b2d74')
    },
  },
  {
    id: 'gif',
    label: 'GIF',
    get empty() {
      return t('w.chatInfo.0b84a7')
    },
  },
]

export interface SharedFile {
  key: string
  messageId: string
  createdAt: string
  sender: string
  attachment: Attachment
}

export interface SharedLink {
  key: string
  messageId: string
  createdAt: string
  sender: string
  url: string
  host: string
}

export interface SharedSources {
  media: FetchSharedPage<SharedFile>
  files: FetchSharedPage<SharedFile>
  voice: FetchSharedPage<SharedFile>
  gif: FetchSharedPage<SharedFile>
  links: FetchSharedPage<SharedLink>
}

export const FILES_PAGE_SIZE = 50
export const LINKS_PAGE_SIZE = 50

/** The link's host as the row shows it (`www.` dropped), or the URL when it cannot parse. */
export function linkHost(url: string): string {
  return extractLinks(url)[0]?.host ?? url
}

export function toSharedFile(item: ChatFileItem): SharedFile {
  return {
    key: item.attachment.id,
    messageId: item.message_id,
    createdAt: item.created_at,
    sender: item.sender,
    attachment: item.attachment,
  }
}

export interface SharedSourceDeps {
  client: ApiClient
  token: () => string
}

export function createSharedSources(chatId: string, deps: SharedSourceDeps): SharedSources {
  const files =
    (kind: 'media' | 'document' | 'voice' | 'gif'): FetchSharedPage<SharedFile> =>
    async (cursor) => {
      const page = await listChatFiles(
        deps.client,
        chatId,
        { token: deps.token() },
        kind,
        cursor ?? '',
        FILES_PAGE_SIZE,
      )
      return { items: page.items.map(toSharedFile), next: page.next_before }
    }

  const links: FetchSharedPage<SharedLink> = async (cursor): Promise<SharedPage<SharedLink>> => {
    const page = await listChatLinks(deps.client, deps.token(), chatId, cursor, LINKS_PAGE_SIZE)
    return {
      items: page.items.map((row) => ({
        key: `${row.message_id}:${row.position}`,
        messageId: row.message_id,
        createdAt: row.created_at,
        sender: row.sender,
        url: row.url,
        host: linkHost(row.url),
      })),
      next: page.next ?? null,
    }
  }

  return { media: files('media'), files: files('document'), voice: files('voice'), gif: files('gif'), links }
}
