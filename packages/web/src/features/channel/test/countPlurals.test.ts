/** TG-1003: grouped numbers fill the text, the raw count picks the English plural form. */
import { afterAll, beforeAll, expect, test } from 'bun:test'
import { setLocale } from '@tg/core'
import { loadLocale } from '../../../i18n/index'
import { channelPostLabel } from '../channelModel'
import { memberCountText, subscriberCountText } from '../../chatInfo/chatInfoModel'

beforeAll(async () => {
  await loadLocale('en')
  setLocale('en')
})
afterAll(() => setLocale('zh-CN'))

test('info panel counts', () => {
  expect(memberCountText(1)).toBe('1 member')
  expect(memberCountText(1200)).toBe('1,200 members')
  expect(subscriberCountText(1)).toBe('1 subscriber')
})

test('channel post views', () => {
  expect(channelPostLabel({ messageId: 'm', views: 1, author: '' })).toBe('1 view')
  expect(channelPostLabel({ messageId: 'm', views: 2, author: '' })).toBe('2 views')
})
