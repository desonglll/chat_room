/**
 * The strip above the input: reply («回复 Alice»), edit («编辑消息») or forward
 * («转发 2 条消息»), with the quoted text and a ✕ that runs the state machine's `cancel`.
 * Which bar shows is `activeComposerBar` — this component only renders it.
 */
import type { ComposerBar as ComposerBarState } from '@tg/core'
import { messageStore } from '@tg/core'
import { useStore } from 'zustand/react'
import { CloseGlyph, EditBarGlyph, ForwardBarGlyph, ReplyBarGlyph } from './icons'
import { findMessage } from './composerController'

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
  return attachmentName ? `📎 ${attachmentName}` : '消息'
}

function useBarCopy(chatId: string, bar: ComposerBarState): { title: string; body: string } {
  const lookupChat = bar.kind === 'forward' ? bar.fromChatId : chatId
  const firstId = bar.kind === 'forward' ? (bar.messageIds[0] ?? '') : bar.kind === 'none' ? '' : bar.messageId
  // Subscribe to the one message so an incoming edit/recall updates the quote.
  const message = useStore(messageStore, () => (firstId ? findMessage(messageStore, lookupChat, firstId) : null))
  const body = message ? quote(message.content, message.attachment?.file_name) : '消息'
  switch (bar.kind) {
    case 'reply':
      return { title: message ? `回复 ${message.sender}` : '回复消息', body }
    case 'edit':
      return { title: '编辑消息', body }
    case 'forward': {
      const count = bar.messageIds.length
      return {
        title: count > 1 ? `转发 ${count} 条消息` : '转发消息',
        body: message ? `${message.sender}：${body}` : `${count} 条消息`,
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
      <button type="button" className="tg-compose__bar-close" aria-label="取消" title="取消 (Esc)" onClick={onCancel}>
        <CloseGlyph />
      </button>
    </div>
  )
}
