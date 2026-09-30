import { describe, expect, test } from 'bun:test'
import {
  createDraftSynchronizer,
  draftsEqual,
  DEFAULT_DEBOUNCE_MS,
  MAX_DEBOUNCE_MS,
  MIN_DEBOUNCE_MS,
  type DraftClock,
} from './draftSync'

/** A hand-cranked clock: the domain helper must need nothing more than this shape. */
class FakeClock implements DraftClock {
  private now = 0
  private sequence = 0
  private timers = new Map<number, { at: number; fire: () => void }>()

  setTimeout(callback: () => void, delayMs: number): unknown {
    this.sequence += 1
    this.timers.set(this.sequence, { at: this.now + delayMs, fire: callback })
    return this.sequence
  }

  clearTimeout(handle: unknown): void {
    this.timers.delete(handle as number)
  }

  advance(ms: number): void {
    this.now += ms
    const due = [...this.timers.entries()].filter(([, timer]) => timer.at <= this.now).sort((a, b) => a[1].at - b[1].at)
    for (const [id, timer] of due) {
      this.timers.delete(id)
      timer.fire()
    }
  }
}

const harness = (delayMs?: number) => {
  const clock = new FakeClock()
  const saves: Array<{ chatId: string; text: string; reply: string | null }> = []
  const synchronizer = createDraftSynchronizer(
    (chatId, draft) => {
      saves.push({ chatId, text: draft.text, reply: draft.reply_to_message_id })
    },
    delayMs === undefined ? { clock } : { clock, delayMs },
  )
  return { clock, saves, synchronizer }
}

describe('draftsEqual', () => {
  test('treats an absent optional field and an explicit null as the same draft', () => {
    expect(draftsEqual({ text: 'a' }, { text: 'a', reply_to_message_id: null, topic_id: null })).toBe(true)
    expect(draftsEqual({ text: 'a' }, { text: 'a', reply_to_message_id: 'm1' })).toBe(false)
    expect(draftsEqual({ text: 'a' }, { text: 'b' })).toBe(false)
  })
})

describe('createDraftSynchronizer', () => {
  test('debounces a typing burst into one save carrying the final content', () => {
    const { clock, saves, synchronizer } = harness()
    synchronizer.update('c1', { text: 'h' })
    clock.advance(500)
    synchronizer.update('c1', { text: 'he' })
    clock.advance(500)
    synchronizer.update('c1', { text: 'hello' })
    clock.advance(DEFAULT_DEBOUNCE_MS - 1)
    expect(saves).toHaveLength(0)
    clock.advance(1)
    expect(saves).toEqual([{ chatId: 'c1', text: 'hello', reply: null }])
    expect(synchronizer.pending('c1')).toBe(false)
  })

  test("the delay is clamped into the card's 1-2 s range", () => {
    for (const [configured, effective] of [
      [50, MIN_DEBOUNCE_MS],
      [9999, MAX_DEBOUNCE_MS],
    ] as const) {
      const { clock, saves, synchronizer } = harness(configured)
      synchronizer.update('c1', { text: 'x' })
      clock.advance(effective - 1)
      expect(saves).toHaveLength(0)
      clock.advance(1)
      expect(saves).toHaveLength(1)
    }
  })

  test('an update equal to the already-synced state cancels instead of scheduling', () => {
    const { clock, saves, synchronizer } = harness()
    synchronizer.update('c1', { text: 'same' })
    synchronizer.flush('c1')
    expect(saves).toHaveLength(1)
    // The composer re-emits the identical content (focus events, re-render...).
    synchronizer.update('c1', { text: 'same' })
    expect(synchronizer.pending('c1')).toBe(false)
    clock.advance(MAX_DEBOUNCE_MS)
    expect(saves).toHaveLength(1)
    // ...but reverting an unsaved change also cancels the timer it had started.
    synchronizer.update('c1', { text: 'same edited' })
    synchronizer.update('c1', { text: 'same' })
    clock.advance(MAX_DEBOUNCE_MS)
    expect(saves).toHaveLength(1)
  })

  test('flush saves immediately and chats are independent', () => {
    const { clock, saves, synchronizer } = harness()
    synchronizer.update('c1', { text: 'one' })
    synchronizer.update('c2', { text: 'two' })
    synchronizer.flush('c1')
    expect(saves).toEqual([{ chatId: 'c1', text: 'one', reply: null }])
    clock.advance(DEFAULT_DEBOUNCE_MS)
    expect(saves).toHaveLength(2)
    expect(saves[1]).toEqual({ chatId: 'c2', text: 'two', reply: null })
  })

  test('flush with no chat id drains every pending chat; dispose drains none', () => {
    const first = harness()
    first.synchronizer.update('c1', { text: 'a' })
    first.synchronizer.update('c2', { text: 'b' })
    first.synchronizer.flush()
    expect(first.saves.map((save) => save.chatId).sort()).toEqual(['c1', 'c2'])

    const second = harness()
    second.synchronizer.update('c1', { text: 'a' })
    second.synchronizer.dispose()
    second.clock.advance(MAX_DEBOUNCE_MS)
    expect(second.saves).toHaveLength(0)
  })

  test('a remote draft_updated equal to the pending edit cancels the redundant save', () => {
    const { clock, saves, synchronizer } = harness()
    synchronizer.update('c1', { text: 'typed on device one' })
    // The same account's other device saved the same content first.
    synchronizer.accept('c1', { text: 'typed on device one', reply_to_message_id: null })
    expect(synchronizer.pending('c1')).toBe(false)
    clock.advance(MAX_DEBOUNCE_MS)
    expect(saves).toHaveLength(0)
  })

  test('a remote draft differing from the pending edit does not silence this keyboard', () => {
    const { clock, saves, synchronizer } = harness()
    synchronizer.update('c1', { text: 'local edit' })
    synchronizer.accept('c1', { text: 'remote edit' })
    expect(synchronizer.pending('c1')).toBe(true)
    clock.advance(DEFAULT_DEBOUNCE_MS)
    expect(saves).toEqual([{ chatId: 'c1', text: 'local edit', reply: null }])
  })

  test('a failed save is forgotten so the next update retries instead of skipping', async () => {
    const clock = new FakeClock()
    const attempts: string[] = []
    let failing = true
    const synchronizer = createDraftSynchronizer(
      (_chatId, draft) => {
        attempts.push(draft.text)
        return failing ? Promise.reject(new Error('offline')) : Promise.resolve()
      },
      { clock },
    )
    synchronizer.update('c1', { text: 'lost?' })
    synchronizer.flush('c1')
    await Promise.resolve() // let the rejection handler run
    await Promise.resolve()
    failing = false
    synchronizer.update('c1', { text: 'lost?' })
    expect(synchronizer.pending('c1')).toBe(true)
    clock.advance(DEFAULT_DEBOUNCE_MS)
    expect(attempts).toEqual(['lost?', 'lost?'])
  })

  test('the reply target is part of the content: changing only it schedules a save', () => {
    const { clock, saves, synchronizer } = harness()
    synchronizer.update('c1', { text: 're:', reply_to_message_id: 'm1' })
    clock.advance(DEFAULT_DEBOUNCE_MS)
    synchronizer.update('c1', { text: 're:', reply_to_message_id: 'm2' })
    clock.advance(DEFAULT_DEBOUNCE_MS)
    expect(saves.map((save) => save.reply)).toEqual(['m1', 'm2'])
  })
})
