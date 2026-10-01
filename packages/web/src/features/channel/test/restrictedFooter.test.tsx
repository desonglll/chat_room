// TG-1203: a group member restricted from sending gets a bar instead of a composer the server
// silently ignores.
import { expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import { RestrictedFooter } from '../ChannelFooter'
import { canWriteInGroup } from '../channelModel'

test('a group permission view without message.send means read-only', () => {
  expect(canWriteInGroup(['message.send', 'message.send_media'])).toBe(true)
  expect(canWriteInGroup(['message.send_media'])).toBe(false)
  expect(canWriteInGroup([])).toBe(false)
})

test('the restricted bar says why there is no composer', () => {
  expect(renderToStaticMarkup(<RestrictedFooter />)).toContain('管理员已限制你在此群组发送消息')
})
