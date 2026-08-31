import { ref, watch, type Ref } from 'vue'
import { evaluateArithmeticExpression, shouldCalculateExpression } from '../calculator'

interface ComposerInput {
  focusAt: (caret: number) => Promise<void>
}

export function useComposerCalculator(draft: Ref<string>, input: () => ComposerInput | null, composing: () => boolean) {
  const error = ref('')

  watch(draft, () => {
    error.value = ''
  })

  function handleKeydown(event: KeyboardEvent): boolean {
    if (!shouldCalculateExpression(event, composing())) return false
    event.preventDefault()
    const result = evaluateArithmeticExpression(draft.value)
    if (!result.ok) {
      error.value = result.error
      return true
    }
    draft.value = result.value
    void input()?.focusAt(result.value.length)
    return true
  }

  return { error, handleKeydown }
}
