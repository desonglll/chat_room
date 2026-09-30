import { describe, expect, test } from 'bun:test'
import { computeBubbleLayout, groupCorners, type LayoutInput } from '../bubbleLayout'

const base: LayoutInput = {
  groupPosition: 'single',
  isOutgoing: false,
  frame: 'bubble',
  metaPlacement: 'inline',
  hasSenderName: false,
  hasForward: false,
  hasReply: false,
  hasReactions: false,
}

describe('groupCorners — the tweb corner grammar', () => {
  test.each([
    ['single', true, { topStart: 'full', topEnd: 'full', bottomEnd: 'full', bottomStart: 'none' }],
    ['first', false, { topStart: 'full', topEnd: 'full', bottomEnd: 'full', bottomStart: 'merged' }],
    ['middle', false, { topStart: 'merged', topEnd: 'full', bottomEnd: 'full', bottomStart: 'merged' }],
    ['last', true, { topStart: 'merged', topEnd: 'full', bottomEnd: 'full', bottomStart: 'none' }],
    // `last` without a tail keeps the full radius on the tail corner.
    ['last', false, { topStart: 'merged', topEnd: 'full', bottomEnd: 'full', bottomStart: 'full' }],
  ] as const)('%s (tail=%p)', (position, tail, expected) => {
    expect(groupCorners(position, tail)).toEqual(expected)
  })
})

describe('computeBubbleLayout', () => {
  test('only the last (or single) message of a group has a tail', () => {
    expect(computeBubbleLayout({ ...base, groupPosition: 'first' }).tail).toBe(false)
    expect(computeBubbleLayout({ ...base, groupPosition: 'middle' }).tail).toBe(false)
    expect(computeBubbleLayout({ ...base, groupPosition: 'last' }).tail).toBe(true)
    expect(computeBubbleLayout({ ...base, groupPosition: 'single' }).tail).toBe(true)
  })

  test('incoming tail corner is bottom-left; outgoing mirrors to bottom-right', () => {
    const inLayout = computeBubbleLayout({ ...base, groupPosition: 'last' })
    expect(inLayout.corners).toEqual({ topLeft: 'merged', topRight: 'full', bottomRight: 'full', bottomLeft: 'none' })
    const outLayout = computeBubbleLayout({ ...base, groupPosition: 'last', isOutgoing: true })
    expect(outLayout.corners).toEqual({ topLeft: 'full', topRight: 'merged', bottomRight: 'none', bottomLeft: 'full' })
    const outMiddle = computeBubbleLayout({ ...base, groupPosition: 'middle', isOutgoing: true })
    expect(outMiddle.corners).toEqual({
      topLeft: 'full',
      topRight: 'merged',
      bottomRight: 'merged',
      bottomLeft: 'full',
    })
  })

  test('borderless media has no tail and full corners when alone', () => {
    const layout = computeBubbleLayout({ ...base, frame: 'media', metaPlacement: 'overlay' })
    expect(layout.frame).toBe('media')
    expect(layout.tail).toBe(false)
    expect(layout.corners.bottomLeft).toBe('full')
    expect(layout.metaMode).toBe('overlay')
  })

  test.each(['hasSenderName', 'hasForward', 'hasReply', 'hasReactions'] as const)(
    'media falls back to a filled, tailed bubble when %s',
    (flag) => {
      const layout = computeBubbleLayout({ ...base, frame: 'media', metaPlacement: 'overlay', [flag]: true })
      expect(layout.frame).toBe('bubble')
      expect(layout.tail).toBe(true)
    },
  )

  test('reactions claim the meta; otherwise the kind decides', () => {
    expect(computeBubbleLayout({ ...base, hasReactions: true }).metaMode).toBe('reactions')
    expect(
      computeBubbleLayout({ ...base, frame: 'media', metaPlacement: 'overlay', hasReactions: true }).metaMode,
    ).toBe('reactions')
    expect(computeBubbleLayout({ ...base, frame: 'media', metaPlacement: 'overlay', hasReply: true }).metaMode).toBe(
      'overlay',
    )
    expect(computeBubbleLayout(base).metaMode).toBe('inline')
  })

  test('bare contents never get a bubble or a tail', () => {
    const layout = computeBubbleLayout({ ...base, frame: 'bare', hasReply: true })
    expect(layout.frame).toBe('bare')
    expect(layout.tail).toBe(false)
  })
})
