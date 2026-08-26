import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { shouldFocusComposer } from '../composer'
import type { DisplayMessage, FocusShortcut } from '../types'

interface ChatPanelInteractionOptions {
  messages: () => DisplayMessage[]
  pokedAt: () => number
  visible: () => boolean
  authenticated: () => boolean
  focusShortcut: () => FocusShortcut
  focusComposer: () => void
  addFiles: (files: File[]) => void
}

export function useChatPanelInteractions(options: ChatPanelInteractionOptions) {
  const dragActive = ref(false)
  const previewImageId = ref('')
  const shaking = ref(false)
  let dragDepth = 0
  let shakeTimer: number | undefined

  const galleryImages = computed(() =>
    options
      .messages()
      .flatMap((message) =>
        message.type === 'broadcast' && message.attachment?.mime_type.startsWith('image/') && !message.recalled_at
          ? [message.attachment]
          : [],
      ),
  )

  function handleGlobalKeydown(event: KeyboardEvent): void {
    const target = event.target as HTMLElement | null
    const editable = Boolean(target?.closest('input, textarea, select, button, [contenteditable="true"]'))
    if (
      !options.visible() ||
      !options.authenticated() ||
      !shouldFocusComposer(event, options.focusShortcut(), editable, Boolean(document.querySelector('.p-dialog-mask')))
    )
      return
    event.preventDefault()
    options.focusComposer()
  }

  function handleDragEnter(event: DragEvent): void {
    if (!event.dataTransfer?.types.includes('Files')) return
    dragDepth += 1
    dragActive.value = true
  }

  function handleDragLeave(): void {
    dragDepth = Math.max(0, dragDepth - 1)
    if (!dragDepth) dragActive.value = false
  }

  function handleDrop(event: DragEvent): void {
    dragDepth = 0
    dragActive.value = false
    const files = Array.from(event.dataTransfer?.files || [])
    if (files.length) options.addFiles(files)
  }

  watch(options.pokedAt, (value) => {
    if (!value) return
    shaking.value = false
    void requestAnimationFrame(() => {
      shaking.value = true
      window.clearTimeout(shakeTimer)
      shakeTimer = window.setTimeout(() => {
        shaking.value = false
      }, 600)
    })
  })

  onMounted(() => document.addEventListener('keydown', handleGlobalKeydown))
  onBeforeUnmount(() => {
    document.removeEventListener('keydown', handleGlobalKeydown)
    window.clearTimeout(shakeTimer)
  })

  return {
    dragActive,
    galleryImages,
    handleDragEnter,
    handleDragLeave,
    handleDrop,
    previewImageId,
    shaking,
  }
}
