/**
 * Where each shared-content tab reads its pages from. Every source is an authorised
 * chat-scoped endpoint (active member, re-checked per request server-side):
 *
 *   media  `/api/chats/:id/files?kind=all`   photos + videos, GIFs excluded
 *   files  `/api/chats/:id/files?kind=all`   documents (not image/video/audio)
 *   voice  `/api/chats/:id/files?kind=file`  `audio/*`
 *   gif    `/api/chats/:id/files?kind=image` `image/gif`
 *   links  `/api/chats/:id/messages/search?q=http`, URLs extracted client-side
 *
 * The server's `kind` filter knows only all/image/video/file, so voice/GIF/media are
 * narrowed here with `filteredSource`. The gap (no `kind=media|voice|gif`, no link
 * index) is written up for the M2/M4 owners in docs/devlog/TG-106.md.
 */
import type { ApiClient, Attachment, ChatFileItem } from '@tg/core'
import { listChatFiles, searchChatMessages } from '@tg/core'
import { attachmentKind } from '../message/content/attachmentKind'
import { extractLinks } from './linkExtract'
import type { FetchSharedPage, SharedPage } from './sharedPager'
import { filteredSource } from './sharedPager'
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

const mime = (file: SharedFile) => file.attachment.mime_type.toLowerCase()
export const isGif = (file: SharedFile) => mime(file) === 'image/gif'
export const isVoice = (file: SharedFile) => mime(file).startsWith('audio/')
export const isMedia = (file: SharedFile) => attachmentKind(file.attachment) !== 'file' && !isGif(file)
export const isDocument = (file: SharedFile) => attachmentKind(file.attachment) === 'file' && !isVoice(file)

/** Server maximum; most raw rows of a mixed page are filtered away on the narrower tabs. */
export const FILES_PAGE_SIZE = 100
export const LINKS_PAGE_SIZE = 50
/** The link search needle: the server matches it case-insensitively inside `content`. */
export const LINK_NEEDLE = 'http'

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
    (kind: 'all' | 'image' | 'file'): FetchSharedPage<SharedFile> =>
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
    const rows = await searchChatMessages(
      deps.client,
      chatId,
      LINK_NEEDLE,
      { token: deps.token() },
      cursor ?? '',
      LINKS_PAGE_SIZE,
    )
    const items = rows.flatMap((row) =>
      extractLinks(row.content).map((link, index) => ({
        key: `${row.id}:${index}`,
        messageId: row.id,
        createdAt: row.created_at,
        sender: row.sender,
        ...link,
      })),
    )
    const last = rows.at(-1)
    return { items, next: rows.length < LINKS_PAGE_SIZE || !last ? null : last.id }
  }

  return {
    media: filteredSource(files('all'), isMedia),
    files: filteredSource(files('all'), isDocument),
    voice: filteredSource(files('file'), isVoice),
    gif: filteredSource(files('image'), isGif),
    links: filteredSource(links, () => true),
  }
}
