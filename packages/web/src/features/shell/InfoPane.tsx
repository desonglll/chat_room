/**
 * The right pane: minimal chat info — avatar, title, description, members with their
 * status. M1's info panel replaces the body; the pane's mount/unmount contract
 * (`uiStore.activePanel === 'chatInfo'`) stays.
 */
import { chatListStore, presenceStore, selectChatById, selectPresence, uiStore } from '@tg/core'
import { useStore } from 'zustand/react'
import { Avatar, IconButton, ScrollArea } from '@tg/ui'
import { SpriteIcon } from './SpriteIcon'

export function InfoPane() {
  const chatId = useStore(uiStore, (state) => state.activeChatId)
  const chat = useStore(chatListStore, selectChatById(chatId))
  const presence = useStore(presenceStore, selectPresence(chatId))
  const closePanel = useStore(uiStore, (state) => state.closePanel)

  if (!chat) return null

  const online = new Set(presence.participants.map((member) => member.user_id))

  return (
    <aside className="tg-info" aria-label="会话信息">
      <header className="tg-info__header">
        <h2 className="tg-info__heading">会话信息</h2>
        <IconButton label="关闭信息面板" variant="plain" onClick={closePanel}>
          <SpriteIcon name="back" size={20} />
        </IconButton>
      </header>
      <ScrollArea className="tg-info__body" orientation="vertical" overlay>
        <div className="tg-info__identity">
          <Avatar label={chat.title} initials={chat.avatar_emoji || undefined} size="xl" />
          <h3 className="tg-info__title">{chat.title}</h3>
          <p className="tg-info__meta">{chat.member_count} 位成员</p>
        </div>
        {chat.description ? <p className="tg-info__description">{chat.description}</p> : null}
        <h4 className="tg-info__section">成员</h4>
        <ul className="tg-info__members">
          {presence.members.map((member) => (
            <li key={member.user_id} className="tg-info__member">
              <Avatar
                label={member.username}
                initials={member.avatar_emoji || undefined}
                size="sm"
                online={online.has(member.user_id)}
              />
              <span className="tg-info__member-name">{member.username}</span>
              {online.has(member.user_id) ? <span className="tg-info__member-state">在线</span> : null}
            </li>
          ))}
        </ul>
      </ScrollArea>
    </aside>
  )
}
