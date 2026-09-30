import { describe, expect, test } from 'bun:test'
import { createContentRegistry } from '../content/registry'
import { messageContent } from '../content/messageContent'
import { attachmentKind, formatBytes } from '../content/attachmentKind'
import { linkify } from '../content/linkify'
import { makeMessage, photo } from '../fixtures/bubbleFixtures'

const Noop = () => null
const Other = () => null

describe('createContentRegistry', () => {
  test('highest priority wins; a tie goes to the later registration', () => {
    const registry = createContentRegistry()
    registry.register('a', Noop, { match: () => true, priority: 1 })
    registry.register('b', Noop, { match: () => true, priority: 5 })
    registry.register('c', Noop, { match: () => true, priority: 5 })
    expect(registry.resolve(makeMessage())?.kind).toBe('c')
  })

  test('default match is the future wire discriminator `message.kind`', () => {
    const registry = createContentRegistry()
    registry.register('poll', Noop)
    expect(registry.resolve(makeMessage())).toBeUndefined()
    expect(registry.resolve({ ...makeMessage(), kind: 'poll' } as never)?.kind).toBe('poll')
  })

  test('re-registering replaces, and the undo restores the previous entry', () => {
    const registry = createContentRegistry()
    registry.register('text', Noop, { match: () => true })
    const undo = registry.register('text', Other, { match: () => true })
    expect(registry.resolve(makeMessage())?.component).toBe(Other)
    expect(registry.kinds()).toEqual(['text'])
    undo()
    expect(registry.resolve(makeMessage())?.component).toBe(Noop)
  })

  test('frame and metaPlacement accept a constant or a function and default to bubble/inline', () => {
    const registry = createContentRegistry()
    registry.register('x', Noop, {
      match: () => true,
      frame: 'bare',
      metaPlacement: (m) => (m.content ? 'inline' : 'overlay'),
    })
    const kind = registry.resolve(makeMessage({ content: '' }))
    expect(kind?.frame(makeMessage())).toBe('bare')
    expect(kind?.metaPlacement(makeMessage({ content: '' }))).toBe('overlay')
    registry.register('y', Noop, { match: () => true, priority: 9 })
    expect(registry.resolve(makeMessage())?.frame(makeMessage())).toBe('bubble')
  })
})

describe('built-in kinds', () => {
  const resolve = (overrides: Parameters<typeof makeMessage>[0]) => messageContent.resolve(makeMessage(overrides))?.kind

  test('text / image / video / file / deleted', () => {
    expect(resolve({})).toBe('text')
    expect(resolve({ attachment: photo('a', 'p') })).toBe('image')
    expect(resolve({ attachment: { ...photo('v', 'v'), mime_type: 'video/mp4' } })).toBe('video')
    expect(resolve({ attachment: { ...photo('f', 'f'), mime_type: 'application/zip' } })).toBe('file')
    expect(resolve({ attachment: photo('a', 'p'), recalled_at: '2026-09-30T00:00:00Z' })).toBe('deleted')
  })

  test('an image is borderless only without a caption', () => {
    const bare = makeMessage({ content: '', attachment: photo('a', 'p') })
    const captioned = makeMessage({ content: 'caption', attachment: photo('a', 'p') })
    expect(messageContent.resolve(bare)?.frame(bare)).toBe('media')
    expect(messageContent.resolve(bare)?.metaPlacement(bare)).toBe('overlay')
    expect(messageContent.resolve(captioned)?.frame(captioned)).toBe('bubble')
    expect(messageContent.resolve(captioned)?.metaPlacement(captioned)).toBe('inline')
  })

  test('SVG is never rendered inline as an image', () => {
    expect(attachmentKind({ ...photo('s', 's'), mime_type: 'image/svg+xml' })).toBe('file')
  })

  test('formatBytes', () => {
    expect(formatBytes(512)).toBe('512 B')
    expect(formatBytes(2_400_000)).toBe('2.3 MB')
    expect(formatBytes(-1)).toBe('')
  })
})

describe('linkify', () => {
  test('links only absolute http(s) URLs and leaves trailing punctuation as text', () => {
    expect(linkify('see https://x.io/a?b=1. ok')).toEqual([
      { type: 'text', text: 'see ' },
      { type: 'link', text: 'https://x.io/a?b=1', href: 'https://x.io/a?b=1' },
      { type: 'text', text: '. ok' },
    ])
    expect(linkify('javascript:alert(1) www.x.io')).toEqual([{ type: 'text', text: 'javascript:alert(1) www.x.io' }])
    expect(linkify('（https://例子.cn）')).toEqual([
      { type: 'text', text: '（' },
      { type: 'link', text: 'https://例子.cn', href: 'https://例子.cn' },
      { type: 'text', text: '）' },
    ])
  })
})
