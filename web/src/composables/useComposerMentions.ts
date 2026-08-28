import { computed, nextTick, ref, type Ref } from 'vue'
import type { RoomMember } from '../types'

interface ComposerMentionsOptions {
  draft: Ref<string>
  input: () => HTMLTextAreaElement | null
  participants: () => RoomMember[]
}

export function useComposerMentions(options: ComposerMentionsOptions) {
  const query = ref<string | null>(null)
  let mentionStart = 0

  const matches = computed(() => {
    if (query.value === null) return []
    const normalized = query.value.toLowerCase()
    return options
      .participants()
      .filter((member) => member.username.toLowerCase().startsWith(normalized))
      .slice(0, 6)
  })

  function update(): void {
    const input = options.input()
    if (!input) {
      query.value = null
      return
    }
    const caret = input.selectionStart ?? options.draft.value.length
    const match = options.draft.value.slice(0, caret).match(/(?:^|\s)@([^\s@]*)$/)
    if (!match) {
      query.value = null
      return
    }
    mentionStart = caret - match[1].length - 1
    query.value = match[1]
  }

  function insert(username: string): void {
    const caret = options.input()?.selectionStart ?? options.draft.value.length
    const before = options.draft.value.slice(0, mentionStart)
    const after = options.draft.value.slice(caret)
    const insertion = `@${username} `
    options.draft.value = `${before}${insertion}${after}`
    query.value = null
    void nextTick(() => {
      const input = options.input()
      const position = before.length + insertion.length
      input?.focus()
      input?.setSelectionRange(position, position)
    })
  }

  return { insert, matches, query, update }
}
