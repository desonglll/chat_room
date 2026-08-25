import { describe, expect, test } from 'bun:test'

async function source(relativePath: string): Promise<string> {
  return Bun.file(`${import.meta.dir}/${relativePath}`).text()
}

describe('chat UI regression contracts', () => {
  test('composer textarea does not leave an inline baseline gap', async () => {
    expect(await source('composer.css')).toContain('display: block;')
  })

  test('password rooms expose the remember switch beside the password field', async () => {
    const component = await source('components/ChatAccessPanel.vue')
    expect(component).toContain('ToggleSwitch')
    expect(component).toContain('切换会话时记住密码')
    expect(component).toContain('update:rememberRoomPasswords')
  })

  test('contact profile cards expose their actions in the header', async () => {
    const component = await source('components/ProfileCardDialog.vue')
    expect(component).toContain('aria-label="联系人操作"')
    expect(component).toContain('设置备注')
    expect(component).toContain('删除好友')
    expect(component).toContain('拉黑')
  })

  test('favorite action communicates and renders its pressed state', async () => {
    const component = await source('components/MessageHoverActions.vue')
    expect(component).toContain('favorited: boolean')
    expect(component).toContain(':aria-pressed="favorited"')
    expect(component).toContain(":fill=\"favorited ? 'currentColor' : 'none'\"")
  })
})
