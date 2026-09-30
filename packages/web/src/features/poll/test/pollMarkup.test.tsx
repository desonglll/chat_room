/** Server-rendered poll bubbles (packages/web has no DOM under `bun test`). */
import { describe, expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import type { PollState } from '@tg/core'
import { MessageBubble, messageContent } from '../../message'
import { makeCtx, makeMessage } from '../../message/fixtures/bubbleFixtures'
import '../register'
import { PollBody } from '../PollContent'
import type { PollController } from '../usePoll'
import { counted, makePoll } from './fixtures'

const noop = () => {}

function controller(poll: PollState, viewerId = 'u-me'): PollController {
  return { poll, viewerId, busy: false, error: null, vote: noop, retract: noop, close: noop }
}

function body(poll: PollState, senderId: string | null = 'u-alice', viewerId = 'u-me') {
  return renderToStaticMarkup(
    <PollBody poll={poll} senderId={senderId} metaSpacer={null} controller={controller(poll, viewerId)} />,
  )
}

describe('poll content registration', () => {
  test('a message carrying `poll` resolves to the poll kind; a recalled one stays deleted', () => {
    expect(messageContent.resolve(makeMessage({ poll: makePoll() }))?.kind).toBe('poll')
    expect(messageContent.resolve(makeMessage())?.kind).toBe('text')
    const recalled = makeMessage({ poll: makePoll(), recalled_at: '2026-10-01T00:00:00Z' })
    expect(messageContent.resolve(recalled)?.kind).toBe('deleted')
  })

  test('the whole bubble renders the poll body instead of the text', () => {
    const html = renderToStaticMarkup(
      <MessageBubble message={makeMessage({ content: '午饭吃什么？', poll: makePoll() })} ctx={makeCtx()} />,
    )
    expect(html).toContain('class="tg-poll"')
    expect(html).toContain('午饭吃什么？')
    expect(html).toContain('匿名投票')
  })
})

describe('poll body', () => {
  test('before voting: a ballot of buttons and no numbers', () => {
    const html = body(makePoll({ chosen: [] }))
    expect(html.split('tg-poll__choice').length - 1).toBe(3)
    expect(html).not.toContain('tg-poll__percent')
    expect(html).toContain('暂无投票')
  })

  test('multiple choice ballots are checkboxes with a vote button', () => {
    const html = body(makePoll({ chosen: [], multiple_choice: true }))
    expect(html).toContain('role="checkbox"')
    expect(html).toContain('>投票</button>')
  })

  test('after voting: percentages, animated bars sized by share, the pick marked', () => {
    const html = body(counted([3, 1], { chosen: [0] }))
    expect(html).toContain('>75%<')
    expect(html).toContain('>25%<')
    expect(html).toContain('--poll-share:0.75')
    expect(html).toContain('data-mark="chosen"')
    expect(html).toContain('撤回投票')
  })

  test('a quiz shows right and wrong, the explanation, and no retraction', () => {
    const html = body(counted([1, 1], { quiz: true, chosen: [0], correct_option: 1, explanation: '看书' }))
    expect(html).toContain('data-mark="wrong"')
    expect(html).toContain('data-mark="correct"')
    expect(html).toContain('看书')
    expect(html).not.toContain('撤回投票')
  })

  test('the voter list is offered for public polls only', () => {
    expect(body(counted([1, 0], { chosen: [0], public_voters: true }))).toContain('查看投票人')
    expect(body(counted([1, 0], { chosen: [0] }))).not.toContain('查看投票人')
  })

  test('only the author sees «结束投票», and never on a closed poll', () => {
    expect(body(makePoll(), 'u-me', 'u-me')).toContain('结束投票')
    expect(body(makePoll(), 'u-alice', 'u-me')).not.toContain('结束投票')
    expect(body(makePoll({ closed: true }), 'u-me', 'u-me')).not.toContain('结束投票')
  })
})
