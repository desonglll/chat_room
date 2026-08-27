import type { DownloadProgress } from './attachmentDownloads'
import type {
  Attachment,
  AttachmentUploadSession,
  ChatStatus,
  ConversationSummary,
  DisplayMessage,
  FocusShortcut,
  ReadReceipt,
  Room,
  RoomMember,
  SendShortcut,
  SocialUser,
  TypingDraft,
  User,
} from './types'

export interface ChatPanelProps {
  room: Room | null
  conversation: ConversationSummary | null
  user: User | null
  password: string
  rememberRoomPasswords: boolean
  contact: SocialUser | null
  setFriendRemark: (userId: string, remark: string) => Promise<void>
  token: string
  status: ChatStatus
  statusLabel: string
  authenticated: boolean
  historyReady: boolean
  error: string
  messages: DisplayMessage[]
  favoriteMessageIds: string[]
  members: RoomMember[]
  participants: RoomMember[]
  readReceipts: ReadReceipt[]
  currentUserId: string
  visible: boolean
  sendShortcut: SendShortcut
  focusShortcut: FocusShortcut
  typingDrafts: TypingDraft[]
  downloading: boolean
  downloadProgress: DownloadProgress | null
  maxUploadBytes: number
  pokedAt: number
  loadingOlder: boolean
  hasMoreHistory: boolean
  pendingUploads: AttachmentUploadSession[]
  aiEnabled: boolean
  aiPanelOpen: boolean
  loading: boolean
  ensureMessage: (messageId: string) => Promise<boolean>
}

export type ChatPanelEmits = {
  back: []
  manage: []
  leave: []
  join: []
  requestJoin: []
  authenticate: []
  send: [content: string, replyTo: string]
  read: [messageId: string]
  upload: [files: File[], content: string, replyTo: string, isSensitive: boolean]
  resumeUpload: [session: AttachmentUploadSession, file: File]
  cancelUpload: [session: AttachmentUploadSession]
  cancelUploadTask: [key: string]
  retryUploadTask: [key: string]
  recall: [messageId: string]
  edit: [messageId: string, content: string]
  forward: [messageIds: string[]]
  favorite: [messageIds: string[]]
  typing: [content: string]
  download: [attachments: Attachment[]]
  cancelDownload: []
  poke: [userId: string]
  retry: [messageId: string]
  reaction: [messageId: string, emoji: string, active: boolean]
  loadOlder: []
  removeFriend: []
  blockUser: []
  assistant: [messageIds?: string[]]
  catchUp: []
  'update:password': [password: string]
  'update:rememberRoomPasswords': [remember: boolean]
}
