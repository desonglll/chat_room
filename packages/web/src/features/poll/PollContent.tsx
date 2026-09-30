/**
 * The poll bubble body (TG-406), registered with TG-103's content registry by `register.ts`.
 *
 * Two faces, as in Telegram: a ballot (tap an option; multiple-choice collects ticks and
 * sends with «投票») until the viewer votes or the poll closes, then results with live
 * percentages and bars. Bars grow on appearance and glide on every update; both collapse
 * under `prefers-reduced-motion` (poll.css).
 */
import { lazy, Suspense, useState, type CSSProperties } from 'react'
import type { PollState } from '@tg/core'
import type { MessageContentProps } from '../message'
import { BulbGlyph, CheckGlyph, CrossGlyph } from './glyphs'
import { describePoll, type PollOptionView } from './pollView'
import { usePoll, type PollController } from './usePoll'

const PollVotersDialog = lazy(() => import('./PollVotersDialog'))

export function PollContent({ message, metaSpacer }: MessageContentProps) {
  if (!message.poll) return null
  return <PollBody poll={message.poll} senderId={message.sender_id} metaSpacer={metaSpacer} />
}

interface PollBodyProps {
  poll: PollState
  senderId: string | null
  metaSpacer: MessageContentProps['metaSpacer']
  /** Tests inject a controller; the app uses `usePoll`. */
  controller?: PollController | undefined
}

export function PollBody({ poll: fromMessage, senderId, metaSpacer, controller }: PollBodyProps) {
  const live = usePollOr(fromMessage, controller)
  const { poll, busy } = live
  const view = describePoll(poll)
  const [ticked, setTicked] = useState<number[]>([])
  const [votersOpen, setVotersOpen] = useState(false)
  const ballot = !view.showResults
  const multiple = poll.multiple_choice === true

  const choose = (index: number) => {
    if (busy) return
    if (!multiple) {
      live.vote([index])
      return
    }
    setTicked((current) => (current.includes(index) ? current.filter((i) => i !== index) : [...current, index]))
  }

  return (
    <div className="tg-poll" data-results={view.showResults || undefined} aria-busy={busy || undefined}>
      <div className="tg-poll__question">{poll.question}</div>
      <div className="tg-poll__kind">{view.kindLabel}</div>
      <ul className="tg-poll__options">
        {view.options.map((option) =>
          ballot ? (
            <li key={option.index}>
              <button
                type="button"
                className="tg-poll__choice"
                role={multiple ? 'checkbox' : undefined}
                aria-checked={multiple ? ticked.includes(option.index) : undefined}
                disabled={busy}
                onClick={(event) => {
                  event.stopPropagation()
                  choose(option.index)
                }}
              >
                <span
                  className="tg-poll__marker"
                  data-shape={multiple ? 'square' : 'round'}
                  data-ticked={ticked.includes(option.index) || undefined}
                >
                  {ticked.includes(option.index) ? <CheckGlyph /> : null}
                </span>
                <span className="tg-poll__text">{option.text}</span>
              </button>
            </li>
          ) : (
            <ResultRow key={option.index} option={option} />
          ),
        )}
      </ul>
      {view.explanation ? (
        <div className="tg-poll__explanation">
          <BulbGlyph />
          <span>{view.explanation}</span>
        </div>
      ) : null}
      {live.error ? (
        <div className="tg-poll__error" role="alert">
          {live.error}
        </div>
      ) : null}
      <div className="tg-poll__footer">
        {ballot && multiple ? (
          <button
            type="button"
            className="tg-poll__action tg-poll__action--primary"
            disabled={busy || ticked.length === 0}
            onClick={(event) => {
              event.stopPropagation()
              live.vote([...ticked].sort((a, b) => a - b))
            }}
          >
            投票
          </button>
        ) : (
          <span className="tg-poll__count">{view.footer}</span>
        )}
        {poll.public_voters && view.showResults && poll.total_voters > 0 ? (
          <FooterAction label="查看投票人" onClick={() => setVotersOpen(true)} />
        ) : null}
        {view.canRetract ? <FooterAction label="撤回投票" disabled={busy} onClick={live.retract} /> : null}
        {!poll.closed && senderId !== null && senderId === live.viewerId ? (
          <FooterAction label={poll.quiz ? '结束测验' : '结束投票'} disabled={busy} onClick={live.close} />
        ) : null}
        {metaSpacer}
      </div>
      {votersOpen ? (
        <Suspense fallback={null}>
          <PollVotersDialog poll={poll} open onClose={() => setVotersOpen(false)} />
        </Suspense>
      ) : null}
    </div>
  )
}

function usePollOr(poll: PollState, controller: PollController | undefined): PollController {
  const live = usePoll(poll)
  return controller ?? live
}

function FooterAction({ label, onClick, disabled }: { label: string; onClick: () => void; disabled?: boolean }) {
  return (
    <button
      type="button"
      className="tg-poll__action"
      disabled={disabled}
      onClick={(event) => {
        event.stopPropagation()
        onClick()
      }}
    >
      {label}
    </button>
  )
}

function ResultRow({ option }: { option: PollOptionView }) {
  const barStyle = { '--poll-share': String(option.share) } as CSSProperties
  const label =
    option.mark === 'correct'
      ? '正确答案'
      : option.mark === 'wrong'
        ? '你的答案（错误）'
        : option.mark === 'chosen'
          ? '你的选择'
          : ''
  return (
    <li className="tg-poll__result" data-mark={option.mark === 'none' ? undefined : option.mark}>
      <span className="tg-poll__percent">{option.percent}%</span>
      <span className="tg-poll__result-body">
        <span className="tg-poll__text">
          {option.text}
          {option.mark !== 'none' ? (
            <span className="tg-poll__mark" role="img" aria-label={label}>
              {option.mark === 'wrong' ? <CrossGlyph /> : <CheckGlyph />}
            </span>
          ) : null}
        </span>
        <span className="tg-poll__bar" style={barStyle} aria-hidden="true" />
      </span>
    </li>
  )
}
