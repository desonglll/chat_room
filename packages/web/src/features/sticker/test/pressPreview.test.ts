import { describe, expect, test } from 'bun:test'
import { createPressPreview, PREVIEW_HOLD_MS } from '../preview/pressPreview'
import { FakeClock } from './fixtures'

function setup() {
  const clock = new FakeClock()
  const opened: Array<string | null> = []
  const preview = createPressPreview({ clock, onPreview: (id) => opened.push(id) })
  return { clock, opened, preview }
}

describe('press-and-hold preview', () => {
  test('a hold opens, sliding switches, release closes and swallows the click', () => {
    const { clock, opened, preview } = setup()
    preview.down('a', 10, 10)
    clock.advance(PREVIEW_HOLD_MS)
    expect(preview.open).toBe('a')
    preview.move(80, 10, 'b')
    preview.move(90, 10, null)
    expect(preview.open).toBe('b')
    expect(preview.up()).toBe(true)
    expect(opened).toEqual(['a', 'b', null])
  })

  test('a quick tap is an ordinary click', () => {
    const { clock, opened, preview } = setup()
    preview.down('a', 0, 0)
    clock.advance(PREVIEW_HOLD_MS - 1)
    expect(preview.up()).toBe(false)
    clock.advance(1000)
    expect(opened).toEqual([])
  })

  test('moving beyond the slop before the hold is a scroll, not a preview', () => {
    const { clock, opened, preview } = setup()
    preview.down('a', 0, 0)
    preview.move(0, 30, 'a')
    clock.advance(PREVIEW_HOLD_MS)
    expect(opened).toEqual([])
    preview.down('a', 0, 0)
    preview.cancel()
    clock.advance(PREVIEW_HOLD_MS)
    expect(opened).toEqual([])
  })
})
