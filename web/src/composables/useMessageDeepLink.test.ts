import { createRenderer, defineComponent, h, nextTick, reactive } from 'vue'
import { describe, expect, mock, test } from 'bun:test'

const route = reactive({
  name: 'room',
  params: { id: 'room-1' },
  query: { message: 'message-1' } as { message?: string },
})

mock.module('vue-router', () => ({ useRoute: () => route }))

const { useMessageDeepLink } = await import('./useMessageDeepLink')

function mountDeepLink(locate: (messageId: string) => Promise<boolean>) {
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
        useMessageDeepLink(
          () => 'room-1',
          () => true,
          locate,
        )
        return () => h('div')
      },
    }),
  )
  app.mount({})
  return app
}

describe('message deep-link navigation', () => {
  test('retries the same favorite after an earlier locate attempt failed', async () => {
    let attempts = 0
    const app = mountDeepLink(async () => ++attempts > 1)

    try {
      await nextTick()
      await nextTick()
      expect(attempts).toBe(1)

      route.name = 'favorites'
      route.query = {}
      await nextTick()
      route.name = 'room'
      route.query = { message: 'message-1' }
      await nextTick()
      await nextTick()

      expect(attempts).toBe(2)
    } finally {
      app.unmount()
    }
  })
})
