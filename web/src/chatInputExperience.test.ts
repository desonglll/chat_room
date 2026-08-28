import { describe, expect, test } from 'bun:test'
import { readFileSync } from 'node:fs'

function source(path: string): string {
  return readFileSync(new URL(path, import.meta.url), 'utf8')
}

describe('chat input experience contract', () => {
  test('does not render PWA actions outside the fixed-height workspace', () => {
    const app = source('./App.vue')
    const bootstrap = source('./composables/useAppBootstrap.ts')
    expect(app).not.toContain("import PwaStatusBar from './components/PwaStatusBar.vue'")
    expect(app).not.toContain('<PwaStatusBar />')
    expect(bootstrap).toContain("import { clearPwaCaches, usePwa } from '../pwa'")
    expect(bootstrap).toContain('usePwa()')
  })

  test('uses one composer input in both chat and AI workspaces', () => {
    const assistant = source('./components/AiAssistantPage.vue')
    const chat = source('./components/MessageComposer.vue')
    expect(assistant).toContain("import ComposerInput from './ComposerInput.vue'")
    expect(assistant).toContain('<ComposerInput')
    expect(chat).toContain("import ComposerInput from './ComposerInput.vue'")
    expect(chat).toContain('<ComposerInput')
    expect(assistant).not.toContain('<AiPromptComposer')
  })

  test('keeps the AI composer inside the fixed-height workspace below long answers', () => {
    const assistant = source('./components/AiAssistantPage.vue')
    const workspace = source('./workspace.css')
    expect(workspace).toContain('height: 100%;')
    expect(workspace).toContain('overflow: hidden;')
    expect(assistant).toContain(
      'class="cr-page grid h-full min-h-0 min-w-0 flex-1 grid-rows-[auto_minmax(0,1fr)] overflow-hidden"',
    )
    expect(assistant).toContain('class="flex min-h-0 flex-col overflow-hidden"')
    expect(assistant).toContain('class="grid min-h-0 flex-1 grid-rows-[auto_minmax(0,1fr)] overflow-hidden"')
    expect(assistant).toContain('class="flex min-h-0 min-w-0 flex-col overflow-hidden"')
    expect(assistant).toContain('class="min-h-0 flex-1 overflow-hidden"')
    expect(assistant).not.toContain('max-h-[calc(100%-4.5rem)]')
    expect(assistant).not.toContain('grid-rows-[auto_auto_minmax(0,1fr)_auto]')
  })

  test('exposes a named multi-select action on every chat message', () => {
    const actions = source('./components/MessageHoverActions.vue')
    const list = source('./components/MessageList.vue')
    expect(actions).toContain('多选消息')
    expect(actions).toContain("emit('select')")
    expect(list).toContain('@select="emit(\'toggleSelect\', message.message_id)"')
  })
})
