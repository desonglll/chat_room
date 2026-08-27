import { ref } from 'vue'
import type { ConversationMentionRange } from '../assistantMentions'
import type { AiUiMessage } from '../aiUi'
import type { AiModelChoice, AiThread } from '../types'

interface PromptInputTarget {
  focusAt: (caret: number) => Promise<void>
}

interface MessageListTarget {
  scrollToLatest: (smooth?: boolean) => Promise<void>
  scrollToLatestSoon: () => void
}

export function useAiAssistantState() {
  const threads = ref<AiThread[]>([])
  const activeThreadId = ref('')
  const messages = ref<AiUiMessage[]>([])
  const modelOptions = ref<AiModelChoice[]>([])
  const selectedModelId = ref('')
  const roomPassword = ref('')
  const prompt = ref('')
  const loading = ref(false)
  const loadingThreads = ref(false)
  const promptInput = ref<PromptInputTarget | null>(null)
  const messageList = ref<MessageListTarget | null>(null)
  const mentionRange = ref<ConversationMentionRange | null>(null)
  const mentionIndex = ref(0)

  return {
    activeThreadId,
    loading,
    loadingThreads,
    mentionIndex,
    mentionRange,
    messageList,
    messages,
    modelOptions,
    prompt,
    promptInput,
    roomPassword,
    selectedModelId,
    threads,
  }
}
