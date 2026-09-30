/**
 * TG-104 markup: the composer's structure and accessible names over static rendering
 * (`bun test` has no DOM; behaviour is covered by composerController/uploads tests).
 */
import { describe, expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import { AttachMenu } from '../AttachMenu'
import { ComposerBar } from '../ComposerBar'
import { Composer } from '../Composer'
import { FormatToolbar } from '../FormatToolbar'
import { MentionPopup } from '../MentionPopup'

const session = { sendMessage: () => true, setDraftText: () => undefined, sendFrame: () => true }

describe('Composer markup', () => {
  test('idle: emoji, labelled textarea, attachments, voice button disabled until TG-401', () => {
    const html = renderToStaticMarkup(<Composer chatId="c1" currentUserId="me" members={[]} session={session} />)
    expect(html).toContain('class="tg-compose"')
    expect(html).toContain('aria-label="表情"')
    expect(html).toContain('aria-label="消息内容"')
    expect(html).toContain('aria-label="添加附件"')
    expect(html).toMatch(
      /aria-label="语音消息（即将推出）"[^>]*disabled|disabled[^>]*aria-label="语音消息（即将推出）"/,
    )
    expect(html).toContain('data-kind="voice"')
    expect(html).not.toContain('data-kind="send"')
  })

  test('reply / edit / forward bars name what they do and offer a cancel', () => {
    const reply = renderToStaticMarkup(
      <ComposerBar chatId="c1" bar={{ kind: 'reply', messageId: 'm' }} onCancel={() => undefined} />,
    )
    expect(reply).toContain('data-kind="reply"')
    expect(reply).toContain('回复消息')
    expect(reply).toContain('aria-label="取消"')
    const edit = renderToStaticMarkup(
      <ComposerBar chatId="c1" bar={{ kind: 'edit', messageId: 'm' }} onCancel={() => undefined} />,
    )
    expect(edit).toContain('编辑消息')
    const forward = renderToStaticMarkup(
      <ComposerBar
        chatId="c1"
        bar={{ kind: 'forward', messageIds: ['a', 'b'], fromChatId: 'c2' }}
        onCancel={() => undefined}
      />,
    )
    expect(forward).toContain('转发 2 条消息')
    expect(renderToStaticMarkup(<ComposerBar chatId="c1" bar={{ kind: 'none' }} onCancel={() => undefined} />)).toBe('')
  })

  test('mention list is an ARIA listbox with the active option selected', () => {
    const html = renderToStaticMarkup(
      <MentionPopup
        id="ml"
        candidates={[
          { user_id: 'u1', username: 'alice', avatar_emoji: '' },
          { user_id: 'u2', username: 'bob', avatar_emoji: '' },
        ]}
        activeIndex={1}
        onPick={() => undefined}
        onHover={() => undefined}
      />,
    )
    expect(html).toContain('role="listbox"')
    expect(html).toContain('id="ml-opt-1"')
    expect(html).toMatch(/id="ml-opt-1" role="option" aria-selected="true"/)
    expect(html).toContain('@alice')
  })

  test('format toolbar exposes every shortcut with its key hint', () => {
    const html = renderToStaticMarkup(<FormatToolbar onFormat={() => undefined} />)
    expect(html).toContain('role="toolbar"')
    for (const label of ['粗体', '斜体', '下划线', '删除线', '等宽', '链接'])
      expect(html).toContain(`aria-label="${label}"`)
    expect(html).toContain('Ctrl+B')
  })

  test('attachment menu trigger is a labelled menu button with hidden pickers', () => {
    const html = renderToStaticMarkup(<AttachMenu onFiles={() => undefined} />)
    expect(html).toContain('aria-haspopup="menu"')
    expect(html).toContain('accept="image/*,video/*"')
  })
})
