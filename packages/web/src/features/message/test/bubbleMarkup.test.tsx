/**
 * Markup assertions over server-rendered bubbles (the packages/web pattern: `bun test` has
 * no DOM). Covers the card's matrix in/out × text/image/reply/forward/edited, the tail rule,
 * and the no-overlap spacer contract.
 */
import { describe, expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import type { DisplayMessage } from '@tg/core'
import { MessageBubble } from '../MessageBubble'
import { registerMessageContent } from '../content/messageContent'
import type { MessageContentProps } from '../content/contentTypes'
import { bubbleFixtures, makeCtx, makeMessage, photo, VIEWER } from '../fixtures/bubbleFixtures'
import type { MessageActions, MessageRenderContext } from '../types'

const ACTIONS: MessageActions = {
  onReply: () => {},
  onReact: () => {},
  onDelete: () => {},
  onSelect: () => {},
  onJumpTo: () => {},
}

function render(message: DisplayMessage, ctx: Partial<MessageRenderContext> = {}, extra: object = {}) {
  return renderToStaticMarkup(
    <MessageBubble message={message} ctx={makeCtx(ctx)} actions={ACTIONS} viewerId={VIEWER} {...extra} />,
  )
}

const count = (html: string, needle: string) => html.split(needle).length - 1

/** The inner HTML of the first `<span class="{cls}"…>…</span>`, balancing nested spans. */
function spanAt(html: string, cls: string): { inner: string; after: string } | null {
  const open = html.indexOf(`<span class="${cls}"`)
  if (open < 0) return null
  const start = html.indexOf('>', open) + 1
  let depth = 1
  let cursor = start
  while (depth > 0) {
    const nextOpen = html.indexOf('<span', cursor)
    const nextClose = html.indexOf('</span>', cursor)
    if (nextOpen >= 0 && nextOpen < nextClose) {
      depth += 1
      cursor = nextOpen + 5
    } else {
      depth -= 1
      cursor = nextClose + 7
    }
  }
  return { inner: html.slice(start, cursor - 7), after: html.slice(cursor) }
}

/**
 * The no-overlap contract, as markup: the invisible spacer and the visible meta render the
 * same content (so they are the same width), and the spacer is the LAST thing in its block
 * (nothing can flow after it and under the absolutely positioned meta).
 */
function expectSpacerContract(html: string) {
  const meta = spanAt(html, 'tg-bubble__meta')
  const spacer = spanAt(html, 'tg-bubble__meta-spacer')
  expect(meta).not.toBeNull()
  expect(spacer).not.toBeNull()
  expect(spacer?.inner).toBe(meta?.inner)
  expect(spacer?.after).toMatch(/^<\/(div|span)>/)
  expect(count(html, 'tg-bubble__meta-spacer')).toBe(1)
  expect(html).toContain('class="tg-bubble__meta-spacer" aria-hidden="true"')
}

describe('the in/out × kind matrix', () => {
  for (const isOutgoing of [false, true]) {
    const side = isOutgoing ? 'out' : 'in'
    const sender = isOutgoing ? { sender_id: VIEWER, sender: '我' } : {}

    test(`${side} · text`, () => {
      const html = render(makeMessage({ ...sender, content: '你好' }), { isOutgoing }, { delivery: 'read' })
      expect(html).toContain(`data-side="${side}"`)
      expect(html).toContain('data-frame="bubble"')
      expect(count(html, 'class="tg-bubble__tail"')).toBe(1)
      expectSpacerContract(html)
      // Ticks only ever on your own messages.
      expect(count(html, 'data-delivery="read"')).toBe(isOutgoing ? 2 : 0)
    })

    test(`${side} · image (borderless)`, () => {
      const html = render(makeMessage({ ...sender, content: '', attachment: photo('a1', 'p') }), { isOutgoing })
      expect(html).toContain('data-frame="media"')
      expect(html).not.toContain('class="tg-bubble__tail"')
      expect(html).toContain('data-flush-top=""')
      expect(html).toContain('data-flush-bottom=""')
      expect(html).toContain('class="tg-bubble__meta" data-overlay=""')
      expect(html).not.toContain('tg-bubble__meta-spacer')
      expect(html).toContain('class="tg-bubble__image"')
    })

    test(`${side} · image + caption`, () => {
      const html = render(makeMessage({ ...sender, content: '日落', attachment: photo('a1', 'p') }), { isOutgoing })
      expect(html).toContain('data-frame="bubble"')
      expect(html).toContain('tg-bubble__caption')
      expect(html).toContain('data-flush-top=""')
      expect(html).not.toContain('data-flush-bottom')
      expectSpacerContract(html)
    })

    test(`${side} · reply`, () => {
      const reply = { message_id: 'm-x', sender: 'Bob', content: '三点？', attachment_file_name: null, recalled: false }
      const html = render(makeMessage({ ...sender, content: '好', reply_to: reply }), { isOutgoing })
      expect(html).toContain('class="tg-bubble__reply"')
      expect(html).toContain('aria-label="跳转到 Bob 的消息"')
      expect(html).toContain('三点？')
      expectSpacerContract(html)
    })

    test(`${side} · forwarded`, () => {
      const from = { sender: 'Carol', room_name: '产品组' }
      const html = render(makeMessage({ ...sender, content: '公告', forwarded_from: from }), { isOutgoing })
      expect(html).toContain('转发自')
      expect(html).toContain('Carol')
      expect(html).toContain('产品组')
      expectSpacerContract(html)
    })

    test(`${side} · edited`, () => {
      const html = render(makeMessage({ ...sender, content: '改', edited_at: '2026-09-30T10:00:00Z' }), { isOutgoing })
      // Once in the visible meta, once in the spacer — so the spacer grows with the marker.
      expect(count(html, '已编辑</span>')).toBe(2)
      expectSpacerContract(html)
    })
  }
})

describe('no-overlap spacer across every fixture', () => {
  for (const fixture of bubbleFixtures()) {
    if (fixture.message.type !== 'broadcast') continue
    test(fixture.id, () => {
      const html = render(fixture.message, fixture.ctx, {
        delivery: fixture.delivery,
        selectionMode: fixture.selectionMode,
      })
      expect(count(html, 'class="tg-bubble__meta"')).toBe(1)
      if (html.includes('data-overlay=""')) expect(html).not.toContain('tg-bubble__meta-spacer')
      else expectSpacerContract(html)
    })
  }

  test('with reactions the spacer rides the reaction row, not the text', () => {
    const message = makeMessage({ content: '聚餐', reactions: [{ emoji: '👍', user_ids: [VIEWER, 'u-2'] }] })
    const html = render(message)
    expectSpacerContract(html)
    const row = html.slice(html.indexOf('class="tg-bubble__reactions"'))
    expect(row).toContain('tg-bubble__meta-spacer')
    expect(html).toContain('aria-pressed="true"')
    expect(html).toContain('aria-label="👍 2 人"')
  })
})

describe('group shape', () => {
  test('only the last message of a group has a tail', () => {
    const tails = (['first', 'middle', 'last', 'single'] as const).map((groupPosition) =>
      count(render(makeMessage(), { groupPosition }), 'class="tg-bubble__tail"'),
    )
    expect(tails).toEqual([0, 0, 1, 1])
  })

  test('corner attributes follow the grammar for an incoming middle message', () => {
    const html = render(makeMessage(), { groupPosition: 'middle' })
    expect(html).toContain('data-tl="merged" data-tr="full" data-br="full" data-bl="merged"')
  })

  test('avatar: shown, reserved, or absent; never on outgoing', () => {
    expect(render(makeMessage(), { showAvatar: true })).toContain('tg-avatar')
    const reserved = render(makeMessage(), {}, { reserveAvatar: true })
    expect(reserved).toContain('class="tg-message__avatar"></div>')
    expect(render(makeMessage(), { isOutgoing: true, showAvatar: true })).not.toContain('tg-message__avatar')
  })

  test('sender name only on incoming with showSenderName', () => {
    expect(render(makeMessage(), { showSenderName: true })).toContain('class="tg-bubble__sender">Alice<')
    expect(render(makeMessage(), { showSenderName: true, isOutgoing: true })).not.toContain('tg-bubble__sender')
  })
})

describe('states', () => {
  test('system message is a centred service pill, not a bubble', () => {
    const html = render({ type: 'system', key: 's', content: 'Bob 加入了群组' })
    expect(html).toContain('tg-message--service')
    expect(html).toContain('class="tg-service-pill">Bob 加入了群组<')
    expect(html).not.toContain('tg-bubble')
  })

  test('deleted placeholder drops quote, forward and reactions', () => {
    const html = render(
      makeMessage({
        recalled_at: '2026-09-30T00:00:00Z',
        reply_to: { message_id: 'x', sender: 'B', content: 'c', attachment_file_name: null, recalled: false },
        forwarded_from: { sender: 'C', room_name: '' },
        reactions: [{ emoji: '👍', user_ids: ['u'] }],
      }),
    )
    expect(html).toContain('消息已撤回')
    expect(html).not.toContain('tg-bubble__reply')
    expect(html).not.toContain('tg-bubble__forward')
    expect(html).not.toContain('tg-bubble__reactions')
    expectSpacerContract(html)
  })

  test('hover bar offers react / reply / more when actions allow', () => {
    const html = render(makeMessage())
    expect(html).toContain('class="tg-bubble__actions"')
    expect(html).toContain('aria-label="添加回应"')
    expect(html).toContain('aria-label="回复"')
    expect(html).toContain('aria-label="更多操作"')
    expect(renderToStaticMarkup(<MessageBubble message={makeMessage()} ctx={makeCtx()} />)).not.toContain(
      'tg-bubble__actions',
    )
  })

  test('selection mode: checkbox on every row, no hover bar', () => {
    const html = render(makeMessage(), { selected: true }, { selectionMode: true })
    expect(html).toContain('data-selection-mode=""')
    expect(html).toContain('type="checkbox"')
    expect(html).toContain('checked=""')
    expect(html).toContain('aria-label="取消选择消息"')
    expect(html).not.toContain('tg-bubble__actions')
  })

  test('highlighted row carries the flash attribute', () => {
    expect(render(makeMessage(), { highlighted: true })).toContain('data-highlighted=""')
  })

  test('sending and failed show the clock and the danger mark', () => {
    const mine = { sender_id: VIEWER, sender: '我' }
    expect(render(makeMessage({ ...mine, delivery_state: 'sending' }), { isOutgoing: true })).toContain(
      'data-delivery="sending"',
    )
    expect(render(makeMessage({ ...mine, delivery_state: 'failed' }), { isOutgoing: true })).toContain(
      'data-delivery="failed"',
    )
  })

  test('upload placeholder shows progress', () => {
    const upload = bubbleFixtures().find((f) => f.id === 'out-upload')
    const html = render(upload!.message, { isOutgoing: true })
    expect(html).toContain('上传中 40%')
    expect(html).toContain('inline-size:40%')
  })
})

describe('content registry plugs into the bubble', () => {
  test('a registered kind renders inside the full bubble and receives the spacer', () => {
    function PollContent({ metaSpacer }: MessageContentProps) {
      return (
        <div className="tg-bubble__text">
          <span>poll-body</span>
          {metaSpacer}
        </div>
      )
    }
    const undo = registerMessageContent('poll', PollContent, { match: (m) => m.content === '#poll', priority: 50 })
    try {
      const html = render(makeMessage({ content: '#poll' }), { showSenderName: true })
      expect(html).toContain('poll-body')
      expect(html).toContain('tg-bubble__sender')
      expectSpacerContract(html)
    } finally {
      undo()
    }
    expect(render(makeMessage({ content: '#poll' }))).not.toContain('poll-body')
  })
})
