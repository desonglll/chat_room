/**
 * What the second line of a chat row says, and in which voice. Priority is Telegram's:
 * someone typing > the caller's own draft > the last message > an empty-chat hint.
 * Pure — the row renders the result, tests assert it.
 */
import type { ChatType, ConversationLastMessage } from '@tg/core'
import { t } from '../../i18n/index'

export type PreviewMedia = 'photo' | 'video' | 'gif' | 'voice' | 'audio' | 'sticker' | 'file'

export type ChatPreview =
  | { kind: 'typing'; text: string }
  | { kind: 'draft'; text: string }
  | { kind: 'message'; sender: string | null; media: PreviewMedia | null; text: string; recalled: boolean }
  | { kind: 'empty'; text: string }

export interface PreviewInput {
  chatType: ChatType
  lastMessage: ConversationLastMessage | null
  currentUserId: string
  /** The caller's unsent draft for this chat; '' when none. */
  draftText: string
  /** Presence seam (TG-107): replaces the whole line while set. */
  typingText?: string | null | undefined
  /** Whether this chat is the one open in the middle pane (its draft is not "pending"). */
  active?: boolean | undefined
}

const MEDIA_BY_EXTENSION: Record<string, PreviewMedia> = {
  jpg: 'photo',
  jpeg: 'photo',
  png: 'photo',
  webp: 'photo',
  heic: 'photo',
  bmp: 'photo',
  gif: 'gif',
  mp4: 'video',
  mov: 'video',
  webm: 'video',
  mkv: 'video',
  m4v: 'video',
  ogg: 'voice',
  oga: 'voice',
  opus: 'voice',
  mp3: 'audio',
  m4a: 'audio',
  wav: 'audio',
  flac: 'audio',
  aac: 'audio',
  tgs: 'sticker',
}

export const MEDIA_LABEL: Record<PreviewMedia, string> = {
  get photo() {
    return t('w.chatList.be8da6')
  },
  get video() {
    return t('w.chatList.fa4e33')
  },
  gif: 'GIF',
  get voice() {
    return t('w.chatList.87053f')
  },
  get audio() {
    return t('w.chatList.461189')
  },
  get sticker() {
    return t('w.chatList.f7c0f3')
  },
  get file() {
    return t('w.chatList.49deaf')
  },
}

/** Media kind from the attachment name (the list summary carries no MIME type). */
export function mediaFromFileName(fileName: string | null): PreviewMedia | null {
  if (!fileName) return null
  const dot = fileName.lastIndexOf('.')
  const extension = dot >= 0 ? fileName.slice(dot + 1).toLowerCase() : ''
  return MEDIA_BY_EXTENSION[extension] ?? 'file'
}

/** One line: newlines and runs of whitespace collapse, as Telegram's preview does. */
export const singleLine = (text: string): string => text.replace(/\s+/g, ' ').trim()

/**
 * Sender prefix: groups name the sender ("你" for the caller); private chats and channels
 * never do; a system message (no sender id) never does.
 */
function senderPrefix(input: PreviewInput, message: ConversationLastMessage): string | null {
  if (input.chatType === 'private' || input.chatType === 'channel') return null
  if (message.sender_id === null) return null
  if (message.sender_id === input.currentUserId) return t('w.chatList.5630b8')
  return message.sender || null
}

export function buildChatPreview(input: PreviewInput): ChatPreview {
  if (input.typingText) return { kind: 'typing', text: input.typingText }
  const draft = singleLine(input.draftText)
  if (draft && !input.active) return { kind: 'draft', text: draft }
  const message = input.lastMessage
  if (!message)
    return { kind: 'empty', text: input.chatType === 'private' ? t('w.chatList.d3f20d') : t('w.chatList.30dfbc') }
  if (message.recalled)
    return { kind: 'message', sender: null, media: null, text: t('w.chatList.26b4ea'), recalled: true }
  const media = mediaFromFileName(message.attachment_file_name)
  const caption = singleLine(message.content)
  const text = caption || (media === 'file' ? (message.attachment_file_name ?? '') : media ? MEDIA_LABEL[media] : '')
  return { kind: 'message', sender: senderPrefix(input, message), media, text, recalled: false }
}
