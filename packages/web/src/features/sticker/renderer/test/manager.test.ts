import { describe, expect, test } from 'bun:test'
import { harness } from './managerHarness'

describe('stickerManager', () => {
  test('loading → static first frame; nothing renders until the view is on screen', async () => {
    const h = harness()
    const a = h.mount('/s/1.tgs')
    expect(a.phase()).toBe('loading')
    expect(a.canvas.width).toBe(200)
    await h.flush()
    expect(a.states.at(-1)).toEqual({ phase: 'static', poster: 'blob:poster-1' })
    expect(h.calls.units).toBe(0)
    expect(h.ticking()).toBe(false)
  })

  test('on screen it goes live and advances with the ticker', async () => {
    const h = harness()
    const a = h.mount('/s/1.tgs')
    await h.flush()
    await a.setVisible(true)
    expect(a.phase()).toBe('live')
    expect(h.ticking()).toBe(true)
    const before = h.calls.renders
    for (let frame = 0; frame < 10; frame += 1) h.tick()
    expect(h.calls.renders - before).toBe(10)
    expect(h.blits.at(-1)?.frame).toBe(10)
  })

  test('twenty copies of one sticker: one parse, one render unit, twenty canvases', async () => {
    const h = harness()
    const views = Array.from({ length: 20 }, () => h.mount('/s/same.tgs'))
    await h.flush()
    for (const view of views) await view.setVisible(true)
    expect(h.calls.load).toEqual(['url:/s/same.tgs'])
    expect(h.calls.units).toBe(1)
    expect(h.calls.snapshots).toBe(1)
    h.tick()
    const lastFrame = new Set(h.blits.slice(-20).map((blit) => blit.canvas))
    expect(lastFrame.size).toBe(20)
    expect(views.every((view) => view.phase() === 'live')).toBe(true)
  })

  test('off screen stops the frame loop entirely; back on screen resumes without a new unit', async () => {
    const h = harness()
    const a = h.mount('/s/1.tgs')
    await h.flush()
    await a.setVisible(true)
    h.tick()
    await a.setVisible(false)
    expect(h.ticking()).toBe(false)
    const renders = h.calls.renders
    await a.setVisible(true)
    expect(h.ticking()).toBe(true)
    expect(h.calls.units).toBe(1)
    expect(h.calls.renders).toBe(renders)
  })

  test('a paused unit is released after the idle delay; the view falls back to its still', async () => {
    const h = harness({ idleReleaseMs: 5 })
    const a = h.mount('/s/1.tgs')
    await h.flush()
    await a.setVisible(true)
    await a.setVisible(false)
    // Timer-driven: poll rather than guess a sleep that a loaded CI box might overrun.
    for (let waited = 0; h.calls.destroyed === 0 && waited < 1000; waited += 10) {
      await new Promise((resolve) => setTimeout(resolve, 10))
    }
    expect(h.calls.destroyed).toBe(1)
    expect(a.phase()).toBe('static')
  })

  test('a hidden tab pauses everything; showing it again resumes', async () => {
    const h = harness()
    const a = h.mount('/s/1.tgs')
    await h.flush()
    await a.setVisible(true)
    await h.setSignal({ hidden: true })
    expect(h.ticking()).toBe(false)
    await h.setSignal({ hidden: false })
    expect(h.ticking()).toBe(true)
  })

  test('reduced motion keeps every view on its static first frame', async () => {
    const h = harness()
    await h.setSignal({ reducedMotion: true })
    const a = h.mount('/s/1.tgs')
    await h.flush()
    await a.setVisible(true)
    expect(a.phase()).toBe('static')
    expect(h.calls.units).toBe(0)
    await h.setSignal({ reducedMotion: false })
    expect(a.phase()).toBe('live')
    await h.setSignal({ reducedMotion: true })
    expect(a.phase()).toBe('static')
    expect(h.calls.destroyed).toBe(1)
  })

  test('over the cap, extra stickers stay static', async () => {
    const h = harness({ maxGroups: 2 })
    const views = [h.mount('/s/1.tgs'), h.mount('/s/2.tgs'), h.mount('/s/3.tgs')]
    await h.flush()
    for (const view of views) await view.setVisible(true)
    expect(views.map((view) => view.phase())).toEqual(['live', 'live', 'static'])
    await views[0]?.setVisible(false)
    expect(views[2]?.phase()).toBe('live')
  })

  test('a play-once sticker stops at the end, and replay plays it again', async () => {
    const h = harness({}, 6)
    const a = h.mount('/s/once.tgs', { loop: false })
    await h.flush()
    await a.setVisible(true)
    for (let frame = 0; frame < 20; frame += 1) h.tick()
    await h.flush()
    expect(h.ticking()).toBe(false)
    expect(h.blits.at(-1)?.frame).toBe(5)
    a.view.replay()
    await h.flush()
    expect(h.blits.at(-1)?.frame).toBe(0)
    expect(h.ticking()).toBe(true)
  })

  test('tap-to-play: autoplay off stays static until replay', async () => {
    const h = harness({}, 6)
    const a = h.mount('/s/tap.tgs', { autoplay: false })
    await h.flush()
    await a.setVisible(true)
    expect(a.phase()).toBe('static')
    a.view.replay()
    await h.flush()
    expect(a.phase()).toBe('live')
    for (let frame = 0; frame < 20; frame += 1) h.tick()
    await h.flush()
    expect(h.ticking()).toBe(false)
  })

  test('a bad file ends in the error phase and reports once', async () => {
    const h = harness()
    const a = h.mount('/s/bad.tgs')
    await h.flush()
    expect(a.phase()).toBe('error')
    expect(a.errors).toHaveLength(1)
  })

  test('destroy releases the unit and stops the loop', async () => {
    const h = harness()
    const a = h.mount('/s/1.tgs')
    await h.flush()
    await a.setVisible(true)
    a.view.destroy()
    await h.flush()
    expect(h.calls.destroyed).toBe(1)
    expect(h.ticking()).toBe(false)
    expect(h.manager.stats()).toMatchObject({ views: 0, groups: 0 })
  })
})
