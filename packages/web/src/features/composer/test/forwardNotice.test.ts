/** TG-802: a forward refused by the recipient's voice privacy is reported, others are not. */
import { expect, test } from 'bun:test'
import type { ForwardResult } from '@tg/core'
import { forwardRefusalNotice } from '../forwardNotice'

const result = (skipped_reason: string | null): ForwardResult => ({
  message_id: 'm',
  target_room_id: 'c',
  forwarded_message_id: skipped_reason ? null : 'n',
  skipped_reason,
})

test('a voice-privacy refusal earns the voice notice', () => {
  expect(forwardRefusalNotice([result(null), result('voice_messages_restricted')])).toBe('对方设置了不接收你的语音消息')
})

test('successes and unrelated skips stay silent', () => {
  expect(forwardRefusalNotice([result(null)])).toBeNull()
  expect(forwardRefusalNotice([result('message was recalled')])).toBeNull()
})
