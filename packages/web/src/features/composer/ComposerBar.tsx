/**
 * The strip above the input: reply («回复 Alice»), edit («编辑消息») or forward
 * («转发 2 条消息»), with the quoted text and a ✕ that runs the state machine's `cancel`.
 * Which bar shows is `activeComposerBar` — this component only renders it.
 */
import type { ComposerBar as ComposerBarState } from '@tg/core'
import { composerStore, messageStore } from '@tg/core'
import { useStore } from 'zustand/react'
import { CloseGlyph, EditBarGlyph, ForwardBarGlyph, ReplyBarGlyph } from './icons'
import { findMessage } from './composerController'
import { t } from '../../i18n/index'

export interface ComposerBarProps {
  chatId: string
  bar: ComposerBarState
  onCancel(): void
}

/** One-line quote: markdown markers and newlines flattened, attachments named. */
function quote(content: string, attachmentName: string | null | undefined): string {
  const text = content
    .replace(/[*_~`]|<\/?u>/g, '')
    .replace(/\s+/g, ' ')
    .trim()
  if (text) return text
  return attachmentName ? `📎 ${attachmentName}` : t('w.composer.dc6de3')
}

function useBarCopy(chatId: string, bar: ComposerBarState): { title: string; body: string } {
  const lookupChat = bar.kind === 'forward' ? bar.fromChatId : chatId
  const firstId = bar.kind === 'forward' ? (bar.messageIds[0] ?? '') : bar.kind === 'none' ? '' : bar.messageId
  // Subscribe to the one message so an incoming edit/recall updates the quote.
  const message = useStore(messageStore, () => (firstId ? findMessage(messageStore, lookupChat, firstId) : null))
  const body = message ? quote(message.content, message.attachment?.file_name) : t('w.composer.dc6de3')
  // TG-409: a quote replaces the body; a cross-chat reply shows its source snapshot (the
  // message is not in this chat's timeline).
  const extras = useStore(composerStore, (state) => state.replyExtras[chatId])
  switch (bar.kind) {
    case 'reply':
      if (extras?.source) {
        return {
          title: t('w.composer.9fa890', extras.source.sender, extras.source.chatTitle || t('w.composer.otherChat')),
          body: extras.quote ? `「${extras.quote.text}」` : quote(extras.source.text, null),
        }
      }
      if (extras?.quote)
        return {
          title: message ? t('w.composer.6ef258', message.sender) : t('w.composer.23c0e1'),
          body: `「${extras.quote.text}」`,
        }
      return { title: message ? t('w.composer.17f37e', message.sender) : t('w.composer.1f0d18'), body }
    case 'edit':
      return { title: t('w.composer.8a8de1'), body }
    case 'forward': {
      const count = bar.messageIds.length
      return {
        title: count > 1 ? t('w.composer.eb5a89', count) : t('w.composer.d646f7'),
        body: message ? `${message.sender}：${body}` : t('w.composer.8ab7d9', count),
      }
    }
    default:
      return { title: '', body: '' }
  }
}

const GLYPHS = { reply: ReplyBarGlyph, edit: EditBarGlyph, forward: ForwardBarGlyph } as const

export function ComposerBar({ chatId, bar, onCancel }: ComposerBarProps) {
  const copy = useBarCopy(chatId, bar)
  if (bar.kind === 'none') return null
  const Glyph = GLYPHS[bar.kind]
  return (
    <div className="tg-compose__bar" data-kind={bar.kind}>
      <span className="tg-compose__bar-icon">
        <Glyph />
      </span>
      <span className="tg-compose__bar-text">
        <span className="tg-compose__bar-title">{copy.title}</span>
        <span className="tg-compose__bar-body">{copy.body}</span>
      </span>
      <button
        type="button"
        className="tg-compose__bar-close"
        aria-label={t('w.composer.4d0b46')}
        title={t('w.composer.12f3a1')}
        onClick={onCancel}
      >
        <CloseGlyph />
      </button>
    </div>
  )
}
