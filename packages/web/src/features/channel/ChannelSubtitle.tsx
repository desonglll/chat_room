/** The chat header's second line for a channel: the subscriber count, not presence. */
import { chatListStore, selectChatById } from '@tg/core'
import { useStore } from 'zustand/react'
import { subscriberLine } from './channelModel'

export function ChannelSubtitle({ chatId, className }: { chatId: string; className?: string | undefined }) {
  const count = useStore(chatListStore, (state) => selectChatById(chatId)(state)?.member_count ?? 0)
  return <span className={className}>{subscriberLine(count)}</span>
}
