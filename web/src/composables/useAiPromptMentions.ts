import { computed, type ComputedRef, type Ref } from 'vue'
import {
  activeConversationMention,
  conversationMentionCandidates,
  insertConversationMention,
  type ConversationMentionRange,
  type MentionableConversation,
} from '../assistantMentions'
import { shouldSubmitMessage } from '../composer'

interface AiPromptMentionsOptions {
  prompt: Ref<string>
  input: Ref<{ focusAt: (caret: number) => Promise<void> } | null>
  range: Ref<ConversationMentionRange | null>
  activeIndex: Ref<number>
  conversations: ComputedRef<MentionableConversation[]>
  attachConversation: (roomId: string) => Promise<void>
  submit: () => Promise<void>
}

export function useAiPromptMentions(options: AiPromptMentionsOptions) {
  const candidates = computed(() => conversationMentionCandidates(options.range.value, options.conversations.value))

  function handleInput(value: string, caret: number): void {
    options.prompt.value = value
    options.range.value = activeConversationMention(value, caret, options.conversations.value)
    options.activeIndex.value = Math.min(options.activeIndex.value, Math.max(0, candidates.value.length - 1))
  }

  function choose(conversation: MentionableConversation): void {
    if (!options.range.value) return
    const inserted = insertConversationMention(options.prompt.value, options.range.value, conversation)
    options.prompt.value = inserted.value
    options.range.value = null
    options.activeIndex.value = 0
    void options.attachConversation(conversation.roomId)
    void options.input.value?.focusAt(inserted.caret)
  }

  function handleKeydown(event: KeyboardEvent): void {
    if (options.range.value && candidates.value.length) {
      if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
        event.preventDefault()
        const offset = event.key === 'ArrowDown' ? 1 : -1
        options.activeIndex.value =
          (options.activeIndex.value + offset + candidates.value.length) % candidates.value.length
        return
      }
      if (event.key === 'Enter' || event.key === 'Tab') {
        event.preventDefault()
        choose(candidates.value[options.activeIndex.value])
        return
      }
    }
    if (event.key === 'Escape' && options.range.value) {
      event.preventDefault()
      options.range.value = null
      return
    }
    if (!shouldSubmitMessage(event, false)) return
    event.preventDefault()
    void options.submit()
  }

  return { candidates, choose, handleInput, handleKeydown }
}
