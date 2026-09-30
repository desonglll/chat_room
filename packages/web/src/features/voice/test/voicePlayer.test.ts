// TG-401: the single voice player — toggle, seek (drag), speed, auto-play next, onStart.
import { describe, expect, test } from 'bun:test'
import { createVoicePlayer, type AudioLike, type VoiceTrack } from '../voicePlayer'

class FakeAudio implements AudioLike {
  src = ''
  currentTime = 0
  playbackRate = 1
  paused = true
  plays = 0
  onended: ((event: Event) => void) | null = null
  onerror: ((event: Event | string) => void) | null = null
  play() {
    this.paused = false
    this.plays += 1
    return Promise.resolve()
  }
  pause() {
    this.paused = true
  }
}

const track = (id: string, durationMs = 10_000): VoiceTrack => ({ messageId: id, url: `/a/${id}`, durationMs })

function setup(next: Record<string, VoiceTrack> = {}) {
  const audio = new FakeAudio()
  const frames: Array<() => void> = []
  const started: string[] = []
  const rates: number[] = []
  const player = createVoicePlayer({
    createAudio: () => audio,
    requestFrame: (callback) => frames.push(callback),
    cancelFrame: () => {},
    findNext: (id) => next[id] ?? null,
    onStart: (played) => started.push(played.messageId),
    onRateChange: (rate) => rates.push(rate),
  })
  const tick = () => frames.splice(0).forEach((frame) => frame())
  return { audio, player, started, rates, tick }
}

const settle = () => Promise.resolve().then(() => Promise.resolve())

describe('voice player', () => {
  test('toggle plays, reports the start, tracks position, and pauses', async () => {
    const { audio, player, started, tick } = setup()
    player.toggle(track('a'))
    await settle()
    expect(audio.src).toBe('/a/a')
    expect(player.store.getState()).toMatchObject({ playing: true, current: { messageId: 'a' } })
    expect(started).toEqual(['a'])
    audio.currentTime = 2.5
    tick()
    expect(player.store.getState().positionMs).toBe(2_500)
    player.toggle(track('a'))
    expect(audio.paused).toBeTrue()
    expect(player.store.getState().playing).toBeFalse()
  })

  test('seeking another message starts it at that fraction; seeking the loaded one moves it', () => {
    const { audio, player } = setup()
    player.seek(track('a', 8_000), 0.25)
    expect(audio.currentTime).toBe(2)
    expect(player.store.getState()).toMatchObject({ playing: true, positionMs: 2_000 })
    player.seek(track('a', 8_000), 0.5)
    expect(audio.currentTime).toBe(4)
    expect(audio.plays).toBe(1)
    player.seek(track('a', 8_000), 7)
    expect(player.store.getState().positionMs).toBe(8_000)
  })

  test('switching tracks stops the previous one', () => {
    const { audio, player } = setup()
    player.toggle(track('a'))
    player.toggle(track('b'))
    expect(audio.src).toBe('/a/b')
    expect(player.store.getState().current?.messageId).toBe('b')
  })

  test('speed cycles 1× → 1.5× → 2× → 1× and applies to the element', () => {
    const { audio, player, rates } = setup()
    player.toggle(track('a'))
    player.cycleRate()
    expect(audio.playbackRate).toBe(1.5)
    player.cycleRate()
    player.cycleRate()
    expect(rates).toEqual([1.5, 2, 1])
    player.cycleRate()
    player.toggle(track('b'))
    expect(audio.playbackRate).toBe(1.5)
  })

  test('the next voice message plays automatically when one ends', async () => {
    const { audio, player, started } = setup({ a: track('b') })
    player.toggle(track('a'))
    await settle()
    audio.onended?.(new Event('ended'))
    await settle()
    expect(player.store.getState()).toMatchObject({ current: { messageId: 'b' }, playing: true })
    expect(started).toEqual(['a', 'b'])
    audio.onended?.(new Event('ended'))
    expect(player.store.getState()).toMatchObject({ current: { messageId: 'b' }, playing: false, positionMs: 10_000 })
  })

  test('play after the end restarts from zero', () => {
    const { audio, player } = setup()
    player.toggle(track('a'))
    audio.onended?.(new Event('ended'))
    audio.currentTime = 10
    player.toggle(track('a'))
    expect(audio.currentTime).toBe(0)
  })
})
