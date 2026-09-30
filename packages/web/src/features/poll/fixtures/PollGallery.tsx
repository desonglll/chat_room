/**
 * Dev fixture page for TG-406: every poll state inside a real TG-103 bubble, with a local vote
 * simulation (no server) so the ballot → results transition and the bar animation can be
 * watched, plus the creation dialog.
 */
import { createContext, useContext, useState } from 'react'
import type { BroadcastMessage, PollState } from '@tg/core'
import { Button } from '@tg/ui'
import { MessageBubble, registerMessageContent, type MessageContentProps } from '../../message'
import { makeCtx, makeMessage, VIEWER } from '../../message/fixtures/bubbleFixtures'
import { PollBody } from '../PollContent'
import { PollCreateDialog } from '../PollCreateDialog'
import { isPollMessage } from '../register'
import type { PollController } from '../usePoll'

type Simulate = (id: string, options: number[] | null, close?: boolean) => void
const SimulationContext = createContext<{ polls: Map<string, PollState>; simulate: Simulate } | null>(null)

function FixturePollContent({ message, metaSpacer }: MessageContentProps) {
  const simulation = useContext(SimulationContext)
  if (!message.poll || !simulation) return null
  const poll = simulation.polls.get(message.poll.id) ?? message.poll
  const controller: PollController = {
    poll,
    viewerId: VIEWER,
    busy: false,
    error: null,
    vote: (options) => simulation.simulate(poll.id, options),
    retract: () => simulation.simulate(poll.id, null),
    close: () => simulation.simulate(poll.id, poll.chosen ?? [], true),
  }
  return <PollBody poll={poll} senderId={message.sender_id} metaSpacer={metaSpacer} controller={controller} />
}

registerMessageContent('poll-fixture', FixturePollContent, { match: isPollMessage, priority: 60 })

const option = (text: string, voters: number) => ({ text, voters })

const FIXTURES: Array<{ title: string; out: boolean; message: BroadcastMessage }> = [
  {
    title: '匿名单选 · 未投票',
    out: false,
    message: makeMessage({
      poll: {
        id: 'p-single',
        question: '周五团建去哪里？',
        closed: false,
        total_voters: 7,
        options: [option('爬山', 2), option('密室逃脱', 1), option('火锅', 3), option('看电影', 1)],
        chosen: [],
        revision: 1,
      },
    }),
  },
  {
    title: '公开多选 · 自己发起',
    out: true,
    message: makeMessage({
      sender_id: VIEWER,
      poll: {
        id: 'p-multi',
        question: '哪几天有空？',
        closed: false,
        total_voters: 5,
        options: [option('周五晚上', 3), option('周六', 4), option('周日', 1)],
        public_voters: true,
        multiple_choice: true,
        chosen: [],
        revision: 1,
      },
    }),
  },
  {
    title: '测验 · 已答错',
    out: false,
    message: makeMessage({
      poll: {
        id: 'p-quiz',
        question: 'Telegram 最早发布于哪一年？',
        closed: false,
        total_voters: 12,
        options: [option('2011', 2), option('2013', 8), option('2015', 2)],
        quiz: true,
        correct_option: 1,
        explanation: 'Telegram 于 2013 年 8 月发布 iOS 版。',
        chosen: [2],
        revision: 1,
      },
    }),
  },
  {
    title: '投票已结束',
    out: false,
    message: makeMessage({
      poll: {
        id: 'p-closed',
        question: '新 logo 用哪一版？',
        closed: true,
        total_voters: 30,
        options: [option('蓝色', 18), option('绿色', 9), option('橙色', 3)],
        public_voters: true,
        chosen: [],
        revision: 4,
      },
    }),
  },
]

function simulateVote(poll: PollState, options: number[] | null, close: boolean): PollState {
  const before = poll.chosen ?? []
  const after = close ? before : (options ?? [])
  const counts = poll.options.map((entry, index) => {
    const delta = (after.includes(index) ? 1 : 0) - (before.includes(index) ? 1 : 0)
    return { ...entry, voters: entry.voters + delta }
  })
  const voterDelta = (after.length > 0 ? 1 : 0) - (before.length > 0 ? 1 : 0)
  return {
    ...poll,
    options: counts,
    total_voters: poll.total_voters + voterDelta,
    chosen: after,
    closed: poll.closed || close,
    revision: (poll.revision ?? 0) + 1,
  }
}

export function PollGallery() {
  const [polls, setPolls] = useState(() => new Map<string, PollState>())
  const [dialog, setDialog] = useState(false)
  const [theme, setTheme] = useState<'day' | 'night'>('day')
  const simulate: Simulate = (id, options, close = false) =>
    setPolls((current) => {
      const base = current.get(id) ?? FIXTURES.find((f) => f.message.poll?.id === id)?.message.poll
      return base ? new Map(current).set(id, simulateVote(base, options, close)) : current
    })
  const switchTheme = () => {
    const next = theme === 'day' ? 'night' : 'day'
    document.documentElement.setAttribute('data-tg-theme', next)
    setTheme(next)
  }
  return (
    <SimulationContext.Provider value={{ polls, simulate }}>
      <div className="fx-toolbar">
        <strong>TG-406 polls</strong>
        <Button variant="tonal" size="sm" onClick={switchTheme}>
          {theme === 'day' ? '切换到夜间' : '切换到日间'}
        </Button>
        <Button size="sm" onClick={() => setDialog(true)}>
          新建投票
        </Button>
      </div>
      <div className="fx-list">
        {FIXTURES.map((fixture) => (
          <section key={fixture.title}>
            <p className="fx-label">{fixture.title}</p>
            <MessageBubble
              message={fixture.message}
              ctx={makeCtx({ isOutgoing: fixture.out, showSenderName: !fixture.out })}
              viewerId={VIEWER}
            />
          </section>
        ))}
      </div>
      <PollCreateDialog open={dialog} chatId="fixture-chat" onClose={() => setDialog(false)} />
    </SimulationContext.Provider>
  )
}
