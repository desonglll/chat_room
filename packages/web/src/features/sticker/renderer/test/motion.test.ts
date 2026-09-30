/** WebM stickers as manager motion views: the TGS playback policy, applied to `<video>`. */
import { describe, expect, test } from 'bun:test'
import { harness } from './managerHarness'

describe('motion views (WebM)', () => {
  test('held until on screen; plays on screen; paused again off screen', async () => {
    const h = harness()
    const a = h.mountMotion('/s/1.webm')
    await h.flush()
    expect(a.decisions).toEqual([{ playing: false, reducedMotion: false }])
    await a.setVisible(true)
    expect(a.playing()).toBe(true)
    await a.setVisible(false)
    expect(a.playing()).toBe(false)
    expect(h.manager.stats()).toMatchObject({ motionViews: 1, playingMotionViews: 0 })
  })

  test('a hidden tab pauses it; coming back resumes', async () => {
    const h = harness()
    const a = h.mountMotion('/s/1.webm')
    await a.setVisible(true)
    await h.setSignal({ hidden: true })
    expect(a.playing()).toBe(false)
    await h.setSignal({ hidden: false })
    expect(a.playing()).toBe(true)
  })

  test('reduced motion: never plays, and asks for the first frame', async () => {
    const h = harness()
    const a = h.mountMotion('/s/1.webm')
    await a.setVisible(true)
    await h.setSignal({ reducedMotion: true })
    expect(a.decisions.at(-1)).toEqual({ playing: false, reducedMotion: true })
    await a.setVisible(false)
    await a.setVisible(true)
    expect(a.playing()).toBe(false)
  })

  test('shares one concurrency cap with TGS; each video counts as its own group', async () => {
    const h = harness({ maxGroups: 2 })
    const tgs = h.mount('/s/1.tgs')
    await h.flush()
    await tgs.setVisible(true)
    const first = h.mountMotion('/s/same.webm')
    const second = h.mountMotion('/s/same.webm')
    await first.setVisible(true)
    await second.setVisible(true)
    expect(tgs.phase()).toBe('live')
    expect(first.playing()).toBe(true)
    expect(second.playing()).toBe(false)
    // A slot frees up: the waiting video takes it.
    await tgs.setVisible(false)
    expect(second.playing()).toBe(true)
  })

  test('a playing video keeps its slot when another sticker scrolls in', async () => {
    const h = harness({ maxGroups: 1 })
    const a = h.mountMotion('/s/a.webm')
    await a.setVisible(true)
    const tgs = h.mount('/s/1.tgs')
    await h.flush()
    await tgs.setVisible(true)
    expect(a.playing()).toBe(true)
    expect(tgs.phase()).toBe('static')
  })

  test('play-once: ends, stays still, and a replay plays it again', async () => {
    const h = harness()
    const a = h.mountMotion('/s/1.webm', { loop: false })
    await a.setVisible(true)
    expect(a.playing()).toBe(true)
    a.view.ended()
    await h.flush()
    expect(a.playing()).toBe(false)
    a.view.replay()
    await h.flush()
    expect(a.playing()).toBe(true)
  })

  test('not autoplay: only a tap (replay) plays it', async () => {
    const h = harness()
    const a = h.mountMotion('/s/1.webm', { autoplay: false })
    await a.setVisible(true)
    expect(a.playing()).toBe(false)
    a.view.replay()
    await h.flush()
    expect(a.playing()).toBe(true)
  })

  test('destroy unregisters and frees its slot', async () => {
    const h = harness({ maxGroups: 1 })
    const a = h.mountMotion('/s/a.webm')
    const b = h.mountMotion('/s/b.webm')
    await a.setVisible(true)
    await b.setVisible(true)
    expect(b.playing()).toBe(false)
    a.view.destroy()
    await h.flush()
    expect(b.playing()).toBe(true)
    expect(h.manager.stats().motionViews).toBe(1)
  })
})
