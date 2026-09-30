/**
 * Every bubble state in one list — the source for both the markup tests and the dev
 * fixture page (`bun packages/web/src/features/message/fixtures/index.html`). Media are
 * PNG data URIs so the page needs no server.
 */
import type { BroadcastMessage, DisplayMessage, UploadMessage } from '@tg/core'
import { LANDSCAPE_PNG, PORTRAIT_PNG } from './fixturePhotos'
import type { DeliveryStatus, GroupPosition, MessageRenderContext } from '../types'

export const VIEWER = 'u-me'

let sequence = 0

export function makeMessage(overrides: Partial<BroadcastMessage> = {}): BroadcastMessage {
  sequence += 1
  return {
    type: 'broadcast',
    message_id: `m-${sequence}`,
    sender_id: 'u-alice',
    sender: 'Alice',
    sender_avatar: '',
    content: '你好，这是一条消息',
    attachment: null,
    reply_to: null,
    recalled_at: null,
    edited_at: null,
    timestamp: '2026-09-30T09:41:00Z',
    favorite_id: null,
    forwarded_from: null,
    reactions: [],
    ...overrides,
  }
}

export function makeCtx(overrides: Partial<MessageRenderContext> = {}): MessageRenderContext {
  return {
    groupPosition: 'single',
    isOutgoing: false,
    showAvatar: false,
    showSenderName: false,
    highlighted: false,
    selected: false,
    ...overrides,
  }
}

export function photo(id: string, label: string, portrait = false, sensitive = false) {
  return {
    id,
    file_name: `${label}.png`,
    mime_type: 'image/png',
    size_bytes: 182_044,
    download_url: portrait ? PORTRAIT_PNG : LANDSCAPE_PNG,
    is_sensitive: sensitive,
  }
}

export interface BubbleFixture {
  id: string
  title: string
  message: DisplayMessage
  ctx: MessageRenderContext
  delivery?: DeliveryStatus
  selectionMode?: boolean
  reserveAvatar?: boolean
}

const LONG =
  '这是一段很长的文字，用来验证时间戳永远不会与正文重叠：无论最后一行剩下多少空间，隐形占位都会把时间挤到合适的位置。'
const EXACT = 'Line that nearly fills the bubble width so the meta must wrap onto its own line……………'
const REPLY = {
  message_id: 'm-target',
  sender: 'Bob',
  content: '明天下午三点开会？',
  attachment_file_name: null,
  recalled: false,
}
const me = { sender_id: VIEWER, sender: '我' }

function group(prefix: string, outgoing: boolean, texts: string[]): BubbleFixture[] {
  const positions: GroupPosition[] =
    texts.length === 1
      ? ['single']
      : texts.map((_, i) => (i === 0 ? 'first' : i === texts.length - 1 ? 'last' : 'middle'))
  return texts.map((content, i) => ({
    id: `${prefix}-${i}`,
    title: `${outgoing ? 'out' : 'in'} group · ${positions[i]}`,
    message: makeMessage(outgoing ? { ...me, content } : { content }),
    ctx: makeCtx({
      groupPosition: positions[i] as GroupPosition,
      isOutgoing: outgoing,
      showSenderName: !outgoing && i === 0,
      showAvatar: !outgoing && i === texts.length - 1,
    }),
    reserveAvatar: !outgoing,
  }))
}

const upload: UploadMessage = {
  type: 'upload',
  key: 'up-1',
  room_id: 'c1',
  file_name: 'report-q3.pdf',
  mime_type: 'application/pdf',
  size_bytes: 2_400_000,
  preview_url: '',
  is_sensitive: false,
  content: '',
  phase: 'uploading',
  processed_bytes: 960_000,
  total_bytes: 2_400_000,
  status: 'pending',
  error: '',
  timestamp: '2026-09-30T09:45:00Z',
}

export function bubbleFixtures(): BubbleFixture[] {
  const out = (o: Partial<MessageRenderContext> = {}) => makeCtx({ isOutgoing: true, ...o })
  return [
    {
      id: 'system',
      title: 'system message',
      message: { type: 'system', key: 's1', content: 'Bob 加入了群组' },
      ctx: makeCtx(),
    },
    ...group('in-group', false, ['第一条', '中间一条消息', '最后一条，有尾巴']),
    ...group('out-group', true, ['第一条', '中间', '最后一条，有尾巴']),
    { id: 'in-text', title: 'in · text', message: makeMessage({ content: '短消息' }), ctx: makeCtx() },
    {
      id: 'out-text-read',
      title: 'out · text · read',
      message: makeMessage({ ...me, content: '已读' }),
      ctx: out(),
      delivery: 'read',
    },
    {
      id: 'out-sending',
      title: 'out · sending',
      message: makeMessage({ ...me, content: '发送中…', delivery_state: 'sending' }),
      ctx: out(),
    },
    {
      id: 'out-failed',
      title: 'out · failed',
      message: makeMessage({ ...me, content: '网络断了', delivery_state: 'failed' }),
      ctx: out(),
    },
    { id: 'in-long', title: 'in · long text (no overlap)', message: makeMessage({ content: LONG }), ctx: makeCtx() },
    {
      id: 'out-exact',
      title: 'out · text that fills the line',
      message: makeMessage({ ...me, content: EXACT }),
      ctx: out(),
      delivery: 'read',
    },
    {
      id: 'in-word',
      title: 'in · one unbreakable word',
      message: makeMessage({ content: 'https://example.com/'.padEnd(90, 'x') }),
      ctx: makeCtx(),
    },
    {
      id: 'in-edited',
      title: 'in · edited',
      message: makeMessage({ content: '改过的消息', edited_at: '2026-09-30T09:50:00Z' }),
      ctx: makeCtx(),
    },
    {
      id: 'out-edited',
      title: 'out · edited',
      message: makeMessage({ ...me, content: '改过的消息', edited_at: '2026-09-30T09:50:00Z' }),
      ctx: out(),
      delivery: 'read',
    },
    {
      id: 'in-reply',
      title: 'in · reply',
      message: makeMessage({ content: '可以', reply_to: REPLY }),
      ctx: makeCtx({ showSenderName: true }),
    },
    {
      id: 'out-reply',
      title: 'out · reply',
      message: makeMessage({ ...me, content: '好的，三点见', reply_to: REPLY }),
      ctx: out(),
    },
    {
      id: 'in-forward',
      title: 'in · forwarded',
      message: makeMessage({ content: '转发的公告内容', forwarded_from: { sender: 'Carol', room_name: '产品讨论组' } }),
      ctx: makeCtx(),
    },
    {
      id: 'out-forward',
      title: 'out · forwarded',
      message: makeMessage({ ...me, content: '看这个', forwarded_from: { sender: 'Carol', room_name: '' } }),
      ctx: out(),
    },
    {
      id: 'in-image',
      title: 'in · image (borderless)',
      message: makeMessage({ content: '', attachment: photo('a1', 'photo') }),
      ctx: makeCtx(),
    },
    {
      id: 'out-image',
      title: 'out · image (borderless)',
      message: makeMessage({ ...me, content: '', attachment: photo('a2', 'photo', true) }),
      ctx: out(),
      delivery: 'read',
    },
    {
      id: 'in-image-caption',
      title: 'in · image + caption',
      message: makeMessage({ content: '海边的日落', attachment: photo('a3', 'sunset') }),
      ctx: makeCtx(),
    },
    {
      id: 'out-image-reply',
      title: 'out · image + reply',
      message: makeMessage({ ...me, content: '', reply_to: REPLY, attachment: photo('a4', 'reply') }),
      ctx: out(),
    },
    {
      id: 'in-image-sensitive',
      title: 'in · sensitive image',
      message: makeMessage({ content: '', attachment: photo('a5', 'hidden', false, true) }),
      ctx: makeCtx(),
    },
    {
      id: 'in-file',
      title: 'in · file',
      message: makeMessage({
        content: '',
        attachment: {
          id: 'f1',
          file_name: '季度报告-最终版.pdf',
          mime_type: 'application/pdf',
          size_bytes: 2_400_000,
          download_url: '#',
          is_sensitive: false,
        },
      }),
      ctx: makeCtx(),
    },
    {
      id: 'in-deleted',
      title: 'in · deleted',
      message: makeMessage({ content: '', recalled_at: '2026-09-30T09:42:00Z' }),
      ctx: makeCtx(),
    },
    {
      id: 'out-deleted',
      title: 'out · deleted',
      message: makeMessage({ ...me, content: '', recalled_at: '2026-09-30T09:42:00Z' }),
      ctx: out(),
    },
    {
      id: 'in-reactions',
      title: 'in · reactions',
      message: makeMessage({
        content: '周五聚餐？',
        reactions: [
          { emoji: '👍', user_ids: ['u-a', VIEWER] },
          { emoji: '❤️', user_ids: ['u-b'] },
        ],
      }),
      ctx: makeCtx(),
    },
    {
      id: 'out-reactions',
      title: 'out · reactions',
      message: makeMessage({ ...me, content: '好', reactions: [{ emoji: '😂', user_ids: ['u-a', 'u-b', 'u-c'] }] }),
      ctx: out(),
      delivery: 'read',
    },
    {
      id: 'in-highlighted',
      title: 'in · highlighted (jump target)',
      message: makeMessage({ content: '被跳转到的消息' }),
      ctx: makeCtx({ highlighted: true }),
    },
    {
      id: 'in-selecting',
      title: 'in · selection mode',
      message: makeMessage({ content: '未选中' }),
      ctx: makeCtx(),
      selectionMode: true,
    },
    {
      id: 'out-selected',
      title: 'out · selected',
      message: makeMessage({ ...me, content: '已选中' }),
      ctx: out({ selected: true }),
      selectionMode: true,
    },
    { id: 'out-upload', title: 'out · uploading file', message: upload, ctx: out() },
  ]
}
