/** TG-905: the subscriber line picks its English plural from the count, not the grouped text. */
import { afterAll, beforeAll, expect, test } from 'bun:test'
import { setLocale } from '@tg/core'
import { loadLocale } from '../../../i18n/index'
import { subscriberLine } from '../channelModel'

beforeAll(async () => {
  await loadLocale('en')
  setLocale('en')
})
afterAll(() => setLocale('zh-CN'))

test('1 subscriber, 1,234 subscribers', () => {
  expect(subscriberLine(1)).toBe('1 subscriber')
  expect(subscriberLine(1234)).toBe('1,234 subscribers')
})
