/**
 * Dev fixture page: every bubble state from `bubbleFixtures`, in day or night theme, with
 * live actions (react, select, jump) so the interactive states can be poked by hand.
 */
import { useState } from 'react'
import type { BroadcastMessage } from '@tg/core'
import { Button } from '@tg/ui'
import { MessageBubble } from '../MessageBubble'
import type { MessageActions } from '../types'
import { bubbleFixtures, VIEWER } from './bubbleFixtures'

const FIXTURES = bubbleFixtures()

function toggleReaction(message: BroadcastMessage, emoji: string): BroadcastMessage {
  const existing = message.reactions.find((reaction) => reaction.emoji === emoji)
  const others = message.reactions.filter((reaction) => reaction.emoji !== emoji)
  if (existing === undefined) return { ...message, reactions: [...others, { emoji, user_ids: [VIEWER] }] }
  const users = existing.user_ids.includes(VIEWER)
    ? existing.user_ids.filter((id) => id !== VIEWER)
    : [...existing.user_ids, VIEWER]
  const next =
    users.length === 0 ? others : message.reactions.map((r) => (r.emoji === emoji ? { emoji, user_ids: users } : r))
  return { ...message, reactions: next }
}

export function FixtureGallery() {
  const [theme, setTheme] = useState<'day' | 'night'>('day')
  const [messages, setMessages] = useState(() => new Map(FIXTURES.map((f) => [f.id, f.message])))
  const [selected, setSelected] = useState<ReadonlySet<string>>(new Set(['out-selected']))
  const [log, setLog] = useState('')

  const switchTheme = () => {
    const next = theme === 'day' ? 'night' : 'day'
    document.documentElement.setAttribute('data-tg-theme', next)
    setTheme(next)
  }

  const actionsFor = (id: string): MessageActions => {
    const say = (what: string) => () => setLog(`${id}: ${what}`)
    return {
      onReply: say('reply'),
      onEdit: say('edit'),
      onDelete: say('delete'),
      onForward: say('forward'),
      onPin: say('pin'),
      onCopy: say('copy'),
      onJumpTo: (target) => setLog(`${id}: jump to ${target}`),
      onOpenMedia: (attachment) => setLog(`${id}: open media ${attachment}`),
      onReact: (emoji) =>
        setMessages((current) => {
          const message = current.get(id)
          if (message?.type !== 'broadcast') return current
          return new Map(current).set(id, toggleReaction(message, emoji))
        }),
      onSelect: () =>
        setSelected((current) => {
          const next = new Set(current)
          if (next.has(id)) next.delete(id)
          else next.add(id)
          return next
        }),
    }
  }

  return (
    <>
      <div className="fx-toolbar">
        <strong>TG-103 bubbles</strong>
        <Button variant="tonal" size="sm" onClick={switchTheme}>
          {theme === 'day' ? '切换到夜间' : '切换到日间'}
        </Button>
        <span className="fx-log" role="status">
          {log}
        </span>
      </div>
      <div className="fx-list">
        {FIXTURES.map((fixture) => (
          <section key={fixture.id} data-fixture={fixture.id}>
            {fixture.ctx.groupPosition === 'middle' || fixture.ctx.groupPosition === 'last' ? null : (
              <p className="fx-label">{fixture.title}</p>
            )}
            <MessageBubble
              message={messages.get(fixture.id) ?? fixture.message}
              ctx={{ ...fixture.ctx, selected: selected.has(fixture.id) }}
              actions={actionsFor(fixture.id)}
              viewerId={VIEWER}
              delivery={fixture.delivery}
              selectionMode={fixture.selectionMode}
              reserveAvatar={fixture.reserveAvatar}
            />
          </section>
        ))}
      </div>
    </>
  )
}
