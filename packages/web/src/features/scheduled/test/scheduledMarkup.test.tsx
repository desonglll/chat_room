// TG-404 markup (static rendering, no DOM): the send button sits inside the send-options
// context-menu region, and the calendar entry stays hidden while a chat has nothing scheduled.
import { describe, expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import type { ComposerController } from '../../composer/composerController'
import { ScheduledEntry } from '../../composer/ScheduledEntry'
import { SendMenu } from '../../composer/SendMenu'

const controller = {
  schedulePayload: () => ({ content: 'x', replyTo: null }),
  submitSilent: () => 'sent',
  scheduled: () => undefined,
} as unknown as ComposerController

describe('scheduled composer entries', () => {
  test('the send button is wrapped by the send-options menu region', () => {
    const html = renderToStaticMarkup(
      <SendMenu chatId="c1" controller={controller} enabled>
        <button type="button" data-kind="send">
          发送
        </button>
      </SendMenu>,
    )
    expect(html).toContain('class="tg-compose__send-menu"')
    expect(html).toContain('class="tg-context-menu"')
    expect(html).toContain('data-kind="send"')
  })

  test('no calendar entry for a chat without scheduled messages', () => {
    const html = renderToStaticMarkup(<ScheduledEntry chatId="c-none" />)
    expect(html).not.toContain('data-kind="scheduled"')
  })
})
