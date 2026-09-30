// TG-107: the throttled outbound chat action, on an injected fake clock.
import { describe, expect, test } from 'bun:test'
import type { CoreClock } from '../types'
import type { TypingClientFrame } from './chatActionSender'
import { CHAT_ACTION_RESEND_MS, createChatActionSender } from './chatActionSender'
import { TYPING_TTL_MS } from './typingSummary'

class FakeClock implements CoreClock {
  nowMs = 0
  private timers: Array<{ id: number; at: number; callback: () => void }> = []
  private nextId = 1
  now = () => this.nowMs
  setTimeout = (callback: () => void, delayMs: number) => {
    const id = this.nextId++
    this.timers.push({ id, at: this.nowMs + delayMs, callback })
    return id
  }
  clearTimeout = (handle: unknown) => {
    this.timers = this.timers.filter((timer) => timer.id !== handle)
  }
  advance(ms: number) {
    const target = this.nowMs + ms
    for (;;) {
      const due = this.timers.filter((timer) => timer.at <= target).sort((a, b) => a.at - b.at)[0]
      if (!due) break
      this.timers = this.timers.filter((timer) => timer !== due)
      this.nowMs = due.at
      due.callback()
    }
    this.nowMs = target
  }
  pending = () => this.timers.length
}

function harness(online = true) {
  const clock = new FakeClock()
  const sent: Array<{ chatId: string; frame: TypingClientFrame }> = []
  const state = { online }
  const sender = createChatActionSender({
    clock,
    send: (chatId, frame) => {
      if (!state.online) return false
      sent.push({ chatId, frame })
      return true
    },
  })
  const actions = () => sent.map((entry) => entry.frame.action)
  return { clock, sent, sender, state, actions }
}

describe('createChatActionSender', () => {
  test('re-sends inside the receiver TTL', () => {
    expect(CHAT_ACTION_RESEND_MS).toBeLessThan(TYPING_TTL_MS)
  })

  test('throttles the same typing action to one frame per window, then cancels once', () => {
    const { clock, sender, sent, actions } = harness()
    sender.sendChatAction('c1', 'typing', 'h')
    clock.advance(1_000)
    sender.sendChatAction('c1', 'typing', 'he')
    clock.advance(1_000)
    sender.sendChatAction('c1', 'typing', 'hel')
    expect(actions()).toEqual(['typing'])
    clock.advance(CHAT_ACTION_RESEND_MS)
    sender.sendChatAction('c1', 'typing', 'hello')
    expect(actions()).toEqual(['typing', 'typing'])
    expect(sent[1]?.frame.content).toBe('hello')
    sender.sendChatAction('c1', 'cancel')
    sender.sendChatAction('c1', 'cancel')
    expect(actions()).toEqual(['typing', 'typing', 'cancel'])
    expect(sent[2]?.frame).toEqual({ type: 'typing', content: '', action: 'cancel' })
  })

  test('typing has no keep-alive: a paused user lets receivers expire', () => {
    const { clock, sender, actions } = harness()
    sender.sendChatAction('c1', 'typing', 'x')
    clock.advance(30_000)
    expect(actions()).toEqual(['typing'])
  })

  test('non-typing actions stay alive until cancelled', () => {
    const { clock, sender, sent, actions } = harness()
    sender.sendChatAction('c1', 'recording_voice')
    clock.advance(CHAT_ACTION_RESEND_MS * 3)
    expect(actions()).toEqual(['recording_voice', 'recording_voice', 'recording_voice', 'recording_voice'])
    expect(sent.every((entry) => entry.frame.content === '')).toBe(true)
    sender.sendChatAction('c1', 'cancel')
    clock.advance(CHAT_ACTION_RESEND_MS * 3)
    expect(actions().at(-1)).toBe('cancel')
    expect(actions()).toHaveLength(5)
  })

  test('a different action goes out immediately and replaces the keep-alive', () => {
    const { clock, sender, actions } = harness()
    sender.sendChatAction('c1', 'recording_voice')
    clock.advance(500)
    sender.sendChatAction('c1', 'uploading_voice')
    clock.advance(CHAT_ACTION_RESEND_MS)
    expect(actions()).toEqual(['recording_voice', 'uploading_voice', 'uploading_voice'])
  })

  test('empty typing preview is the legacy clear; chats are independent', () => {
    const { sender, sent } = harness()
    sender.sendChatAction('c1', 'typing', '')
    expect(sent).toEqual([])
    sender.sendChatAction('c1', 'typing', 'a')
    sender.sendChatAction('c2', 'choosing_sticker')
    sender.sendChatAction('c1', 'typing', '')
    expect(sent.map((entry) => `${entry.chatId}:${entry.frame.action}`)).toEqual([
      'c1:typing',
      'c2:choosing_sticker',
      'c1:cancel',
    ])
  })

  test('offline sends leave nothing announced, and dispose drops keep-alives', () => {
    const { clock, sender, sent, state } = harness(false)
    sender.sendChatAction('c1', 'recording_voice')
    sender.sendChatAction('c1', 'cancel')
    expect(sent).toEqual([])
    expect(clock.pending()).toBe(0)
    state.online = true
    sender.sendChatAction('c1', 'recording_voice')
    expect(clock.pending()).toBe(1)
    sender.dispose()
    expect(clock.pending()).toBe(0)
  })
})
