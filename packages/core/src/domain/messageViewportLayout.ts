/**
 * Message-list layout (TG-101): which rows start a new calendar day and how consecutive
 * messages group into Telegram "runs" (same sender, no service row between, same day,
 * each gap within `groupWindowMs`). Pure and platform-free so it can be benchmarked over
 * 100k messages and reused by a future React Native list.
 *
 * Contract with the view: the entry at index i describes messages[i]. The first entry's
 * `startsDay` / `groupPosition` depend on a predecessor that may not be loaded yet; the
 * view keeps one loaded-but-hidden "lead" message above the first rendered row precisely
 * so that prepending older history never changes the layout of a row already on screen.
 *
 * `createMessageLayoutCache` makes re-layout incremental: it diffs the new message array
 * against the previous one by object identity (common prefix + common suffix) and only
 * recomputes the changed middle plus the neighbours whose group position can depend on it,
 * so an append, a prepended page, or an edited message costs O(changed) layout work plus
 * one O(n) pointer comparison instead of re-parsing every timestamp.
 */
import type { DisplayMessage } from './messageView'

export type MessageGroupPosition = 'single' | 'first' | 'middle' | 'last'

export interface MessageLayoutOptions {
  currentUserId: string
  /** Group/supergroup chats show sender names and avatars on incoming runs. */
  showGroupIdentity: boolean
  /**
   * TG-904: a broadcast channel. Telegram draws every post on the incoming side — the
   * channel speaks, not the admin who typed it — so no post is "outgoing" (no right side,
   * no read ticks), the publishing admin's own included.
   */
  channel?: boolean
  /** Maximum gap between two messages of one run. Telegram-like default: 5 minutes. */
  groupWindowMs?: number
  /** Local calendar day of an epoch-ms instant; injectable for deterministic tests. */
  dayKeyOf?: (epochMs: number) => number
}

export interface MessageLayoutEntry {
  /** Stable React key: survives optimistic → acknowledged reconciliation. */
  key: string
  /** Local calendar day number; NaN for rows without a timestamp (system rows). */
  day: number
  /** Epoch ms of the message; NaN when unknown. */
  timeMs: number
  /** Continues the run of the row above (implied by `groupPosition`, kept for re-layout). */
  joinsPrevious: boolean
  /** A date separator belongs directly above this row. */
  startsDay: boolean
  groupPosition: MessageGroupPosition
  isOutgoing: boolean
  showAvatar: boolean
  showSenderName: boolean
}

export const DEFAULT_GROUP_WINDOW_MS = 5 * 60 * 1000

const DAY_MS = 86_400_000
const HOUR_MS = 3_600_000

// Timezone offsets only change on the hour (DST) in every real zone, and messages arrive
// in chronological order, so one remembered hour bucket removes almost every `Date`.
let offsetBucket = Number.NaN
let offsetMs = 0

/** Local-time day number from the ECMAScript timezone offset of that instant; no Intl. */
export function localDayKey(epochMs: number): number {
  const bucket = Math.floor(epochMs / HOUR_MS)
  if (bucket !== offsetBucket) {
    offsetBucket = bucket
    offsetMs = new Date(epochMs).getTimezoneOffset() * 60_000
  }
  return Math.floor((epochMs - offsetMs) / DAY_MS)
}

export function messageKey(message: DisplayMessage): string {
  if (message.type === 'broadcast') return message.client_message_id || message.message_id
  return message.key
}

/** The run identity of a row, or '' for rows that never group (service rows). */
function senderOf(message: DisplayMessage, currentUserId: string): string {
  if (message.type === 'broadcast') return message.sender_id ?? `name:${message.sender}`
  if (message.type === 'upload') return currentUserId
  return ''
}

function isOutgoingMessage(message: DisplayMessage, currentUserId: string): boolean {
  if (message.type === 'upload') return true
  return message.type === 'broadcast' && message.sender_id !== null && message.sender_id === currentUserId
}

interface Settings {
  currentUserId: string
  showGroupIdentity: boolean
  channel: boolean
  windowMs: number
  dayOf: (epochMs: number) => number
}

function settingsOf(options: MessageLayoutOptions): Settings {
  return {
    currentUserId: options.currentUserId,
    showGroupIdentity: options.showGroupIdentity,
    channel: options.channel ?? false,
    windowMs: options.groupWindowMs ?? DEFAULT_GROUP_WINDOW_MS,
    dayOf: options.dayKeyOf ?? localDayKey,
  }
}

/** Everything about row i that depends only on rows i-1 and i. Position fields are filled later. */
function baseEntry(
  message: DisplayMessage,
  previous: DisplayMessage | undefined,
  previousEntry: MessageLayoutEntry | undefined,
  settings: Settings,
): MessageLayoutEntry {
  const ms = message.type === 'system' ? Number.NaN : Date.parse(message.timestamp)
  const ownDay = Number.isNaN(ms) ? Number.NaN : settings.dayOf(ms)
  const previousDay = previousEntry ? previousEntry.day : Number.NaN
  // A row without a readable time inherits the running day: it never opens a new one.
  const day = Number.isNaN(ownDay) ? previousDay : ownDay
  const startsDay = !Number.isNaN(day) && (previousEntry === undefined || day !== previousDay)
  const sender = senderOf(message, settings.currentUserId)
  const previousMs = previousEntry ? previousEntry.timeMs : Number.NaN
  const joinsPrevious =
    previous !== undefined &&
    sender !== '' &&
    sender === senderOf(previous, settings.currentUserId) &&
    !startsDay &&
    !Number.isNaN(ms) &&
    !Number.isNaN(previousMs) &&
    Math.abs(ms - previousMs) <= settings.windowMs
  return {
    key: messageKey(message),
    day,
    timeMs: ms,
    joinsPrevious,
    startsDay,
    groupPosition: 'single',
    isOutgoing: !settings.channel && isOutgoingMessage(message, settings.currentUserId),
    showAvatar: false,
    showSenderName: false,
  }
}

function withPosition(
  entry: MessageLayoutEntry,
  message: DisplayMessage,
  continued: boolean,
  settings: Settings,
): MessageLayoutEntry {
  const joined = entry.joinsPrevious
  const groupPosition: MessageGroupPosition = joined ? (continued ? 'middle' : 'last') : continued ? 'first' : 'single'
  const identity = settings.showGroupIdentity && !entry.isOutgoing && message.type === 'broadcast'
  const showSenderName = identity && !joined
  const showAvatar = identity && !continued
  if (
    entry.groupPosition === groupPosition &&
    entry.showSenderName === showSenderName &&
    entry.showAvatar === showAvatar
  ) {
    return entry
  }
  return { ...entry, groupPosition, showSenderName, showAvatar }
}

function sameBase(left: MessageLayoutEntry, right: MessageLayoutEntry): boolean {
  return (
    left.key === right.key &&
    left.joinsPrevious === right.joinsPrevious &&
    left.startsDay === right.startsDay &&
    (left.day === right.day || (Number.isNaN(left.day) && Number.isNaN(right.day)))
  )
}

interface PreviousLayout {
  messages: readonly DisplayMessage[]
  entries: readonly MessageLayoutEntry[]
}

function layoutFrom(
  messages: readonly DisplayMessage[],
  settings: Settings,
  previous: PreviousLayout | null,
): MessageLayoutEntry[] {
  const count = messages.length
  const oldMessages = previous?.messages ?? []
  const oldEntries = previous?.entries ?? []
  const oldCount = oldMessages.length
  const limit = Math.min(count, oldCount)
  let prefix = 0
  while (prefix < limit && messages[prefix] === oldMessages[prefix]) prefix += 1
  let suffix = 0
  while (suffix < limit - prefix && messages[count - 1 - suffix] === oldMessages[oldCount - 1 - suffix]) {
    suffix += 1
  }
  const shift = oldCount - count // new index i ↔ old index i + shift inside the suffix

  const entries: MessageLayoutEntry[] = oldEntries.slice(0, prefix)
  entries.length = count
  let stop = count
  for (let index = prefix; index < count; index += 1) {
    const entry = baseEntry(messages[index] as DisplayMessage, messages[index - 1], entries[index - 1], settings)
    if (index >= count - suffix) {
      const old = oldEntries[index + shift] as MessageLayoutEntry
      if (sameBase(old, entry)) {
        stop = index
        break
      }
    }
    entries[index] = entry
  }
  for (let index = stop; index < count; index += 1) entries[index] = oldEntries[index + shift] as MessageLayoutEntry

  // Positions: every recomputed row, plus the row above them (its successor changed).
  const from = Math.max(0, prefix - 1)
  const to = Math.min(count - 1, stop)
  for (let index = from; index <= to; index += 1) {
    const continued = index + 1 < count && (entries[index + 1] as MessageLayoutEntry).joinsPrevious
    entries[index] = withPosition(
      entries[index] as MessageLayoutEntry,
      messages[index] as DisplayMessage,
      continued,
      settings,
    )
  }
  return entries
}

/** Full, stateless layout of `messages`. */
export function computeMessageLayout(
  messages: readonly DisplayMessage[],
  options: MessageLayoutOptions,
): MessageLayoutEntry[] {
  return layoutFrom(messages, settingsOf(options), null)
}

export type MessageLayoutCache = (
  messages: readonly DisplayMessage[],
  options: MessageLayoutOptions,
) => MessageLayoutEntry[]

/**
 * A memoizing layout function for one list instance. Unchanged entries keep their object
 * identity across calls, so memoized rows skip re-rendering.
 */
export function createMessageLayoutCache(): MessageLayoutCache {
  let previous: PreviousLayout | null = null
  let previousSettings: Settings | null = null
  return (messages, options) => {
    const settings = settingsOf(options)
    const reusable =
      previousSettings !== null &&
      previousSettings.currentUserId === settings.currentUserId &&
      previousSettings.showGroupIdentity === settings.showGroupIdentity &&
      previousSettings.channel === settings.channel &&
      previousSettings.windowMs === settings.windowMs &&
      previousSettings.dayOf === settings.dayOf
    if (previous && previous.messages === messages && reusable) return previous.entries as MessageLayoutEntry[]
    const entries = layoutFrom(messages, settings, reusable ? previous : null)
    previous = { messages, entries }
    previousSettings = settings
    return entries
  }
}
