import { describe, expect, test } from 'bun:test'
import { readFileSync } from 'node:fs'

function source(path: string): string {
  return readFileSync(new URL(path, import.meta.url), 'utf8')
}

describe('chat input experience contract', () => {
  test('does not render an install action over workspace controls', () => {
    const pwaStatus = source('./components/PwaStatusBar.vue')
    expect(pwaStatus).not.toContain('安装')
    expect(pwaStatus).not.toContain('installApp')
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

  test('exposes a named multi-select action on every chat message', () => {
    const actions = source('./components/MessageHoverActions.vue')
    const list = source('./components/MessageList.vue')
    expect(actions).toContain('多选消息')
    expect(actions).toContain("emit('select')")
    expect(list).toContain('@select="emit(\'toggleSelect\', message.message_id)"')
  })
})
