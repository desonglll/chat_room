// TG-202: view formatting, the post parts the meta draws, the live view store and the
// batching view reporter.
import { describe, expect, test } from 'bun:test'
import type { BroadcastMessage, MessageViewCount } from '@tg/core'
import { MAX_VIEWED_POSTS } from '@tg/core'
import { canPublish, channelPostLabel, channelPostOf, formatViews, subscriberLine } from '../channelModel'
import { applyViewsFrame, createChannelStore, effectiveViews } from '../channelStore'
import { createViewReporter } from '../viewReporter'

const message = (extra: Partial<BroadcastMessage> = {}) =>
  ({ type: 'broadcast', message_id: 'm1', ...extra }) as BroadcastMessage

describe('channel model', () => {
  test('views use Telegram compact units', () => {
    expect([0, 7, 999, 1000, 1234, 9999, 12_345, 999_999, 1_234_567, 25_000_000].map(formatViews)).toEqual([
      '0',
      '7',
      '999',
      '1K',
      '1.2K',
      '9.9K',
      '12K',
      '999K',
      '1.2M',
      '25M',
    ])
    expect(formatViews(-3)).toBe('0')
  })

  test('the subscriber line groups digits', () => {
    expect(subscriberLine(1)).toBe('1 位订阅者')
    expect(subscriberLine(200_000)).toBe('200,000 位订阅者')
  })

  test('only a message carrying views is a channel post', () => {
    expect(channelPostOf(message())).toBeNull()
    expect(channelPostOf(message({ views: 0 }))).toEqual({ messageId: 'm1', views: 0, author: '' })
    const signed = channelPostOf(message({ views: 1500, post_author: '小编' }))
    expect(signed).toEqual({ messageId: 'm1', views: 1500, author: '小编' })
    expect(channelPostLabel(signed!)).toBe('1.5K 次浏览 小编')
  })

  test('publishing needs message.post in the permission view', () => {
    expect(canPublish(null)).toBe(false)
    expect(canPublish(['message.send'])).toBe(false)
    expect(canPublish(['message.post'])).toBe(true)
  })
})

describe('channel store', () => {
  test('counts only grow, whatever order the sources arrive in', () => {
    const store = createChannelStore()
    applyViewsFrame({ type: 'message_views_updated', views: [{ message_id: 'm1', views: 5 }] }, store)
    store.getState().mergeViews([
      { message_id: 'm1', views: 3 },
      { message_id: 'm2', views: 1 },
    ])
    expect(store.getState().views).toEqual({ m1: 5, m2: 1 })
    const before = store.getState()
    store.getState().mergeViews([{ message_id: 'm1', views: 5 }])
    expect(store.getState()).toBe(before)
    expect(effectiveViews(9, store.getState().views.m1)).toBe(9)
    expect(effectiveViews(2, undefined)).toBe(2)
  })

  test('the first signature sticks', () => {
    const store = createChannelStore()
    store.getState().rememberAuthor('m1', '甲')
    store.getState().rememberAuthor('m1', '')
    expect(store.getState().authors.m1).toBe('甲')
  })
})

describe('view reporter', () => {
  function setup(fail = false) {
    const timers: Array<() => void> = []
    const reports: Array<{ chatId: string; ids: readonly string[] }> = []
    const counts: MessageViewCount[] = []
    const reporter = createViewReporter({
      report: (chatId, ids) => {
        reports.push({ chatId, ids })
        return fail
          ? Promise.reject(new Error('down'))
          : Promise.resolve(ids.map((id) => ({ message_id: id, views: 1 })))
      },
      onCounts: (answer) => counts.push(...answer),
      setTimer: (callback) => timers.push(callback),
    })
    return { reporter, timers, reports, counts }
  }

  test('batches per chat, once per post, in reports of at most 100', async () => {
    const { reporter, timers, reports, counts } = setup()
    for (let index = 0; index < MAX_VIEWED_POSTS + 20; index += 1) reporter.seen('c1', `m${index}`)
    reporter.seen('c1', 'm0')
    reporter.seen('c2', 'x')
    reporter.seen('c2', 'pending:local')
    expect(timers).toHaveLength(1)
    timers[0]!()
    await Promise.resolve()
    expect(reports.map((report) => [report.chatId, report.ids.length])).toEqual([
      ['c1', 100],
      ['c1', 20],
      ['c2', 1],
    ])
    expect(counts).toHaveLength(121)
    reporter.seen('c1', 'm5')
    expect(timers).toHaveLength(1)
  })

  test('a failed report is retried when the post mounts again', async () => {
    const { reporter, timers, reports } = setup(true)
    reporter.seen('c1', 'm1')
    timers[0]!()
    await new Promise((resolve) => setTimeout(resolve, 0))
    reporter.seen('c1', 'm1')
    expect(timers).toHaveLength(2)
    timers[1]!()
    expect(reports).toHaveLength(2)
  })
})
