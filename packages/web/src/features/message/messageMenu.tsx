/**
 * The message context menu as data. One builder serves the right-click / long-press menu
 * and the hover bar's "more" menu, so the two can never disagree about what is allowed.
 *
 * Rules: an absent action hides its row (permissions arrive as absent callbacks); a
 * recalled message only offers selection; a message not yet acknowledged by the server
 * (`sending` / `failed`) has no server id to reply to, forward, pin or react to.
 */
import type { MenuItem } from '@tg/ui'
import type { BroadcastMessage } from '@tg/core'
import type { MessageActions } from './types'
import { CopyGlyph, DeleteGlyph, EditGlyph, ForwardGlyph, PinGlyph, ReplyGlyph, SelectGlyph } from './icons'
import { t } from '../../i18n/index'

export interface MenuFacts {
  /** The server has acknowledged the message (it has a real id). */
  delivered: boolean
}

/**
 * A menu row contributed by another feature (TG-305: "保存 GIF"). It is offered only on a
 * live, delivered message and placed after 转发; return `null` to hide it for `message`.
 */
export type MessageMenuContribution = (message: BroadcastMessage) => MenuItem | null

const contributions = new Map<string, MessageMenuContribution>()

/** Adds or replaces the contribution `id`; returns an undo. */
export function registerMessageMenuItem(id: string, contribute: MessageMenuContribution): () => void {
  contributions.set(id, contribute)
  return () => {
    if (contributions.get(id) === contribute) contributions.delete(id)
  }
}

export function buildMessageMenu(message: BroadcastMessage, actions: MessageActions, facts: MenuFacts): MenuItem[] {
  const recalled = message.recalled_at !== null
  const live = !recalled && facts.delivered
  const items: MenuItem[] = []
  const add = (enabled: boolean, item: MenuItem) => {
    if (enabled) items.push(item)
  }

  add(live && actions.onReply !== undefined, {
    id: 'reply',
    label: t('w.message.ffc785'),
    icon: <ReplyGlyph />,
    onSelect: actions.onReply,
  })
  add(live && actions.onQuote !== undefined && message.content.trim() !== '', {
    id: 'quote',
    label: t('w.message.23c0e1'),
    icon: <ReplyGlyph />,
    onSelect: actions.onQuote,
  })
  add(live && actions.onReplyElsewhere !== undefined, {
    id: 'reply-elsewhere',
    label: t('w.message.b45d88'),
    icon: <ReplyGlyph />,
    onSelect: actions.onReplyElsewhere,
  })
  add(live && actions.onEdit !== undefined, {
    id: 'edit',
    label: t('w.message.a7f814'),
    icon: <EditGlyph />,
    onSelect: actions.onEdit,
  })
  add(!recalled && actions.onCopy !== undefined && message.content.trim() !== '', {
    id: 'copy',
    label: t('w.message.4641d2'),
    icon: <CopyGlyph />,
    onSelect: actions.onCopy,
  })
  add(live && actions.onPin !== undefined, {
    id: 'pin',
    label: t('w.message.7bcf18'),
    icon: <PinGlyph />,
    onSelect: actions.onPin,
  })
  add(live && actions.onForward !== undefined, {
    id: 'forward',
    label: t('w.message.0d5a8a'),
    icon: <ForwardGlyph />,
    onSelect: actions.onForward,
  })
  if (live) {
    for (const contribute of contributions.values()) {
      const item = contribute(message)
      if (item) items.push(item)
    }
  }
  add(actions.onSelect !== undefined, {
    id: 'select',
    label: t('w.message.70b208'),
    icon: <SelectGlyph />,
    onSelect: actions.onSelect,
  })
  add(!recalled && actions.onDelete !== undefined, {
    id: 'delete',
    label: t('w.message.3755f5'),
    icon: <DeleteGlyph />,
    danger: true,
    separatorBefore: items.length > 0,
    onSelect: actions.onDelete,
  })
  return items
}

/** Whether the hover bar may offer a reaction picker for this message. */
export function canReact(message: BroadcastMessage, actions: MessageActions, facts: MenuFacts): boolean {
  return message.recalled_at === null && facts.delivered && actions.onReact !== undefined
}
