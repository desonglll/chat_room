/**
 * A topic's icon, Telegram style: its emoji when set, else a speech-bubble glyph filled
 * with the topic's palette colour carrying the title's first letter; General is a "#".
 * The colour is topic data (one of six server-checked values), passed as a custom
 * property so the stylesheet stays token-only.
 */
import type { CSSProperties } from 'react'
import { topicColorHex } from '@tg/core'
import { topicInitial } from './topicListModel'

export interface TopicIconProps {
  title: string
  emoji: string
  color: number
  general?: boolean | undefined
  size?: number | undefined
}

const BUBBLE =
  'M12 2.2C6.4 2.2 2 6.1 2 10.9c0 2.5 1.2 4.7 3.1 6.3l-.8 4.1 4.4-2.1c1 .3 2.1.4 3.3.4 5.6 0 10-3.9 10-8.7S17.6 2.2 12 2.2Z'

export function TopicIcon({ title, emoji, color, general = false, size = 28 }: TopicIconProps) {
  const box: CSSProperties = { inlineSize: size, blockSize: size, fontSize: Math.round(size * 0.78) }
  if (general) {
    return (
      <span className="tg-topic-icon tg-topic-icon--general" style={box} aria-hidden="true">
        #
      </span>
    )
  }
  if (emoji) {
    return (
      <span className="tg-topic-icon tg-topic-icon--emoji" style={box} aria-hidden="true">
        {emoji}
      </span>
    )
  }
  const tint = { ...box, '--topic-color': topicColorHex(color) } as CSSProperties
  return (
    <span className="tg-topic-icon tg-topic-icon--glyph" style={tint} aria-hidden="true">
      <svg viewBox="0 0 24 24" width={size} height={size} focusable="false">
        <path d={BUBBLE} className="tg-topic-icon__bubble" />
        <text x="12" y="11.2" textAnchor="middle" dominantBaseline="central" className="tg-topic-icon__letter">
          {topicInitial(title)}
        </text>
      </svg>
    </span>
  )
}
