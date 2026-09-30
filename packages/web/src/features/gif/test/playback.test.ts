import { describe, expect, test } from 'bun:test'
import { autoplayAllowed, createGifPlayback, shouldPlay } from '../playback'

function fakeMedia() {
  const calls: string[] = []
  return { calls, media: { play: () => calls.push('play'), pause: () => calls.push('pause') } }
}

describe('GIF autoplay policy', () => {
  test('reduced motion or Save-Data disables autoplay', () => {
    expect(autoplayAllowed({ reducedMotion: false, saveData: false })).toBe(true)
    expect(autoplayAllowed({ reducedMotion: true, saveData: false })).toBe(false)
    expect(autoplayAllowed({ reducedMotion: false, saveData: true })).toBe(false)
  })

  test('an off-screen GIF never plays, whatever else is true', () => {
    expect(shouldPlay({ visible: false, started: true, stopped: false }, true)).toBe(false)
    expect(shouldPlay({ visible: true, started: false, stopped: false }, true)).toBe(true)
    expect(shouldPlay({ visible: true, started: false, stopped: false }, false)).toBe(false)
    expect(shouldPlay({ visible: true, started: true, stopped: false }, false)).toBe(true)
  })

  test('autoplay: plays in the viewport and pauses as soon as it leaves', () => {
    const { calls, media } = fakeMedia()
    const changes: boolean[] = []
    const playback = createGifPlayback(true, (playing) => changes.push(playing))
    playback.attach(media)
    expect(calls).toEqual(['pause'])
    playback.setVisible(true)
    expect(playback.isPlaying()).toBe(true)
    playback.setVisible(false)
    expect(playback.isPlaying()).toBe(false)
    expect(calls).toEqual(['pause', 'play', 'pause'])
    expect(changes).toEqual([true, false])
    playback.setVisible(true)
    expect(calls.at(-1)).toBe('play')
  })

  test('reduced motion: an on-screen GIF stays still until tapped, then pauses off-screen', () => {
    const { calls, media } = fakeMedia()
    const playback = createGifPlayback(autoplayAllowed({ reducedMotion: true, saveData: false }))
    playback.attach(media)
    playback.setVisible(true)
    expect(playback.isPlaying()).toBe(false)
    expect(calls).not.toContain('play')
    playback.toggle()
    expect(playback.isPlaying()).toBe(true)
    expect(calls.at(-1)).toBe('play')
    playback.setVisible(false)
    expect(calls.at(-1)).toBe('pause')
    // Back on screen: the viewer's own tap still holds.
    playback.setVisible(true)
    expect(calls.at(-1)).toBe('play')
    playback.toggle()
    expect(playback.isPlaying()).toBe(false)
    expect(calls.at(-1)).toBe('pause')
  })

  test('a tap pauses an autoplaying GIF, and it stays paused when it scrolls back', () => {
    const { calls, media } = fakeMedia()
    const playback = createGifPlayback(true)
    playback.attach(media)
    playback.setVisible(true)
    playback.toggle()
    expect(playback.isPlaying()).toBe(false)
    playback.setVisible(false)
    playback.setVisible(true)
    expect(playback.isPlaying()).toBe(false)
    expect(calls.at(-1)).toBe('pause')
  })

  test('without an element the decision is still tracked and applied on attach', () => {
    const playback = createGifPlayback(true)
    playback.setVisible(true)
    expect(playback.isPlaying()).toBe(true)
    const { calls, media } = fakeMedia()
    playback.attach(media)
    expect(calls).toEqual(['play'])
  })
})
