// TG-202 markup: the subscriber bar's three states, the post meta inside the TG-103 bubble,
// and the create dialog's error copy. (zustand renders a store's initial state server-side, and
// the modal is a portal, so live values and the dialog body are covered by the model tests.)
import { afterEach, expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import type { BroadcastMessage } from '@tg/core'
import { ApiError, chatListStore } from '@tg/core'
import { MessageMeta, MetaSpacer, metaParts } from '../../message/MessageMeta'
import { ChannelFooterView, channelStore, createChannelError } from '../index'

afterEach(() => {
  chatListStore.getState().setChats([])
  channelStore.getState().clear()
})

const noop = () => {}
const footer = (state: { subscribed?: boolean; muted?: boolean; pending?: boolean; error?: string }) =>
  renderToStaticMarkup(
    <ChannelFooterView
      subscribed={state.subscribed ?? false}
      muted={state.muted ?? false}
      pending={state.pending ?? false}
      busy={false}
      error={state.error ?? ''}
      onSubscribe={noop}
      onToggleMute={noop}
      onLeave={noop}
    />,
  )

test('a visitor gets 订阅, a subscriber 静音/取消静音 and 退订', () => {
  expect(footer({})).toContain('订阅')
  expect(footer({})).not.toContain('退订')
  const subscribed = footer({ subscribed: true })
  expect(subscribed).toContain('静音')
  expect(subscribed).toContain('退订')
  expect(subscribed).not.toContain('取消静音')
  expect(footer({ subscribed: true, muted: true })).toContain('取消静音')
  expect(footer({ pending: true })).toContain('等待管理员审核')
  expect(footer({ subscribed: true, error: '操作失败' })).toContain('role="alert"')
})

const post = (extra: Partial<BroadcastMessage>) =>
  ({
    type: 'broadcast',
    message_id: 'p1',
    sender_id: 'u1',
    sender: '新闻频道',
    sender_avatar: '',
    content: 'hi',
    attachment: null,
    reply_to: null,
    recalled_at: null,
    edited_at: null,
    timestamp: '2026-10-01T08:30:00Z',
    favorite_id: null,
    forwarded_from: null,
    reactions: [],
    ...extra,
  }) as BroadcastMessage

test('a channel post meta shows views and the signature in both copies', () => {
  const parts = metaParts(post({ views: 1234, post_author: '小编' }), false, 'sent')
  let html = renderToStaticMarkup(<MessageMeta parts={parts} overlay={false} dateTime="" />)
  expect(html).toContain('1.2K')
  expect(html).toContain('小编')
  expect(html).toContain('1.2K 次浏览 小编')
  // The visible copy and the invisible spacer draw the same post parts (no-overlap rule).
  html = renderToStaticMarkup(<MetaSpacer parts={parts} />)
  expect(html).toContain('1.2K')
  const plain = renderToStaticMarkup(
    <MessageMeta parts={metaParts(post({}), false, 'sent')} overlay={false} dateTime="" />,
  )
  expect(plain).not.toContain('tg-channel-post')
})

test('create errors are explained', () => {
  expect(createChannelError(new ApiError(409, '/api/chats', 'conflict'))).toContain('同名')
  expect(createChannelError(new ApiError(400, '/api/chats', 'bad'))).toContain('不合要求')
  expect(createChannelError(new Error('network'))).toContain('稍后再试')
})
