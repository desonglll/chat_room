import { computed, createRenderer, defineComponent, h, nextTick, ref, watch } from 'vue'
import { describe, expect, test } from 'bun:test'
import { useMessageViewport } from './useMessageViewport'
import type { BroadcastMessage } from '../types'

function historyMessage(): BroadcastMessage {
  return {
    type: 'broadcast',
    message_id: 'message-1',
    sender_id: 'other-user',
    sender: 'other',
    sender_avatar: '',
    content: 'history',
    attachment: null,
    forwarded_from: null,
    reply_to: null,
    recalled_at: null,
    edited_at: null,
    timestamp: '2026-08-21T00:00:00Z',
    reactions: [],
    motion: 'none',
  }
}

describe('message viewport initialization', () => {
  test('reveals initial history only after positioning it at the bottom', async () => {
    const originalWindow = globalThis.window
    const originalDocument = globalThis.document
    Object.defineProperty(globalThis, 'window', {
      configurable: true,
      writable: true,
      value: {
        cancelAnimationFrame: () => undefined,
        clearTimeout: () => undefined,
        requestAnimationFrame: () => 0,
        setTimeout: () => 0,
      },
    })
    Object.defineProperty(globalThis, 'document', {
      configurable: true,
      writable: true,
      value: {
        addEventListener: () => undefined,
        removeEventListener: () => undefined,
        visibilityState: 'visible',
      },
    })

    const historyReady = ref(false)
    const events: string[] = []
    let scrollTop = 0
    const list = ref({
      clientHeight: 300,
      scrollHeight: 900,
      get scrollTop() {
        return scrollTop
      },
      set scrollTop(value: number) {
        scrollTop = value
        events.push(`scroll:${value}`)
      },
      querySelector: () => null,
      querySelectorAll: () => [],
    } as unknown as HTMLElement)
    let viewport: ReturnType<typeof useMessageViewport> | undefined
    const renderer = createRenderer<Record<string, never>, Record<string, never>>({
      patchProp: () => undefined,
      insert: () => undefined,
      remove: () => undefined,
      createElement: () => ({}),
      createText: () => ({}),
      createComment: () => ({}),
      setText: () => undefined,
      setElementText: () => undefined,
      parentNode: () => null,
      nextSibling: () => null,
      querySelector: () => null,
      setScopeId: () => undefined,
      cloneNode: (node) => node,
      insertStaticContent: () => [{}, {}],
    })
    const app = renderer.createApp(
      defineComponent({
        setup() {
          viewport = useMessageViewport({
            list,
            broadcasts: computed(() => [historyMessage()]),
            roomId: () => 'room-1',
            unreadCount: () => 0,
            historyReady: () => historyReady.value,
            currentUserId: () => 'current-user',
            readReceipts: () => [],
            visible: () => true,
            onRead: () => undefined,
          })
          return () => h('div')
        },
      }),
    )

    try {
      app.mount({})

      expect(viewport?.viewportReady.value).toBe(false)
      if (viewport) {
        watch(
          viewport.viewportReady,
          (ready) => {
            if (ready) events.push('visible')
          },
          { flush: 'sync' },
        )
      }

      historyReady.value = true
      await nextTick()
      await nextTick()

      expect(scrollTop).toBe(900)
      expect(events).toEqual(['scroll:900', 'visible'])
    } finally {
      app.unmount()
      if (originalWindow) globalThis.window = originalWindow
      else Reflect.deleteProperty(globalThis, 'window')
      if (originalDocument) globalThis.document = originalDocument
      else Reflect.deleteProperty(globalThis, 'document')
    }
  })
})
