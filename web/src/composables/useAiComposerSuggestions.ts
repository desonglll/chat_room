import { onBeforeUnmount, ref, watch, type Ref } from 'vue'
import { streamAiSuggestions } from '../aiSuggestionApi'

interface AiComposerSuggestionOptions {
  draft: Ref<string>
  roomId: () => string
  token: () => string
  password: () => string
  focusDraftEnd: () => void
}

export function useAiComposerSuggestions(options: AiComposerSuggestionOptions) {
  const loading = ref(false)
  const error = ref('')
  const summary = ref('')
  const current = ref('')
  const remaining = ref<string[]>([])
  let typewriterTimer: number | undefined
  let typewriting = false

  function clearTypewriter(): void {
    window.clearInterval(typewriterTimer)
    typewriterTimer = undefined
    typewriting = false
  }

  function clear(): void {
    clearTypewriter()
    current.value = ''
    remaining.value = []
    error.value = ''
    summary.value = ''
  }

  function typewrite(text: string): void {
    clearTypewriter()
    typewriting = true
    options.draft.value = ''
    let index = 0
    typewriterTimer = window.setInterval(() => {
      index = Math.min(text.length, index + 2)
      options.draft.value = text.slice(0, index)
      if (index >= text.length) clearTypewriter()
    }, 16)
  }

  async function open(): Promise<void> {
    if (loading.value) return
    error.value = ''
    summary.value = ''
    current.value = ''
    remaining.value = []
    loading.value = true
    try {
      await streamAiSuggestions(options.roomId(), options.token(), options.password(), (item) => {
        if (item.type === 'summary') {
          summary.value = item.content
        } else if (!current.value) {
          current.value = item.content
          typewrite(item.content)
        } else if (item.content !== current.value && !remaining.value.includes(item.content)) {
          remaining.value.push(item.content)
        }
      })
    } catch (caught) {
      error.value = caught instanceof Error ? caught.message : 'AI 助手不可用'
    } finally {
      loading.value = false
    }
  }

  function useSuggestion(suggestion: string, index: number): void {
    clearTypewriter()
    const rest = remaining.value.filter((_, candidate) => candidate !== index)
    if (current.value) rest.unshift(current.value)
    remaining.value = rest
    current.value = suggestion
    options.draft.value = suggestion
    options.focusDraftEnd()
  }

  watch(options.draft, (content) => {
    if (typewriting) return
    if ((current.value || remaining.value.length) && content !== current.value) {
      current.value = ''
      remaining.value = []
    }
  })

  onBeforeUnmount(clearTypewriter)

  return { clear, current, error, loading, open, remaining, summary, useSuggestion }
}
