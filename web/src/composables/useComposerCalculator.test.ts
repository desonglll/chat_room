import { describe, expect, test } from 'bun:test'
import { nextTick, ref } from 'vue'
import { useComposerCalculator } from './useComposerCalculator'

function keyEvent(altKey = true): { event: KeyboardEvent; wasPrevented: () => boolean } {
  let prevented = false
  const event = {
    key: 'Enter',
    altKey,
    shiftKey: false,
    metaKey: false,
    ctrlKey: false,
    isComposing: false,
    keyCode: 13,
    preventDefault: () => {
      prevented = true
    },
  } as KeyboardEvent
  return { event, wasPrevented: () => prevented }
}

describe('composer calculator', () => {
  test('replaces the draft, preserves focus, and consumes Option+Enter', () => {
    const draft = ref('2 + 3 * 4')
    let focusedAt = -1
    const { error, handleKeydown } = useComposerCalculator(
      draft,
      () => ({
        focusAt: async (caret) => {
          focusedAt = caret
        },
      }),
      () => false,
    )
    const shortcut = keyEvent()

    expect(handleKeydown(shortcut.event)).toBe(true)
    expect(shortcut.wasPrevented()).toBe(true)
    expect(draft.value).toBe('14')
    expect(focusedAt).toBe(2)
    expect(error.value).toBe('')
  })

  test('keeps invalid input and clears its error after the user edits', async () => {
    const draft = ref('2 +')
    const { error, handleKeydown } = useComposerCalculator(
      draft,
      () => null,
      () => false,
    )

    expect(handleKeydown(keyEvent().event)).toBe(true)
    expect(draft.value).toBe('2 +')
    expect(error.value).toBe('无法计算这个算式')

    draft.value = '2 + 2'
    await nextTick()
    expect(error.value).toBe('')
  })

  test('leaves other key combinations for the composer', () => {
    const draft = ref('2 + 2')
    const { handleKeydown } = useComposerCalculator(
      draft,
      () => null,
      () => false,
    )
    const plainEnter = keyEvent(false)

    expect(handleKeydown(plainEnter.event)).toBe(false)
    expect(plainEnter.wasPrevented()).toBe(false)
    expect(draft.value).toBe('2 + 2')
  })
})
