/**
 * TG-107: the one-line "who is doing what" text for a chat header or chat-list row.
 *
 * Pure: the caller passes the indicators, the clock reading and the viewer. Rules:
 * - every one of the nine TG-007 activity actions has its own copy (`cancel` is a clear,
 *   never displayed);
 * - an indicator counts only while `now - receivedAt < TYPING_TTL_MS`, so the text is gone
 *   within 5 s of the last frame even before the store purges it;
 * - private chats omit the name ("正在输入"); groups/channels name the actor:
 *   1 → "A 正在输入", 2 → "A 和 B 正在输入", 3+ → "A 和其他 N 人正在输入";
 * - several actors doing the SAME thing keep that action's copy; mixed actions fall back
 *   to the generic 正在输入 (Telegram Android's multi-user behaviour), because naming one
 *   person's action for the whole group would misdescribe the others.
 */
import type { ChatType, TypingAction } from '../types'
import { t } from '../i18n/t'

/** The nine displayable actions (TG-007's ten minus the `cancel` clear). */
export type ActiveTypingAction = Exclude<TypingAction, 'cancel'>

/** Receiver-side lifetime of one typing frame. Senders refresh faster (CHAT_ACTION_RESEND_MS). */
export const TYPING_TTL_MS = 5_000

export const TYPING_ACTION_COPY: Readonly<Record<ActiveTypingAction, string>> = {
  get typing() {
    return t('c.domain.86d1ab')
  },
  get recording_voice() {
    return t('c.domain.fc731e')
  },
  get recording_video_note() {
    return t('c.domain.8be901')
  },
  get uploading_photo() {
    return t('c.domain.2e09bd')
  },
  get uploading_video() {
    return t('c.domain.f7acdd')
  },
  get uploading_document() {
    return t('c.domain.3c3920')
  },
  get uploading_voice() {
    return t('c.domain.cad352')
  },
  get choosing_sticker() {
    return t('c.domain.8d9591')
  },
  get choosing_location() {
    return t('c.domain.0cc4ec')
  },
}

/** Name used when a frame arrived without a username (TG-007 omits it when absent). */
/** «有人» in the current language (a function: the language can change at runtime). */
export const unknownActorName = () => t('c.domain.someone')

export interface TypingActor {
  user_id: string
  username: string
  action: ActiveTypingAction
  receivedAt: number
}

export interface TypingSummaryInput {
  /** In display order (first-seen first); presenceStore keeps that order stable. */
  indicators: readonly TypingActor[]
  now: number
  chatType: ChatType
  /** The viewer never sees their own activity. */
  currentUserId: string
}

export interface TypingSummary {
  text: string
  /** The shared action when all actors agree, else `typing`; drives the indicator glyph. */
  action: ActiveTypingAction
  count: number
}

export function isTypingLive(indicator: Pick<TypingActor, 'receivedAt'>, now: number): boolean {
  const age = now - indicator.receivedAt
  // A frame stamped in the future (clock step backwards) is treated as fresh.
  return age < TYPING_TTL_MS
}

export function summarizeTyping(input: TypingSummaryInput): TypingSummary | null {
  const actors = input.indicators.filter(
    (indicator) => indicator.user_id !== input.currentUserId && isTypingLive(indicator, input.now),
  )
  const first = actors[0]
  if (!first) return null
  const action = actors.every((actor) => actor.action === first.action) ? first.action : 'typing'
  const verb = TYPING_ACTION_COPY[action]
  if (input.chatType === 'private') return { text: verb, action, count: actors.length }
  const name = (actor: TypingActor) => actor.username.trim() || unknownActorName()
  const second = actors[1]
  // Chinese spacing: a space separates a name from the verb, but "其他 N 人" is already a
  // counted phrase and joins the verb directly ("A 和其他 2 人正在输入").
  let text: string
  if (!second) text = `${name(first)} ${verb}`
  else if (actors.length === 2) text = t('c.domain.2fe61d', name(first), name(second), verb)
  else text = t('c.domain.f6ec68', name(first), actors.length - 1, verb)
  return { text, action, count: actors.length }
}
