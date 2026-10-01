/**
 * The right info panel (TG-106): top bar, one of three identity headers, details with
 * the notification switch, then the shared-content tabs. The whole body is one scroll
 * column with a sticky tab strip, as in Telegram.
 */
import { useRef } from 'react'
import type { ApiClient } from '@tg/core'
import { chatListStore, presenceStore } from '@tg/core'
import { IconButton, ScrollArea } from '@tg/ui'
import { useStore } from 'zustand/react'
import { countMembers, PANEL_HEADING, withKnownMembers } from './chatInfoModel'
import { CloseIcon, SearchIcon } from './icons'
import { ChatAdminEntry } from '../chatAdmin/ChatAdminEntry'
import { ChatActions } from '../chatLifecycle'
import { InfoDetails, InfoIdentity } from './InfoHeader'
import { SharedSection } from './SharedSection'
import type { ChatInfoSourceOverrides } from './useChatInfo'
import { useChatInfo } from './useChatInfo'
import { t } from '../../i18n/index'

export interface ChatInfoPanelProps {
  chatId: string
  client: ApiClient
  onClose(): void
  /** "Search in this chat". The entry is hidden when no search surface is wired. */
  onSearchInChat?: ((chatId: string) => void) | undefined
  /** Playing the exit slide; the shell has already given the column back. */
  closing?: boolean | undefined
  onExited?: (() => void) | undefined
  /** Test/screenshot injection of the page sources. */
  sources?: ChatInfoSourceOverrides | undefined
}

export function ChatInfoPanel({
  chatId,
  client,
  onClose,
  onSearchInChat,
  closing = false,
  onExited,
  sources,
}: ChatInfoPanelProps) {
  const info = useChatInfo(chatId, client, sources)
  // Two primitive selections: an object selector would be a new snapshot on every read.
  const knownMembers = useStore(presenceStore, (state) => countMembers(state, chatId).members)
  const onlineCount = useStore(presenceStore, (state) => countMembers(state, chatId).online)
  const scrollerRef = useRef<HTMLDivElement>(null)
  // The loaded member list is proof too, once it is complete.
  const listedMembers = useStore(info.pagers.members, (state) => (state.done ? state.items.length : 0))
  const chat = useStore(chatListStore, (state) => state.chats.find((candidate) => candidate.id === chatId))
  const header = withKnownMembers(info.header, Math.max(knownMembers, listedMembers))

  return (
    <aside
      className="tg-info tg-chatinfo"
      aria-label={t('w.chatInfo.473797')}
      data-closing={closing || undefined}
      onAnimationEnd={(event) => {
        if (closing && event.target === event.currentTarget) onExited?.()
      }}
    >
      <header className="tg-chatinfo__bar">
        <IconButton label={t('w.chatInfo.5f589c')} variant="plain" onClick={onClose}>
          <CloseIcon />
        </IconButton>
        <h2 className="tg-chatinfo__heading">{header ? PANEL_HEADING[header.variant] : t('w.chatInfo.473797')}</h2>
        {onSearchInChat ? (
          <IconButton label={t('w.chatInfo.ed1d79')} variant="plain" onClick={() => onSearchInChat(chatId)}>
            <SearchIcon />
          </IconButton>
        ) : null}
      </header>
      {/* Keyed: another chat starts at its own top, not at the previous chat's offset. */}
      <ScrollArea key={chatId} className="tg-chatinfo__body" viewportRef={scrollerRef} orientation="vertical" overlay>
        {header ? (
          <>
            <InfoIdentity header={header} onlineCount={onlineCount} />
            <InfoDetails
              header={header}
              notificationsOn={info.notificationsOn}
              notificationsBusy={info.notificationsBusy}
              onNotificationsChange={info.setNotifications}
              chatId={chatId}
            />
            {header.variant === 'group' ? <ChatAdminEntry chatId={chatId} /> : null}
            <SharedSection
              chatId={chatId}
              pagers={info.pagers}
              showMembers={header.variant === 'group'}
              scrollerRef={scrollerRef}
            />
            {/* TG-701: edit, invite, leave, delete. */}
            {chat ? <ChatActions chat={chat} /> : null}
          </>
        ) : null}
      </ScrollArea>
    </aside>
  )
}
