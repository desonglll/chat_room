/**
 * The chat session's public types (options, stores, topic mode, the session surface), split
 * from `chatSession.ts` when it crossed the 350-line gate. `chatSession.ts` re-exports them,
 * so importers keep one import path.
 */
import type {
  ApiClient,
  BroadcastFrame,
  ChatListStore,
  ChatSocketStatus,
  ClientFrame,
  ComposerStore,
  CoreClock,
  CoreSocketFactory,
  DraftsApi,
  MessageEntity,
  MessageStore,
  PresenceStore,
  ServerFrame,
  StoredMessage,
} from '@tg/core'
import type { ChannelStore } from '../channel/channelStore'
import type { PollStore } from '../poll/pollStore'

/** Server cap: message ≤ 4096 chars (`src/realtime/auth.rs`). */
export const MAX_MESSAGE_CHARS = 4096

export interface ChatSessionStores {
  message: MessageStore
  presence: PresenceStore
  composer: ComposerStore
  chatList: ChatListStore
  /** Live poll tallies (TG-406). `poll_updated` frames are dropped when absent. */
  poll?: PollStore | undefined
  /** Live channel view counts (TG-202). `message_views_updated` frames are dropped when absent. */
  channel?: ChannelStore | undefined
}

/**
 * TG-204 forum topic mode (built by `features/forum/topicSessionMode`): the timeline stays
 * keyed by chat id but shows one topic — foreign broadcasts are dropped, history and
 * catch-up come from the topic endpoints, reads go to the topic cursor.
 */
export interface ChatSessionTopicMode {
  /** The WS `message` frame's `topic_id`; null for General (omitted on the wire). */
  sendTopicId: string | null
  accepts(frame: BroadcastFrame): boolean
  /** The topic's newest page, in `listChatMessages` order. */
  latest(): Promise<StoredMessage[]>
  /** Advance the topic read cursor (replaces the chat-level `read` frame). */
  read(messageId: string): void
}

export interface ChatSessionOptions {
  chatId: string
  token: string
  currentUserId: string
  socketUrl: string
  createSocket: CoreSocketFactory
  clock: CoreClock
  client: ApiClient
  draftsApi: DraftsApi
  stores: ChatSessionStores
  /** Whether the reader can see the chat right now (page visible). Default: always. */
  isVisible?: () => boolean
  topic?: ChatSessionTopicMode | null | undefined
  /** False: never send the chat-level `read` frame (a forum chat reads per topic). Default true. */
  readCursor?: boolean | undefined
}

/** TG-404: per-send options; `silent` delivers without notifications. */
export interface SendMessageOptions {
  silent?: boolean
  /** TG-408: the sender dismissed the link card in the composer. */
  noLinkPreview?: boolean
  /** TG-1206: custom emoji ranges of the (already trimmed) text; omitted from the frame when empty. */
  entities?: MessageEntity[]
}

export interface ChatSession {
  start(): void
  /** Flushes a pending draft save, closes the socket, detaches every subscription. */
  stop(): void
  /**
   * Optimistic append + WS send; false marks the row failed (offline). TG-404: `silent`.
   * TG-1208: while the chat is still opening (connecting, or replaying history) the send is
   * parked and goes out after the replay, in order — it returns true.
   */
  sendMessage(text: string, options?: SendMessageOptions): boolean
  /**
   * Composer edit: store + debounced cloud save. Typing frames are the composer's
   * (TG-107 `createChatActionSender` via `sendFrame`), not this method's.
   */
  setDraftText(text: string): void
  /** One raw client frame on this chat's socket (edit, recall, reaction, typing …). */
  sendFrame(frame: ClientFrame): boolean
  /**
   * Advance the viewer's read cursor to the newest server message, when it moved. Called
   * on history completion, on every settled arrival, and by the host when the page
   * becomes visible; `isVisible` (option) gates it so a background tab reads nothing.
   */
  markRead(): void
  status(): ChatSocketStatus
  onStatus(handler: (status: ChatSocketStatus) => void): () => void
  /** TG-1208: whether a send would go out (or be parked) now; the composer's gate. */
  sendable(): boolean
  onSendable(handler: (sendable: boolean) => void): () => void
  /** Every server frame, after the store fan-out (TG-204's topic list refreshes on them). */
  onFrame(handler: (frame: ServerFrame) => void): () => void
}
