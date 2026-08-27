import { createRenderer, defineComponent, h, nextTick, ref } from 'vue'
import { createMemoryHistory, createRouter } from 'vue-router'
import { describe, expect, test } from 'bun:test'
import { useRoomMessageNavigation } from './useRoomMessageNavigation'

function testRenderer() {
  return createRenderer<Record<string, never>, Record<string, never>>({
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
}

describe('room message navigation', () => {
  test('waits for viewport initialization before consuming a message deep link', async () => {
    const originalDocument = globalThis.document
    const originalHistory = globalThis.history
    Object.defineProperty(globalThis, 'document', {
      configurable: true,
      writable: true,
      value: {
        addEventListener: () => undefined,
        querySelector: () => null,
        removeEventListener: () => undefined,
      },
    })
    Object.defineProperty(globalThis, 'history', {
      configurable: true,
      writable: true,
      value: { state: {} },
    })
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [{ path: '/rooms/:id', name: 'room', component: { render: () => null } }],
    })
    await router.push('/rooms/room-1?message=message-1')
    await router.isReady()
    const viewportReady = ref(false)
    const located: string[] = []
    const messageList = {
      isReady: () => viewportReady.value,
      scrollToMessage: async (messageId: string) => {
        located.push(messageId)
        return true
      },
    }
    const app = testRenderer().createApp(
      defineComponent({
        setup() {
          useRoomMessageNavigation({
            roomId: () => 'room-1',
            ready: () => true,
            visible: () => true,
            authenticated: () => true,
            messageList: () => messageList,
            closeFiles: () => undefined,
          })
          return () => h('div')
        },
      }),
    )
    app.use(router)

    try {
      app.mount({})
      await nextTick()
      await nextTick()
      expect(located).toEqual([])

      viewportReady.value = true
      await nextTick()
      await nextTick()
      await nextTick()
      expect(located).toEqual(['message-1'])
    } finally {
      app.unmount()
      if (originalDocument) globalThis.document = originalDocument
      else Reflect.deleteProperty(globalThis, 'document')
      if (originalHistory) globalThis.history = originalHistory
      else Reflect.deleteProperty(globalThis, 'history')
    }
  })
})
